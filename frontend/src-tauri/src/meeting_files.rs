use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MEETING_FILE: &str = "meeting.md";
pub const TRANSCRIPT_FILE: &str = "transcript.md";
pub const SUMMARY_FILE: &str = "summary.md";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDocument {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
    pub recorded: bool,
    pub written: bool,
    pub archived: bool,
    pub folder_path: String,
    pub external_meeting: Option<serde_json::Value>,
    /// Hash of the complete meeting.md used for optimistic concurrency.
    pub content_hash: String,
}

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

fn split_meeting_document(content: &str) -> Result<(serde_yaml::Mapping, &str)> {
    let rest = content
        .strip_prefix("---\n")
        .context("meeting.md does not start with YAML frontmatter")?;
    let end = rest
        .find("\n---")
        .context("meeting.md frontmatter is not closed")?;
    let metadata = serde_yaml::from_str::<serde_yaml::Mapping>(&rest[..end])
        .context("Failed to parse meeting.md frontmatter")?;
    let body = rest
        .get(end + 4..)
        .unwrap_or_default()
        .trim_start_matches('\n');
    Ok((metadata, body))
}

fn origin_value(origin: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(origin.to_string())
}

fn note_origins(metadata: &serde_yaml::Mapping) -> Option<&Vec<serde_yaml::Value>> {
    metadata
        .get(origin_value("note_origins"))
        .and_then(serde_yaml::Value::as_sequence)
}

/// Legacy documents predate `note_origins` and are recordings. New written
/// notes always declare their origin explicitly.
pub fn meeting_has_recorded_content(folder: &Path) -> bool {
    let Ok(content) = fs::read_to_string(folder.join(MEETING_FILE)) else {
        return false;
    };
    let Ok((metadata, _)) = split_meeting_document(&content) else {
        return false;
    };
    note_origins(&metadata).map_or(true, |origins| {
        origins
            .iter()
            .any(|value| value.as_str() == Some("recorded"))
    })
}

/// Returns whether a recorded note also contains a user-authored change.
/// Missing metadata is treated as recorded-only for backwards compatibility.
pub fn meeting_has_written_content(folder: &Path) -> bool {
    let Ok(content) = fs::read_to_string(folder.join(MEETING_FILE)) else {
        return false;
    };
    let Ok((metadata, _)) = split_meeting_document(&content) else {
        return false;
    };
    note_origins(&metadata).is_some_and(|origins| {
        origins
            .iter()
            .any(|value| value.as_str() == Some("written"))
    })
}

pub fn write_written_note(
    folder: &Path,
    note_id: &str,
    title: &str,
    created_at: &str,
    updated_at: &str,
    body: &str,
) -> Result<PathBuf> {
    let content = format!(
        "---\n\
empathy_schema: 2\n\
type: note\n\
id: {}\n\
title: {}\n\
created_at: {}\n\
updated_at: {}\n\
project: \"\"\n\
participants: []\n\
tags: [note]\n\
status: active\n\
note_origins: [written]\n\
archived: false\n\
---\n\n{}\n",
        yaml_string(note_id),
        yaml_string(title),
        yaml_string(created_at),
        yaml_string(updated_at),
        body.trim(),
    );
    let path = folder.join(MEETING_FILE);
    atomic_write(&path, &content)?;
    Ok(path)
}

pub fn read_note_document(folder: &Path) -> Result<NoteDocument> {
    let path = folder.join(MEETING_FILE);
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let (metadata, body) = split_meeting_document(&content)?;
    let string_value = |key: &str| {
        metadata
            .get(origin_value(key))
            .and_then(serde_yaml::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let origins = note_origins(&metadata);
    let external_meeting = metadata
        .get(origin_value("external_meeting"))
        .map(serde_json::to_value)
        .transpose()
        .context("Failed to read external meeting metadata")?;
    let content_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
    Ok(NoteDocument {
        id: string_value("id"),
        title: string_value("title"),
        content: body.trim_end().to_string(),
        created_at: string_value("created_at"),
        updated_at: string_value("updated_at"),
        recorded: origins.map_or(true, |values| {
            values
                .iter()
                .any(|value| value.as_str() == Some("recorded"))
        }),
        written: origins
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("written"))),
        archived: metadata
            .get(origin_value("archived"))
            .and_then(serde_yaml::Value::as_bool)
            .unwrap_or(false),
        folder_path: folder.to_string_lossy().to_string(),
        external_meeting,
        content_hash,
    })
}

