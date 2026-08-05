//! Provider-neutral contracts and fail-closed feature gates for meeting integrations.
//!
//! Tokens never belong in these files. OAuth credentials are stored in the operating
//! system credential vault; portable metadata and audit receipts remain file-first.
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};
use tauri::{AppHandle, Manager, Runtime};

const SCHEMA: u32 = 1;
const DIRECTORY: &str = "integrations";
const FLAGS_FILE: &str = "feature-flags.json";
const ACCOUNTS_FILE: &str = "accounts.json";
const KEYRING_SERVICE: &str = "ai.empathy.desktop";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntegrationFeatureFlags {
    pub schema: u32,
    pub outlook_calendar: bool,
    pub outlook_mail_context: bool,
    pub teams_agent: bool,
    pub zoom_rtms: bool,
    pub google_meet: bool,
    pub google_meet_media_preview: bool,
}

impl Default for IntegrationFeatureFlags {
    fn default() -> Self {
        Self {
            schema: SCHEMA,
            outlook_calendar: false,
            outlook_mail_context: false,
            teams_agent: false,
            zoom_rtms: false,
            google_meet: false,
            google_meet_media_preview: false,
        }
    }
}

impl IntegrationFeatureFlags {
    fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!(
                "Schema de integrações não suportado: {}",
                self.schema
            ));
        }
        if self.outlook_mail_context && !self.outlook_calendar {
            return Err("Ative o calendário do Outlook antes do contexto de e-mails".into());
        }
        if self.google_meet_media_preview && !self.google_meet {
            return Err("Ative o Google Meet antes da mídia em tempo real".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationProvider {
    Microsoft,
    MicrosoftTeams,
    Zoom,
    GoogleMeet,
}

impl IntegrationProvider {
    fn key(&self) -> &'static str {
        match self {
            Self::Microsoft => "microsoft",
            Self::MicrosoftTeams => "microsoft-teams",
            Self::Zoom => "zoom",
            Self::GoogleMeet => "google-meet",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectorPermission {
    #[serde(rename = "calendar.basic")]
    CalendarBasic,
    #[serde(rename = "mail.metadata")]
    MailMetadata,
    #[serde(rename = "mail.content")]
    MailContent,
    #[serde(rename = "meeting.participants")]
    MeetingParticipants,
    #[serde(rename = "meeting.artifacts")]
    MeetingArtifacts,
    #[serde(rename = "meeting.realtime-media")]
    MeetingRealtimeMedia,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectedAccount {
    pub schema: u32,
    pub id: String,
    pub provider: IntegrationProvider,
    pub subject: String,
    pub tenant_id: Option<String>,
    pub email: String,
    pub display_name: String,
    pub granted_permissions: Vec<ConnectorPermission>,
    pub connected_at: String,
    pub updated_at: String,
}

impl ConnectedAccount {
    fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("Schema de conta não suportado: {}", self.schema));
        }
        validate_identifier(&self.id, "conta")?;
        if self.subject.trim().is_empty() || self.email.trim().is_empty() {
            return Err("Conta conectada sem identidade ou e-mail".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConnectedAccountsFile {
    schema: u32,
    accounts: Vec<ConnectedAccount>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityStage {
    LocalReady,
    ProviderSetup,
    AdminConsent,
    ExternalReview,
    DeveloperPreview,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationCapability {
    pub id: &'static str,
    pub provider: IntegrationProvider,
    pub name: &'static str,
    pub description: &'static str,
    pub stage: CapabilityStage,
    pub prerequisites: Vec<&'static str>,
    pub reads_user_data: bool,
    pub requires_explicit_action: bool,
}

fn capabilities() -> Vec<IntegrationCapability> {
    vec![
        IntegrationCapability {
            id: "outlook_calendar",
            provider: IntegrationProvider::Microsoft,
            name: "Calendário do Outlook",
            description: "Eventos, organizador e convidados da conta conectada.",
            stage: CapabilityStage::ProviderSetup,
            prerequisites: vec!["Microsoft Entra app", "OAuth PKCE", "Calendars.ReadBasic"],
            reads_user_data: true,
            requires_explicit_action: true,
        },
        IntegrationCapability {
            id: "outlook_mail_context",
            provider: IntegrationProvider::Microsoft,
            name: "Contexto selecionado de e-mails",
            description: "Mensagens escolhidas pelo usuário na própria caixa postal.",
            stage: CapabilityStage::ProviderSetup,
            prerequisites: vec!["Outlook conectado", "Mail.ReadBasic ou Mail.Read"],
            reads_user_data: true,
            requires_explicit_action: true,
        },
        IntegrationCapability {
            id: "teams_agent",
            provider: IntegrationProvider::MicrosoftTeams,
            name: "Agente Empathy para Teams",
            description: "Participante de IA visível, com consentimento e auditoria.",
            stage: CapabilityStage::AdminConsent,
            prerequisites: vec!["Serviço hospedado", "Teams app", "Consentimento do tenant"],
            reads_user_data: true,
            requires_explicit_action: true,
        },
        IntegrationCapability {
            id: "zoom_rtms",
            provider: IntegrationProvider::Zoom,
            name: "Agente Empathy para Zoom",
            description: "Eventos e mídia em tempo real por RTMS.",
            stage: CapabilityStage::ExternalReview,
            prerequisites: vec!["Zoom Developer app", "RTMS", "Revisão do Zoom"],
            reads_user_data: true,
            requires_explicit_action: true,
        },
        IntegrationCapability {
            id: "google_meet",
            provider: IntegrationProvider::GoogleMeet,
            name: "Google Meet",
            description: "Participantes, eventos e artefatos de transcrição.",
            stage: CapabilityStage::ProviderSetup,
            prerequisites: vec!["Google Cloud project", "OAuth", "Meet REST API"],
            reads_user_data: true,
            requires_explicit_action: true,
        },
        IntegrationCapability {
            id: "google_meet_media_preview",
            provider: IntegrationProvider::GoogleMeet,
            name: "Mídia em tempo real do Meet",
            description: "Áudio em tempo real protegido por gate de Developer Preview.",
            stage: CapabilityStage::DeveloperPreview,
            prerequisites: vec!["Meet Media API preview", "Consentimento dos participantes"],
            reads_user_data: true,
            requires_explicit_action: true,
        },
    ]
}

fn integrations_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join(DIRECTORY))
        .map_err(|error| error.to_string())
}

fn flags_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    integrations_dir(app).map(|path| path.join(FLAGS_FILE))
}

fn accounts_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    integrations_dir(app).map(|path| path.join(ACCOUNTS_FILE))
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 200
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.@".contains(character))
    {
        return Err(format!("Identificador de {label} inválido"));
    }
    Ok(())
}

fn read_accounts<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<ConnectedAccount>, String> {
    let path = accounts_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let file: ConnectedAccountsFile = serde_json::from_str(&raw)
        .map_err(|error| format!("Contas conectadas inválidas: {error}"))?;
    if file.schema != SCHEMA {
        return Err(format!("Schema de contas não suportado: {}", file.schema));
    }
    for account in &file.accounts {
        account.validate()?;
    }
    Ok(file.accounts)
}

fn write_accounts<R: Runtime>(
    app: &AppHandle<R>,
    accounts: &[ConnectedAccount],
) -> Result<(), String> {
    for account in accounts {
        account.validate()?;
    }
    write_recoverable_json(
        &accounts_path(app)?,
        &ConnectedAccountsFile {
            schema: SCHEMA,
            accounts: accounts.to_vec(),
        },
    )
}

fn read_flags<R: Runtime>(app: &AppHandle<R>) -> Result<IntegrationFeatureFlags, String> {
    let path = flags_path(app)?;
    if !path.exists() {
        return Ok(IntegrationFeatureFlags::default());
    }
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let flags: IntegrationFeatureFlags = serde_json::from_str(&raw)
        .map_err(|error| format!("Configuração de integrações inválida: {error}"))?;
    flags.validate()?;
    Ok(flags)
}

fn write_flags<R: Runtime>(
    app: &AppHandle<R>,
    flags: &IntegrationFeatureFlags,
) -> Result<(), String> {
    flags.validate()?;
    write_recoverable_json(&flags_path(app)?, flags)
}

fn write_recoverable_json<T: Serialize>(path: &std::path::Path, value: &T) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or_else(|| "Diretório de integrações inválido".to_string())?;
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    let backup = path.with_extension("json.bak");
    let json = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let mut file = fs::File::create(&temporary).map_err(|error| error.to_string())?;
    file.write_all(&json).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    if path.exists() {
        let _ = fs::remove_file(&backup);
        fs::rename(&path, &backup).map_err(|error| error.to_string())?;
    }
    if let Err(error) = fs::rename(&temporary, &path) {
        if backup.exists() {
            let _ = fs::rename(&backup, &path);
        }
        return Err(error.to_string());
    }
    let _ = fs::remove_file(backup);
    Ok(())
}

fn keyring_account(
    provider: &IntegrationProvider,
    account_id: &str,
    token_kind: &str,
) -> Result<String, String> {
    validate_identifier(account_id, "conta")?;
    validate_identifier(token_kind, "credencial")?;
    Ok(format!(
        "integration:{}:{}:{}",
        provider.key(),
        account_id,
        token_kind
    ))
}

fn delete_token(
    provider: &IntegrationProvider,
    account_id: &str,
    token_kind: &str,
) -> Result<(), String> {
    let account = keyring_account(provider, account_id, token_kind)?;
    let entry =
        keyring::Entry::new(KEYRING_SERVICE, &account).map_err(|error| error.to_string())?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Não foi possível remover a credencial segura: {error}"
        )),
    }
}

