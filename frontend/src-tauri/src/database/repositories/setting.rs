use crate::database::models::{Setting, TranscriptSetting};
use crate::summary::CustomOpenAIConfig;
use sqlx::SqlitePool;

const KEYRING_SERVICE: &str = "ai.empathy.desktop";

fn keyring_account(kind: &str, provider: &str) -> String {
    format!("{}:{}", kind, provider.trim().to_ascii_lowercase())
}

fn keyring_error(error: keyring::Error) -> sqlx::Error {
    sqlx::Error::Protocol(format!("Secure credential store error: {}", error))
}

fn save_secure_secret(kind: &str, provider: &str, secret: &str) -> Result<(), sqlx::Error> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &keyring_account(kind, provider))
        .map_err(keyring_error)?;
    entry.set_password(secret).map_err(keyring_error)
}

fn get_secure_secret(kind: &str, provider: &str) -> Result<Option<String>, sqlx::Error> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &keyring_account(kind, provider))
        .map_err(keyring_error)?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(keyring_error(error)),
    }
}

fn delete_secure_secret(kind: &str, provider: &str) -> Result<(), sqlx::Error> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &keyring_account(kind, provider))
        .map_err(keyring_error)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(keyring_error(error)),
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct SaveModelConfigRequest {
    pub provider: String,
    pub model: String,
    #[serde(rename = "whisperModel")]
    pub whisper_model: String,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    #[serde(rename = "ollamaEndpoint")]
    pub ollama_endpoint: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
pub struct SaveTranscriptConfigRequest {
    pub provider: String,
    pub model: String,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
}

pub struct SettingsRepository;

// Transcript providers: localWhisper, deepgram, elevenLabs, groq, openai
// Summary providers: openai, claude, ollama, groq, added openrouter
// NOTE: Handle data exclusion in the higher layer as this is database abstraction layer(using SELECT *)

impl SettingsRepository {
    pub async fn migrate_legacy_secrets(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        for provider in ["openai", "claude", "ollama", "groq", "openrouter"] {
            let _ = Self::get_api_key(pool, provider).await?;
        }
        for provider in ["localWhisper", "deepgram", "elevenLabs", "groq", "openai"] {
            let _ = Self::get_transcript_api_key(pool, provider).await?;
        }
        let _ = Self::get_custom_openai_config(pool).await?;
        Ok(())
    }

    pub async fn get_model_config(
        pool: &SqlitePool,
    ) -> std::result::Result<Option<Setting>, sqlx::Error> {
        let setting = sqlx::query_as::<_, Setting>(
            r#"SELECT id, provider, model, whisperModel,
                       NULL AS groqApiKey,
                       NULL AS openaiApiKey,
                       NULL AS anthropicApiKey,
                       NULL AS ollamaApiKey,
                       NULL AS openRouterApiKey,
                       ollamaEndpoint,
                       customOpenAIConfig
                FROM settings LIMIT 1"#,
        )
        .fetch_optional(pool)
        .await?;
        Ok(setting)
    }

    pub async fn save_model_config(
        pool: &SqlitePool,
        provider: &str,
        model: &str,
        whisper_model: &str,
        ollama_endpoint: Option<&str>,
    ) -> std::result::Result<(), sqlx::Error> {
        // Using id '1' for backward compatibility
        sqlx::query(
            r#"
            INSERT INTO settings (id, provider, model, whisperModel, ollamaEndpoint)
            VALUES ('1', $1, $2, $3, $4)
            ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider,
                model = excluded.model,
                whisperModel = excluded.whisperModel,
                ollamaEndpoint = excluded.ollamaEndpoint
            "#,
        )
        .bind(provider)
        .bind(model)
        .bind(whisper_model)
        .bind(ollama_endpoint)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn save_api_key(
        pool: &SqlitePool,
        provider: &str,
        api_key: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        // Custom OpenAI uses JSON config (customOpenAIConfig) instead of a separate API key column
        if provider == "custom-openai" {
            return Err(sqlx::Error::Protocol(
                "custom-openai provider should use save_custom_openai_config() instead of save_api_key()".into(),
            ));
        }

        let api_key_column = match provider {
            "openai" => "openaiApiKey",
            "claude" => "anthropicApiKey",
            "ollama" => "ollamaApiKey",
            "groq" => "groqApiKey",
            "openrouter" => "openRouterApiKey",
            "builtin-ai" => return Ok(()), // No API key needed
            _ => {
                return Err(sqlx::Error::Protocol(
                    format!("Invalid provider: {}", provider).into(),
                ))
            }
        };

        save_secure_secret("summary", provider, api_key)?;

        // Keep legacy columns empty. Existing values are migrated on first read.
        let query = format!(
            "UPDATE settings SET {} = NULL WHERE id = '1'",
            api_key_column
        );
        sqlx::query(&query).execute(pool).await?;

        Ok(())
    }

    pub async fn get_api_key(
        pool: &SqlitePool,
        provider: &str,
    ) -> std::result::Result<Option<String>, sqlx::Error> {
        // Custom OpenAI uses JSON config - extract API key from there
        if provider == "custom-openai" {
            let config = Self::get_custom_openai_config(pool).await?;
            return Ok(config.and_then(|c| c.api_key));
        }

        let api_key_column = match provider {
            "openai" => "openaiApiKey",
            "ollama" => "ollamaApiKey",
            "groq" => "groqApiKey",
            "claude" => "anthropicApiKey",
            "openrouter" => "openRouterApiKey",
            "builtin-ai" => return Ok(None), // No API key needed
            _ => {
                return Err(sqlx::Error::Protocol(
                    format!("Invalid provider: {}", provider).into(),
                ))
            }
        };

        if let Some(secret) = get_secure_secret("summary", provider)? {
            return Ok(Some(secret));
        }

        let query = format!(
            "SELECT {} FROM settings WHERE id = '1' LIMIT 1",
            api_key_column
        );
        let legacy_key: Option<String> = sqlx::query_scalar(&query).fetch_optional(pool).await?;
        if let Some(ref secret) = legacy_key {
            save_secure_secret("summary", provider, secret)?;
            let clear_query = format!(
                "UPDATE settings SET {} = NULL WHERE id = '1'",
                api_key_column
            );
            sqlx::query(&clear_query).execute(pool).await?;
        }
        Ok(legacy_key)
    }

    pub async fn get_transcript_config(
        pool: &SqlitePool,
    ) -> std::result::Result<Option<TranscriptSetting>, sqlx::Error> {
        let setting = sqlx::query_as::<_, TranscriptSetting>(
            r#"SELECT id, provider, model,
                       NULL AS whisperApiKey,
                       NULL AS deepgramApiKey,
                       NULL AS elevenLabsApiKey,
                       NULL AS groqApiKey,
                       NULL AS openaiApiKey
                FROM transcript_settings LIMIT 1"#,
        )
        .fetch_optional(pool)
        .await?;
        Ok(setting)
    }

    pub async fn save_transcript_config(
        pool: &SqlitePool,
        provider: &str,
        model: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO transcript_settings (id, provider, model)
            VALUES ('1', $1, $2)
            ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider,
                model = excluded.model
            "#,
        )
        .bind(provider)
        .bind(model)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn save_transcript_api_key(
        pool: &SqlitePool,
        provider: &str,
        api_key: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        let api_key_column = match provider {
            "localWhisper" => "whisperApiKey",
            "parakeet" => return Ok(()), // Parakeet doesn't need an API key, return early
            "deepgram" => "deepgramApiKey",
            "elevenLabs" => "elevenLabsApiKey",
            "groq" => "groqApiKey",
            "openai" => "openaiApiKey",
            _ => {
                return Err(sqlx::Error::Protocol(
                    format!("Invalid provider: {}", provider).into(),
                ))
            }
        };

        save_secure_secret("transcript", provider, api_key)?;
        let query = format!(
            "UPDATE transcript_settings SET {} = NULL WHERE id = '1'",
            api_key_column
        );
        sqlx::query(&query).execute(pool).await?;

        Ok(())
    }

    pub async fn get_transcript_api_key(
        pool: &SqlitePool,
        provider: &str,
    ) -> std::result::Result<Option<String>, sqlx::Error> {
        let api_key_column = match provider {
            "localWhisper" => "whisperApiKey",
            "parakeet" => return Ok(None), // Parakeet doesn't need an API key
            "deepgram" => "deepgramApiKey",
            "elevenLabs" => "elevenLabsApiKey",
            "groq" => "groqApiKey",
            "openai" => "openaiApiKey",
            _ => {
                return Err(sqlx::Error::Protocol(
                    format!("Invalid provider: {}", provider).into(),
                ))
            }
        };

        if let Some(secret) = get_secure_secret("transcript", provider)? {
            return Ok(Some(secret));
        }

        let query = format!(
            "SELECT {} FROM transcript_settings WHERE id = '1' LIMIT 1",
            api_key_column
        );
        let legacy_key: Option<String> = sqlx::query_scalar(&query).fetch_optional(pool).await?;
        if let Some(ref secret) = legacy_key {
            save_secure_secret("transcript", provider, secret)?;
            let clear_query = format!(
                "UPDATE transcript_settings SET {} = NULL WHERE id = '1'",
                api_key_column
            );
            sqlx::query(&clear_query).execute(pool).await?;
        }
        Ok(legacy_key)
    }

    pub async fn delete_api_key(
        pool: &SqlitePool,
        provider: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        // Custom OpenAI uses JSON config - clear the entire config
        if provider == "custom-openai" {
            delete_secure_secret("summary", provider)?;
            sqlx::query("UPDATE settings SET customOpenAIConfig = NULL WHERE id = '1'")
                .execute(pool)
                .await?;
            return Ok(());
        }

        let api_key_column = match provider {
            "openai" => "openaiApiKey",
            "ollama" => "ollamaApiKey",
            "groq" => "groqApiKey",
            "claude" => "anthropicApiKey",
            "openrouter" => "openRouterApiKey",
            "builtin-ai" => return Ok(()), // No API key needed
            _ => {
                return Err(sqlx::Error::Protocol(
                    format!("Invalid provider: {}", provider).into(),
                ))
            }
        };

        let query = format!(
            "UPDATE settings SET {} = NULL WHERE id = '1'",
            api_key_column
        );
        delete_secure_secret("summary", provider)?;
        sqlx::query(&query).execute(pool).await?;

        Ok(())
    }

    // ===== CUSTOM OPENAI CONFIG METHODS =====

    /// Gets the custom OpenAI configuration from JSON
    ///
    /// # Returns
    /// * `Ok(Some(CustomOpenAIConfig))` - Config exists and is valid JSON
    /// * `Ok(None)` - No config stored
    /// * `Err(sqlx::Error)` - Database error
    pub async fn get_custom_openai_config(
        pool: &SqlitePool,
    ) -> std::result::Result<Option<CustomOpenAIConfig>, sqlx::Error> {
        use sqlx::Row;

        let row = sqlx::query(
            r#"
            SELECT customOpenAIConfig
            FROM settings
            WHERE id = '1'
            LIMIT 1
            "#,
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(record) => {
                let config_json: Option<String> = record.get("customOpenAIConfig");

                if let Some(json) = config_json {
                    // Parse JSON into CustomOpenAIConfig
                    let config: CustomOpenAIConfig = serde_json::from_str(&json).map_err(|e| {
                        sqlx::Error::Protocol(
                            format!("Invalid JSON in customOpenAIConfig: {}", e).into(),
                        )
                    })?;

                    let mut config = config;
                    let secure_secret = get_secure_secret("summary", "custom-openai")?;
                    if secure_secret.is_none() {
                        if let Some(legacy_secret) = config.api_key.as_deref() {
                            save_secure_secret("summary", "custom-openai", legacy_secret)?;
                            let mut public_config = config.clone();
                            public_config.api_key = None;
                            sqlx::query(
                                "UPDATE settings SET customOpenAIConfig = ? WHERE id = '1'",
                            )
                            .bind(serde_json::to_string(&public_config).map_err(|e| {
                                sqlx::Error::Protocol(format!(
                                    "Failed to sanitize custom OpenAI config: {}",
                                    e
                                ))
                            })?)
                            .execute(pool)
                            .await?;
                        }
                    }
                    config.api_key = secure_secret.or(config.api_key);
                    Ok(Some(config))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Saves the custom OpenAI configuration as JSON
    ///
    /// # Arguments
    /// * `pool` - Database connection pool
    /// * `config` - CustomOpenAIConfig to save (includes endpoint, apiKey, model, maxTokens, temperature, topP)
    ///
    /// # Returns
    /// * `Ok(())` - Config saved successfully
    /// * `Err(sqlx::Error)` - Database or JSON serialization error
    pub async fn save_custom_openai_config(
        pool: &SqlitePool,
        config: &CustomOpenAIConfig,
    ) -> std::result::Result<(), sqlx::Error> {
        if let Some(secret) = config
            .api_key
            .as_deref()
            .filter(|secret| !secret.trim().is_empty())
        {
            save_secure_secret("summary", "custom-openai", secret)?;
        } else {
            delete_secure_secret("summary", "custom-openai")?;
        }

        // Persist non-secret configuration only.
        let mut public_config = config.clone();
        public_config.api_key = None;
        let config_json = serde_json::to_string(&public_config).map_err(|e| {
            sqlx::Error::Protocol(format!("Failed to serialize config to JSON: {}", e).into())
        })?;

        // Upsert into settings table
        sqlx::query(
            r#"
            INSERT INTO settings (id, provider, model, whisperModel, customOpenAIConfig)
            VALUES ('1', 'custom-openai', $1, 'large-v3', $2)
            ON CONFLICT(id) DO UPDATE SET
                customOpenAIConfig = excluded.customOpenAIConfig
            "#,
        )
        .bind(&config.model)
        .bind(config_json)
        .execute(pool)
        .await?;

        Ok(())
    }
}