/// On the first explicit save/use of a recorded note, surface the legacy
/// summary as a signed skill result. summary.md remains untouched.
pub fn merge_legacy_summary(folder: &Path, body: &str) -> Result<String> {
    const MARKER: &str = "skill_id: legacy-meeting-summary";
    if body.contains(MARKER) {
        return Ok(body.to_string());
    }
    let path = folder.join(SUMMARY_FILE);
    if !path.exists() {
        return Ok(body.to_string());
    }
    let legacy =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    if legacy.trim().is_empty() {
        return Ok(body.to_string());
    }
    let block = format!(
        "<!-- empathy-skill-result\nid: {}\nskill_id: legacy-meeting-summary\nskill_name: Resumo da reunião\nlayer: artificial\ncreated_at: {}\nsource_scope: transcript\nprovider: legacy\nmodel: legacy\n-->\n## Resumo da reunião\n\n*Skill (Resumo da reunião)*\n\n{}\n<!-- /empathy-skill-result -->",
        uuid::Uuid::new_v4(), chrono::Utc::now().to_rfc3339(), legacy.trim()
    );
    Ok(if body.trim().is_empty() {
        block
    } else {
        format!("{}\n\n{}", body.trim_end(), block)
    })
}

#[cfg(test)]
mod skill_migration_tests {
    use super::*;
    #[test]
    fn legacy_summary_migration_is_idempotent_and_preserves_source() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join(SUMMARY_FILE),
            "## Decisões\n\nManter Markdown.",
        )
        .unwrap();
        let first = merge_legacy_summary(directory.path(), "Texto humano").unwrap();
        let second = merge_legacy_summary(directory.path(), &first).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.matches("skill_id: legacy-meeting-summary").count(), 1);
        assert_eq!(
            fs::read_to_string(directory.path().join(SUMMARY_FILE)).unwrap(),
            "## Decisões\n\nManter Markdown."
        );
    }
}

pub fn save_note_document(
    folder: &Path,
    title: &str,
    body: &str,
    updated_at: &str,
) -> Result<NoteDocument> {
    let path = folder.join(MEETING_FILE);
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let (mut metadata, _) = split_meeting_document(&content)?;
    metadata.insert(
        origin_value("title"),
        serde_yaml::Value::String(title.to_string()),
    );
    metadata.insert(
        origin_value("updated_at"),
        serde_yaml::Value::String(updated_at.to_string()),
    );
    let mut origins = note_origins(&metadata).cloned().unwrap_or_default();
    if !origins
        .iter()
        .any(|value| value.as_str() == Some("written"))
    {
        origins.push(origin_value("written"));
    }
    metadata.insert(
        origin_value("note_origins"),
        serde_yaml::Value::Sequence(origins),
    );
    let yaml = serde_yaml::to_string(&metadata).context("Failed to serialize note frontmatter")?;
    atomic_write(&path, &format!("---\n{}---\n\n{}\n", yaml, body.trim_end()))?;
    read_note_document(folder)
}

/// Archived notes stay in place and remain indexable; only their active-list
/// visibility changes. Legacy `status: archived` is also recognized.
pub fn meeting_is_archived(folder: &Path) -> bool {
    let Ok(content) = fs::read_to_string(folder.join(MEETING_FILE)) else {
        return false;
    };
    let Ok((metadata, _)) = split_meeting_document(&content) else {
        return false;
    };
    metadata
        .get(origin_value("archived"))
        .and_then(serde_yaml::Value::as_bool)
        .unwrap_or(false)
        || metadata
            .get(origin_value("status"))
            .and_then(serde_yaml::Value::as_str)
            .is_some_and(|status| status.eq_ignore_ascii_case("archived"))
}