#[tauri::command]
pub fn api_get_integration_capabilities() -> Vec<IntegrationCapability> {
    capabilities()
}

#[tauri::command]
pub fn api_get_integration_feature_flags<R: Runtime>(
    app: AppHandle<R>,
) -> Result<IntegrationFeatureFlags, String> {
    read_flags(&app)
}

#[tauri::command]
pub fn api_save_integration_feature_flags<R: Runtime>(
    app: AppHandle<R>,
    flags: IntegrationFeatureFlags,
) -> Result<IntegrationFeatureFlags, String> {
    write_flags(&app, &flags)?;
    Ok(flags)
}

#[tauri::command]
pub fn api_list_connected_accounts<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<ConnectedAccount>, String> {
    read_accounts(&app)
}

#[tauri::command]
pub fn api_disconnect_integration_account<R: Runtime>(
    app: AppHandle<R>,
    account_id: String,
) -> Result<Vec<ConnectedAccount>, String> {
    validate_identifier(&account_id, "conta")?;
    let mut accounts = read_accounts(&app)?;
    let account = accounts
        .iter()
        .find(|account| account.id == account_id)
        .cloned()
        .ok_or_else(|| "Conta conectada não encontrada".to_string())?;

    // Remove credentials before metadata so a vault failure never leaves an
    // apparently disconnected account with reusable tokens.
    delete_token(&account.provider, &account.id, "access")?;
    delete_token(&account.provider, &account.id, "refresh")?;
    accounts.retain(|candidate| candidate.id != account_id);
    write_accounts(&app, &accounts)?;
    Ok(accounts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_flags_are_fail_closed() {
        let flags = IntegrationFeatureFlags::default();
        assert!(!flags.outlook_calendar);
        assert!(!flags.outlook_mail_context);
        assert!(!flags.teams_agent);
        assert!(!flags.zoom_rtms);
        assert!(!flags.google_meet);
        assert!(!flags.google_meet_media_preview);
    }

    #[test]
    fn dependent_capabilities_cannot_be_enabled_alone() {
        let flags = IntegrationFeatureFlags {
            outlook_mail_context: true,
            ..IntegrationFeatureFlags::default()
        };
        assert!(flags.validate().is_err());

        let flags = IntegrationFeatureFlags {
            google_meet_media_preview: true,
            ..IntegrationFeatureFlags::default()
        };
        assert!(flags.validate().is_err());
    }

    #[test]
    fn capability_matrix_exposes_every_gate() {
        let ids = capabilities()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "outlook_calendar",
                "outlook_mail_context",
                "teams_agent",
                "zoom_rtms",
                "google_meet",
                "google_meet_media_preview",
            ]
        );
    }

    #[test]
    fn credential_keys_reject_ambiguous_identifiers() {
        assert!(keyring_account(
            &IntegrationProvider::Microsoft,
            "user@example.com",
            "access"
        )
        .is_ok());
        assert!(keyring_account(&IntegrationProvider::Microsoft, "../user", "access").is_err());
        assert!(keyring_account(&IntegrationProvider::Microsoft, "user", "access token").is_err());
    }

    #[test]
    fn account_metadata_requires_an_explicit_identity() {
        let account = ConnectedAccount {
            schema: SCHEMA,
            id: "microsoft-user@example.com".into(),
            provider: IntegrationProvider::Microsoft,
            subject: String::new(),
            tenant_id: None,
            email: "user@example.com".into(),
            display_name: "User".into(),
            granted_permissions: vec![ConnectorPermission::CalendarBasic],
            connected_at: "2026-08-05T12:00:00Z".into(),
            updated_at: "2026-08-05T12:00:00Z".into(),
        };
        assert!(account.validate().is_err());
    }
}
