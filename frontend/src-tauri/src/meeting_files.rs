use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MEETING_FILE: &str = "meeting.md";
pub const TRANSCRIPT_FILE: &str = "transcript.md";
pub const SUMMARY_FILE: &str = "summary.md";

#[derive(Debug, Clone, Serialize)]
pub struct MarkdownTranscriptSegment {
    pub id: String,
    pub text: String,
    pub timestamp: String,
    pub audio_start_time: Option<f64>,
    pub audio_end_time: Option<f64>,
    pub duration: Option<f64>,
    pub speaker: Option<String>,
}

fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;

    let mut temp_file = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("Failed to create a temporary file in {}", parent.display()))?;
    temp_file
        .write_all(content.as_bytes())
        .with_context(|| format!("Failed to write temporary document for {}", path.display()))?;
    temp_file
        .as_file()
        .sync_all()
        .with_context(|| format!("Failed to sync temporary document for {}", path.display()))?;
    temp_file
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to replace {} atomically", path.display()))?;
    Ok(())
}

fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn summary_markdown(summary: &Value) -> Option<&str> {
    summary
        .get("markdown")
        .and_then(Value::as_str)
        .or_else(|| summary.as_str())
}

pub fn write_meeting_index(
    folder: &Path,
    meeting_id: &str,
    title: &str,
    created_at: &str,
    updated_at: &str,
) -> Result<PathBuf> {
    let content = format!(
        "---\n\
empathy_schema: 1\n\
id: {}\n\
title: {}\n\
created_at: {}\n\
updated_at: {}\n\
---\n\n\
# {}\n\n\
- [Transcrição](./{})\n\
- [Resumo](./{})\n\
- Áudio e outros anexos permanecem nesta pasta.\n",
        yaml_string(meeting_id),
        yaml_string(title),
        yaml_string(created_at),
        yaml_string(updated_at),
        title,
        TRANSCRIPT_FILE,
        SUMMARY_FILE,
    );
    let path = folder.join(MEETING_FILE);
    atomic_write(&path, &content)?;
    Ok(path)
}

pub fn update_machine_metadata(
    folder: &Path,
    meeting_id: &str,
    title: &str,
    transcript_file: &str,
) -> Result<()> {
    let metadata_path = folder.join("metadata.json");
    let mut metadata = if metadata_path.exists() {
        serde_json::from_str::<Value>(
            &fs::read_to_string(&metadata_path)
                .with_context(|| format!("Failed to read {}", metadata_path.display()))?,
        )
        .with_context(|| format!("Failed to parse {}", metadata_path.display()))?
    } else {
        Value::Object(serde_json::Map::new())
    };

    let object = metadata
        .as_object_mut()
        .context("metadata.json root must be an object")?;
    object.insert("meeting_id".into(), Value::String(meeting_id.into()));
    object.insert("meeting_name".into(), Value::String(title.into()));
    object.insert(
        "transcript_file".into(),
        Value::String(transcript_file.into()),
    );
    object.insert("content_format".into(), Value::String("markdown".into()));

    atomic_write(
        &metadata_path,
        &serde_json::to_string_pretty(&metadata).context("Failed to serialize metadata.json")?,
    )
}

pub fn write_transcript(
    folder: &Path,
    meeting_id: Option<&str>,
    title: &str,
    updated_at: &str,
    segments: &[MarkdownTranscriptSegment],
) -> Result<PathBuf> {
    let mut content = String::new();
    content.push_str("---\n");
    content.push_str("empathy_schema: 1\n");
    content.push_str("document: transcript\n");
    if let Some(meeting_id) = meeting_id {
        content.push_str(&format!("meeting_id: {}\n", yaml_string(meeting_id)));
    }
    content.push_str(&format!("title: {}\n", yaml_string(title)));
    content.push_str(&format!("updated_at: {}\n", yaml_string(updated_at)));
    content.push_str(&format!("segments: {}\n", segments.len()));
    content.push_str("---\n\n# Transcrição\n\n");

    if segments.is_empty() {
        content.push_str("_Nenhum trecho transcrito._\n");
    } else {
        for segment in segments {
            let speaker = segment.speaker.as_deref().unwrap_or("Participante");
            content.push_str(&format!("## {} — {}\n\n", segment.timestamp, speaker));
            content.push_str(&format!(
                "<!-- empathy-segment {} -->\n",
                serde_json::to_string(segment)
                    .context("Failed to serialize transcript metadata")?
            ));
            content.push_str(segment.text.trim());
            content.push_str("\n\n");
        }
    }

    let path = folder.join(TRANSCRIPT_FILE);
    atomic_write(&path, &content)?;
    Ok(path)
}