pub fn set_meeting_archived(folder: &Path, archived: bool, updated_at: &str) -> Result<()> {
    let path = folder.join(MEETING_FILE);
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let (mut metadata, body) = split_meeting_document(&content)?;
    metadata.insert(origin_value("archived"), serde_yaml::Value::Bool(archived));
    metadata.insert(
        origin_value("updated_at"),
        serde_yaml::Value::String(updated_at.to_string()),
    );
    let yaml =
        serde_yaml::to_string(&metadata).context("Failed to serialize meeting.md frontmatter")?;
    atomic_write(&path, &format!("---\n{}---\n\n{}", yaml, body))
}

/// Marks a recorded note as manually written or edited while preserving all
/// user-owned Markdown body content and unrelated frontmatter properties.
pub fn mark_meeting_written(folder: &Path, updated_at: &str) -> Result<()> {
    let path = folder.join(MEETING_FILE);
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let (mut metadata, body) = split_meeting_document(&content)?;
    let mut origins = metadata
        .get(origin_value("note_origins"))
        .and_then(serde_yaml::Value::as_sequence)
        .cloned()
        .unwrap_or_else(|| vec![origin_value("recorded")]);
    if !origins
        .iter()
        .any(|value| value.as_str() == Some("recorded"))
    {
        origins.insert(0, origin_value("recorded"));
    }
    if !origins
        .iter()
        .any(|value| value.as_str() == Some("written"))
    {
        origins.push(origin_value("written"));
    }
    metadata.insert(
        origin_value("note_origins"),
        serde_yaml::Value::Sequence(origins),
    );
    metadata.insert(
        origin_value("updated_at"),
        serde_yaml::Value::String(updated_at.to_string()),
    );
    let yaml =
        serde_yaml::to_string(&metadata).context("Failed to serialize meeting.md frontmatter")?;
    atomic_write(&path, &format!("---\n{}---\n\n{}", yaml, body))
}

/// Attaches user-approved external meeting metadata while preserving the note body and
/// unrelated frontmatter. The caller is responsible for obtaining the event again from
/// the provider before invoking this write.
pub fn attach_external_meeting(
    folder: &Path,
    external_meeting: serde_yaml::Value,
    participants: &[String],
    updated_at: &str,
) -> Result<()> {
    let path = folder.join(MEETING_FILE);
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let (mut metadata, body) = split_meeting_document(&content)?;
    let key = |value: &str| serde_yaml::Value::String(value.to_string());
    metadata.insert(key("external_meeting"), external_meeting);
    metadata.insert(
        key("participants"),
        serde_yaml::Value::Sequence(
            participants
                .iter()
                .map(|participant| serde_yaml::Value::String(participant.clone()))
                .collect(),
        ),
    );
    metadata.insert(
        key("updated_at"),
        serde_yaml::Value::String(updated_at.to_string()),
    );
    let mut tags = metadata
        .get(key("tags"))
        .and_then(serde_yaml::Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    if !tags.iter().any(|value| value.as_str() == Some("outlook")) {
        tags.push(serde_yaml::Value::String("outlook".into()));
    }
    metadata.insert(key("tags"), serde_yaml::Value::Sequence(tags));
    let yaml = serde_yaml::to_string(&metadata).context("Failed to serialize note frontmatter")?;
    atomic_write(&path, &format!("---\n{}---\n\n{}", yaml, body))
}

pub fn write_meeting_index(
    folder: &Path,
    meeting_id: &str,
    title: &str,
    created_at: &str,
    updated_at: &str,
) -> Result<PathBuf> {
    let path = folder.join(MEETING_FILE);
    if path.exists() {
        let existing = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        if let Some(rest) = existing.strip_prefix("---\n") {
            if let Some(end) = rest.find("\n---") {
                let yaml = &rest[..end];
                let body = rest
                    .get(end + 4..)
                    .unwrap_or_default()
                    .trim_start_matches('\n');
                let mut metadata =
                    serde_yaml::from_str::<serde_yaml::Mapping>(yaml).with_context(|| {
                        format!("Failed to parse frontmatter in {}", path.display())
                    })?;
                let key = |value: &str| serde_yaml::Value::String(value.to_string());
                metadata.insert(key("empathy_schema"), serde_yaml::Value::Number(2.into()));
                metadata.insert(key("type"), serde_yaml::Value::String("meeting".into()));
                metadata.insert(key("id"), serde_yaml::Value::String(meeting_id.into()));
                metadata.insert(key("title"), serde_yaml::Value::String(title.into()));
                metadata.insert(
                    key("created_at"),
                    serde_yaml::Value::String(created_at.into()),
                );
                metadata.insert(
                    key("updated_at"),
                    serde_yaml::Value::String(updated_at.into()),
                );
                let mut replaced_heading = false;
                let updated_body = body
                    .lines()
                    .map(|line| {
                        if !replaced_heading && line.starts_with("# ") {
                            replaced_heading = true;
                            format!("# {}", title)
                        } else {
                            line.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let yaml = serde_yaml::to_string(&metadata)
                    .context("Failed to serialize meeting frontmatter")?;
                atomic_write(&path, &format!("---\n{}---\n\n{}\n", yaml, updated_body))?;
                return Ok(path);
            }
        }
        anyhow::bail!("Existing meeting.md has invalid frontmatter; refusing to overwrite it");
    }

    let content = format!(
        "---\n\
    empathy_schema: 2\n\
    type: meeting\n\
    id: {}\n\
    title: {}\n\
    created_at: {}\n\
    updated_at: {}\n\
    project: \"\"\n\
    participants: []\n\
    tags: [meeting]\n\
    status: completed\n\
    note_origins: [recorded]\n\
    archived: false\n\
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
    content.push_str("empathy_schema: 2\n");
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
	empathy_schema: 2\n\
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
                let summary =
                    serde_json::from_str::<Value>(&result).unwrap_or(Value::String(result));
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

    #[test]
    fn updating_title_preserves_user_owned_properties() {
        let dir = tempfile::tempdir().unwrap();
        write_meeting_index(
            dir.path(),
            "meeting-1",
            "Título inicial",
            "2026-08-02T18:00:00Z",
            "2026-08-02T18:00:00Z",
        )
        .unwrap();
        let path = dir.path().join(MEETING_FILE);
        let customized = fs::read_to_string(&path)
            .unwrap()
            .replace("project: \"\"", "project: EmpathyIA")
            .replace("participants: []", "participants: [Gabriel]");
        fs::write(&path, customized).unwrap();

        write_meeting_index(
            dir.path(),
            "meeting-1",
            "Título atualizado",
            "2026-08-02T18:00:00Z",
            "2026-08-03T18:00:00Z",
        )
        .unwrap();

        let markdown = fs::read_to_string(path).unwrap();
        assert!(markdown.contains("title: Título atualizado"));
        assert!(markdown.contains("project: EmpathyIA"));
        assert!(markdown.contains("participants:\n- Gabriel"));
    }

    #[test]
    fn written_origin_is_persisted_without_replacing_user_content() {
        let dir = tempfile::tempdir().unwrap();
        write_meeting_index(
            dir.path(),
            "meeting-1",
            "Reunião",
            "2026-08-02T18:00:00Z",
            "2026-08-02T18:00:00Z",
        )
        .unwrap();
        let path = dir.path().join(MEETING_FILE);
        let custom_body = fs::read_to_string(&path).unwrap().replace(
            "- Áudio e outros anexos permanecem nesta pasta.",
            "Meu texto manual.",
        );
        atomic_write(&path, &custom_body).unwrap();

        assert!(!meeting_has_written_content(dir.path()));
        mark_meeting_written(dir.path(), "2026-08-02T19:00:00Z").unwrap();

        let updated = fs::read_to_string(path).unwrap();
        assert!(meeting_has_written_content(dir.path()));
        assert!(updated.contains("Meu texto manual."));
        assert!(updated.contains("- written"));
    }

    #[test]
    fn archive_state_is_reversible_and_preserves_markdown() {
        let dir = tempfile::tempdir().unwrap();
        write_meeting_index(
            dir.path(),
            "meeting-1",
            "Reunião",
            "2026-08-02T18:00:00Z",
            "2026-08-02T18:00:00Z",
        )
        .unwrap();
        let path = dir.path().join(MEETING_FILE);
        let customized = fs::read_to_string(&path)
            .unwrap()
            .replace("# Reunião", "# Reunião\n\nTexto preservado");
        atomic_write(&path, &customized).unwrap();

        set_meeting_archived(dir.path(), true, "2026-08-02T19:00:00Z").unwrap();
        assert!(meeting_is_archived(dir.path()));
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("Texto preservado"));

        set_meeting_archived(dir.path(), false, "2026-08-02T20:00:00Z").unwrap();
        assert!(!meeting_is_archived(dir.path()));
        assert!(fs::read_to_string(path)
            .unwrap()
            .contains("Texto preservado"));
    }

    #[test]
    fn written_note_round_trip_uses_the_same_markdown_contract() {
        let dir = tempfile::tempdir().unwrap();
        write_written_note(
            dir.path(),
            "note-1",
            "Ideias do produto",
            "2026-08-04T12:00:00Z",
            "2026-08-04T12:00:00Z",
            "# Ideias\n\n- Um editor único",
        )
        .unwrap();

        let created = read_note_document(dir.path()).unwrap();
        assert_eq!(created.id, "note-1");
        assert!(!created.recorded);
        assert!(created.written);
        assert_eq!(created.content, "# Ideias\n\n- Um editor único");
        assert!(!meeting_has_recorded_content(dir.path()));

        let saved = save_note_document(
            dir.path(),
            "Ideias revisadas",
            "# Ideias\n\n- Biblioteca Markdown",
            "2026-08-04T12:30:00Z",
        )
        .unwrap();
        assert_eq!(saved.title, "Ideias revisadas");
        assert!(saved.content.contains("Biblioteca Markdown"));
        assert!(meeting_has_written_content(dir.path()));
    }

    #[test]
    fn external_meeting_metadata_preserves_body_and_updates_participants() {
        let dir = tempfile::tempdir().unwrap();
        write_written_note(
            dir.path(),
            "note-1",
            "Planejamento",
            "2026-08-05T12:00:00Z",
            "2026-08-05T12:00:00Z",
            "# Planejamento\n\nTexto do usuário.",
        )
        .unwrap();
        attach_external_meeting(
            dir.path(),
            serde_yaml::to_value(serde_json::json!({
                "schema": 1,
                "provider": "microsoft",
                "calendar_event_id": "event-1"
            }))
            .unwrap(),
            &["Gabriel".into(), "Maria".into()],
            "2026-08-05T12:05:00Z",
        )
        .unwrap();

        let markdown = fs::read_to_string(dir.path().join(MEETING_FILE)).unwrap();
        assert!(markdown.contains("calendar_event_id: event-1"));
        assert!(markdown.contains("- Gabriel"));
        assert!(markdown.contains("- Maria"));
        assert!(markdown.contains("- outlook"));
        assert!(markdown.contains("Texto do usuário."));
        let note = read_note_document(dir.path()).unwrap();
        assert_eq!(
            note.external_meeting
                .as_ref()
                .and_then(|value| value.get("calendar_event_id"))
                .and_then(serde_json::Value::as_str),
            Some("event-1")
        );
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