pub fn write_summary(
    folder: &Path,
    meeting_id: &str,
    title: &str,
    updated_at: &str,
    summary: &Value,
) -> Result<PathBuf> {
    let markdown = summary_markdown(summary).unwrap_or("_Resumo ainda não gerado._");
    let content = format!(
        "---\n\
empathy_schema: 1\n\
document: summary\n\
meeting_id: {}\n\
title: {}\n\
updated_at: {}\n\
---\n\n\
# Resumo\n\n{}\n",
        yaml_string(meeting_id),
        yaml_string(title),
        yaml_string(updated_at),
        markdown.trim(),
    );
    let path = folder.join(SUMMARY_FILE);
    atomic_write(&path, &content)?;
    Ok(path)
}

/// Materialize legacy indexed meetings into their existing recording folders.
/// Existing Markdown is never overwritten here: after creation it is the
/// user-owned document, while SQLite remains an operational index.
pub async fn backfill_missing_markdown(pool: &SqlitePool) -> Result<usize> {
    let meetings = crate::database::repositories::meeting::MeetingsRepository::get_meetings(pool)
        .await
        .context("Failed to list meetings for Markdown migration")?;
    let mut migrated = 0;

    for meeting in meetings {
        let Some(folder_path) = meeting
            .folder_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
        else {
            continue;
        };
        let folder = Path::new(folder_path);
        if !folder.exists() {
            log::warn!(
                "Skipping Markdown migration for {}: folder does not exist ({})",
                meeting.id,
                folder.display()
            );
            continue;
        }

        let transcripts = sqlx::query_as::<_, crate::database::models::Transcript>(
            "SELECT * FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time ASC",
        )
        .bind(&meeting.id)
        .fetch_all(pool)
        .await
        .with_context(|| format!("Failed to load transcripts for {}", meeting.id))?;
        let segments = transcripts
            .into_iter()
            .map(|segment| MarkdownTranscriptSegment {
                id: segment.id,
                text: segment.transcript,
                timestamp: segment.timestamp,
                audio_start_time: segment.audio_start_time,
                audio_end_time: segment.audio_end_time,
                duration: segment.duration,
                speaker: segment.speaker,
            })
            .collect::<Vec<_>>();

        if !folder.join(MEETING_FILE).exists() {
            write_meeting_index(
                folder,
                &meeting.id,
                &meeting.title,
                &meeting.created_at.0.to_rfc3339(),
                &meeting.updated_at.0.to_rfc3339(),
            )?;
        }
        if !folder.join(TRANSCRIPT_FILE).exists() {
            write_transcript(
                folder,
                Some(&meeting.id),
                &meeting.title,
                &meeting.updated_at.0.to_rfc3339(),
                &segments,
            )?;
        }

        if !folder.join(SUMMARY_FILE).exists() {
            let result = sqlx::query_scalar::<_, Option<String>>(
                "SELECT result FROM summary_processes WHERE meeting_id = ?",
            )
            .bind(&meeting.id)
            .fetch_optional(pool)
            .await
            .with_context(|| format!("Failed to load summary for {}", meeting.id))?
            .flatten();
            if let Some(result) = result {
                let summary = serde_json::from_str::<Value>(&result)
                    .unwrap_or_else(|_| Value::String(result));
                write_summary(
                    folder,
                    &meeting.id,
                    &meeting.title,
                    &meeting.updated_at.0.to_rfc3339(),
                    &summary,
                )?;
            }
        }

        update_machine_metadata(folder, &meeting.id, &meeting.title, TRANSCRIPT_FILE)?;
        migrated += 1;
    }

    Ok(migrated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_is_human_readable_and_keeps_machine_metadata() {
        let dir = tempfile::tempdir().unwrap();
        write_transcript(
            dir.path(),
            Some("meeting-1"),
            "Consulta inicial",
            "2026-08-02T18:00:00Z",
            &[MarkdownTranscriptSegment {
                id: "segment-1".into(),
                text: "Olá, como você está?".into(),
                timestamp: "00:03".into(),
                audio_start_time: Some(3.0),
                audio_end_time: Some(5.0),
                duration: Some(2.0),
                speaker: Some("Gabriel".into()),
            }],
        )
        .unwrap();

        let markdown = fs::read_to_string(dir.path().join(TRANSCRIPT_FILE)).unwrap();
        assert!(markdown.contains("# Transcrição"));
        assert!(markdown.contains("## 00:03 — Gabriel"));
        assert!(markdown.contains("Olá, como você está?"));
        assert!(markdown.contains("empathy-segment"));
    }

    #[test]
    fn summary_prefers_markdown_payload() {
        let dir = tempfile::tempdir().unwrap();
        write_summary(
            dir.path(),
            "meeting-1",
            "Reunião",
            "2026-08-02T18:00:00Z",
            &serde_json::json!({ "markdown": "## Decisões\n\n- Fazer." }),
        )
        .unwrap();

        let markdown = fs::read_to_string(dir.path().join(SUMMARY_FILE)).unwrap();
        assert!(markdown.contains("## Decisões"));
        assert!(!markdown.contains("summary_json"));
    }

    #[tokio::test]
    async fn backfill_creates_missing_documents_without_overwriting_them() {
        let dir = tempfile::tempdir().unwrap();
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE meetings (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, folder_path TEXT)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE transcripts (id TEXT PRIMARY KEY, meeting_id TEXT NOT NULL, transcript TEXT NOT NULL, timestamp TEXT NOT NULL, summary TEXT, action_items TEXT, key_points TEXT, audio_start_time REAL, audio_end_time REAL, duration REAL, speaker TEXT)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("CREATE TABLE summary_processes (meeting_id TEXT PRIMARY KEY, result TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO meetings VALUES (?, ?, ?, ?, ?)")
            .bind("meeting-legacy")
            .bind("Reunião antiga")
            .bind("2026-08-02T18:00:00Z")
            .bind("2026-08-02T19:00:00Z")
            .bind(dir.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO transcripts VALUES (?, ?, ?, ?, NULL, NULL, NULL, ?, ?, ?, ?)")
            .bind("segment-1")
            .bind("meeting-legacy")
            .bind("Conteúdo preservado")
            .bind("00:01")
            .bind(1.0_f64)
            .bind(2.0_f64)
            .bind(1.0_f64)
            .bind("Gabriel")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO summary_processes VALUES (?, ?)")
            .bind("meeting-legacy")
            .bind(r###"{"markdown":"## Resumo legado"}"###)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(backfill_missing_markdown(&pool).await.unwrap(), 1);
        assert!(fs::read_to_string(dir.path().join(TRANSCRIPT_FILE))
            .unwrap()
            .contains("Conteúdo preservado"));
        assert!(fs::read_to_string(dir.path().join(SUMMARY_FILE))
            .unwrap()
            .contains("Resumo legado"));

        fs::write(dir.path().join(TRANSCRIPT_FILE), "edição do usuário").unwrap();
        backfill_missing_markdown(&pool).await.unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join(TRANSCRIPT_FILE)).unwrap(),
            "edição do usuário"
        );
    }
}
