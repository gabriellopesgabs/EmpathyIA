//! Portable, user-controlled participant memory.
//!
//! Person records live beside the user's Markdown workspace. Calendar data is
//! only promoted into memory after an explicit confirmation, and hypotheses
//! remain visibly separated from confirmed context.
use crate::audio::recording_preferences::load_recording_preferences;
use crate::database::repositories::meeting::MeetingsRepository;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Runtime, State};

const SCHEMA: u32 = 1;
const PEOPLE_DIRECTORY: &str = "People";
static MEMORY_LOCK: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParticipantSourceReceipt {
    pub provider: String,
    pub source_kind: String,
    pub source_id: String,
    pub note_id: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantMemory {
    pub schema: u32,
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub emails: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub organization: Option<String>,
    pub role: Option<String>,
    #[serde(default)]
    pub confirmed_fields: Vec<String>,
    #[serde(default)]
    pub source_receipts: Vec<ParticipantSourceReceipt>,
    pub created_at: String,
    pub updated_at: String,
    pub merged_into: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub hypotheses: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct ParticipantMemoryUpdate {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub emails: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub organization: Option<String>,
    pub role: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub hypotheses: String,
    pub expected_updated_at: String,
}

#[derive(Debug, Deserialize)]
struct ParticipantFrontmatter {
    empathy_schema: u32,
    #[serde(rename = "type")]
    document_type: String,
    id: String,
    title: String,
    #[serde(default)]
    emails: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
    organization: Option<String>,
    role: Option<String>,
    #[serde(default)]
    confirmed_fields: Vec<String>,
    #[serde(default)]
    source_receipts: Vec<ParticipantSourceReceipt>,
    created_at: String,
    updated_at: String,
    merged_into: Option<String>,
}

#[derive(Serialize)]
struct ParticipantFrontmatterRef<'a> {
    empathy_schema: u32,
    #[serde(rename = "type")]
    document_type: &'static str,
    id: &'a str,
    title: &'a str,
    emails: &'a [String],
    aliases: &'a [String],
    organization: &'a Option<String>,
    role: &'a Option<String>,
    confirmed_fields: &'a [String],
    source_receipts: &'a [ParticipantSourceReceipt],
    created_at: &'a str,
    updated_at: &'a str,
    merged_into: &'a Option<String>,
    status: &'static str,
    tags: [&'static str; 1],
}

#[derive(Debug, Deserialize)]
struct ExternalMeetingParticipant {
    display_name: String,
    email: String,
}

#[derive(Debug, Deserialize)]
struct ExternalMeetingMetadata {
    provider: String,
    calendar_event_id: String,
    organizer: Option<ExternalMeetingParticipant>,
    #[serde(default)]
    attendees: Vec<ExternalMeetingParticipant>,
}

async fn people_directory<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let preferences = load_recording_preferences(app)
        .await
        .map_err(|error| format!("Não foi possível localizar o workspace: {error}"))?;
    let directory = preferences.save_folder.join(PEOPLE_DIRECTORY);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Não foi possível criar a pasta People: {error}"))?;
    Ok(directory)
}

fn validate_id(value: &str) -> Result<(), String> {
    if !value.starts_with("person-")
        || value.len() > 80
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("Identificador de participante inválido".into());
    }
    Ok(())
}

fn normalize_email(value: &str) -> Result<String, String> {
    let email = value.trim().to_ascii_lowercase();
    if email.is_empty()
        || email.len() > 254
        || email.matches('@').count() != 1
        || email.starts_with('@')
        || email.ends_with('@')
        || !email
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "@._+-".contains(character))
    {
        return Err(format!("E-mail inválido: {value}"));
    }
    Ok(email)
}

fn normalize_text_list(values: Vec<String>) -> Vec<String> {
    let mut values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value.len() <= 200)
        .collect::<Vec<_>>();
    values.sort_by_key(|value| value.to_lowercase());
    values.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    values
}

fn memory_path(directory: &Path, id: &str) -> Result<PathBuf, String> {
    validate_id(id)?;
    Ok(directory.join(format!("{id}.md")))
}

fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "Pasta inválida".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(content.as_bytes())
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

fn write_memory(path: &Path, memory: &ParticipantMemory) -> Result<(), String> {
    validate_memory(memory)?;
    let frontmatter = ParticipantFrontmatterRef {
        empathy_schema: 2,
        document_type: "person",
        id: &memory.id,
        title: &memory.display_name,
        emails: &memory.emails,
        aliases: &memory.aliases,
        organization: &memory.organization,
        role: &memory.role,
        confirmed_fields: &memory.confirmed_fields,
        source_receipts: &memory.source_receipts,
        created_at: &memory.created_at,
        updated_at: &memory.updated_at,
        merged_into: &memory.merged_into,
        status: if memory.merged_into.is_some() {
            "merged"
        } else {
            "active"
        },
        tags: ["person"],
    };
    let yaml = serde_yaml::to_string(&frontmatter).map_err(|error| error.to_string())?;
    let body = format!(
        "# {}\n\n## Contexto confirmado\n\n{}\n\n## Hipóteses a revisar\n\n{}\n",
        memory.display_name,
        memory.notes.trim(),
        memory.hypotheses.trim()
    );
    atomic_write(path, &format!("---\n{}---\n\n{}", yaml, body))
}

fn section(body: &str, heading: &str, next: Option<&str>) -> String {
    let marker = format!("## {heading}");
    let Some(rest) = body.split_once(&marker).map(|(_, rest)| rest) else {
        return String::new();
    };
    let value = next
        .and_then(|next_heading| {
            rest.split_once(&format!("## {next_heading}"))
                .map(|(value, _)| value)
        })
        .unwrap_or(rest);
    value.trim().to_string()
}

fn read_memory(path: &Path) -> Result<ParticipantMemory, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let rest = content
        .strip_prefix("---\n")
        .ok_or_else(|| "Memória sem frontmatter".to_string())?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| "Frontmatter incompleto".to_string())?;
    let frontmatter: ParticipantFrontmatter =
        serde_yaml::from_str(&rest[..end]).map_err(|error| error.to_string())?;
    if frontmatter.empathy_schema != 2 || frontmatter.document_type != "person" {
        return Err("Arquivo não é uma memória de participante compatível".into());
    }
    let body = rest
        .get(end + 4..)
        .unwrap_or_default()
        .trim_start_matches('\n');
    let memory = ParticipantMemory {
        schema: SCHEMA,
        id: frontmatter.id,
        display_name: frontmatter.title,
        emails: frontmatter.emails,
        aliases: frontmatter.aliases,
        organization: frontmatter.organization,
        role: frontmatter.role,
        confirmed_fields: frontmatter.confirmed_fields,
        source_receipts: frontmatter.source_receipts,
        created_at: frontmatter.created_at,
        updated_at: frontmatter.updated_at,
        merged_into: frontmatter.merged_into,
        notes: section(body, "Contexto confirmado", Some("Hipóteses a revisar")),
        hypotheses: section(body, "Hipóteses a revisar", None),
        path: path.to_string_lossy().to_string(),
    };
    validate_memory(&memory)?;
    Ok(memory)
}

fn validate_memory(memory: &ParticipantMemory) -> Result<(), String> {
    validate_id(&memory.id)?;
    if memory.schema != SCHEMA
        || memory.display_name.trim().is_empty()
        || memory.display_name.len() > 200
    {
        return Err("Memória de participante inválida".into());
    }
    if memory.notes.len() > 50_000 || memory.hypotheses.len() > 50_000 {
        return Err("O texto da memória excede 50 KB".into());
    }
    if memory.emails.len() > 20
        || memory.aliases.len() > 50
        || memory.source_receipts.len() > 500
        || memory
            .organization
            .as_deref()
            .is_some_and(|value| value.len() > 200)
        || memory
            .role
            .as_deref()
            .is_some_and(|value| value.len() > 200)
    {
        return Err("A memória de participante excede os limites permitidos".into());
    }
    for email in &memory.emails {
        normalize_email(email)?;
    }
    Ok(())
}

fn list_memories(directory: &Path) -> Result<Vec<ParticipantMemory>, String> {
    let mut memories = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("md") {
            memories.push(read_memory(&path)?);
        }
    }
    memories.sort_by_key(|memory| memory.display_name.to_lowercase());
    Ok(memories)
}

#[tauri::command]
pub async fn api_list_participant_memories<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<ParticipantMemory>, String> {
    list_memories(&people_directory(&app).await?)
}

#[tauri::command]
pub async fn api_confirm_note_participants<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    meeting_id: String,
    emails: Vec<String>,
) -> Result<Vec<ParticipantMemory>, String> {
    let _guard = MEMORY_LOCK.lock().await;
    if emails.is_empty() || emails.len() > 50 {
        return Err("Escolha entre 1 e 50 participantes".into());
    }
    let selected = emails
        .into_iter()
        .map(|email| normalize_email(&email))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let meeting = MeetingsRepository::get_meeting_metadata(state.db_manager.pool(), &meeting_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Nota não encontrada".to_string())?;
    let folder = meeting
        .folder_path
        .as_deref()
        .ok_or_else(|| "Nota sem pasta Markdown".to_string())?;
    let note = crate::meeting_files::read_note_document(Path::new(folder))
        .map_err(|error| error.to_string())?;
    let external: ExternalMeetingMetadata = serde_json::from_value(
        note.external_meeting
            .ok_or_else(|| "A nota não está ligada a um evento externo".to_string())?,
    )
    .map_err(|error| format!("Metadados do evento inválidos: {error}"))?;
    if external.provider != "microsoft" {
        return Err("Somente eventos Microsoft são aceitos nesta versão".into());
    }
    let mut candidates = external.attendees;
    if let Some(organizer) = external.organizer {
        candidates.push(organizer);
    }
    let mut by_email = candidates
        .into_iter()
        .filter_map(|candidate| {
            normalize_email(&candidate.email)
                .ok()
                .map(|email| (email, candidate.display_name))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if selected.iter().any(|email| !by_email.contains_key(email)) {
        return Err("Confirme somente participantes do evento ligado à nota".into());
    }
    let directory = people_directory(&app).await?;
    let mut memories = list_memories(&directory)?;
    let observed_at = chrono::Utc::now().to_rfc3339();
    for email in selected {
        let display_name = by_email.remove(&email).unwrap_or_else(|| email.clone());
        if let Some(memory) = memories.iter_mut().find(|memory| {
            memory
                .emails
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(&email))
        }) {
            if !memory.display_name.eq_ignore_ascii_case(&display_name)
                && !memory
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(&display_name))
            {
                memory.aliases.push(display_name);
            }
            let receipt = ParticipantSourceReceipt {
                provider: "microsoft".into(),
                source_kind: "calendar-event".into(),
                source_id: external.calendar_event_id.clone(),
                note_id: meeting_id.clone(),
                observed_at: observed_at.clone(),
            };
            if !memory.source_receipts.iter().any(|source| {
                source.provider == receipt.provider
                    && source.source_id == receipt.source_id
                    && source.note_id == receipt.note_id
            }) {
                memory.source_receipts.push(receipt);
            }
            memory.updated_at = observed_at.clone();
            memory.aliases = normalize_text_list(std::mem::take(&mut memory.aliases));
            let path = memory_path(&directory, &memory.id)?;
            write_memory(&path, memory)?;
            memory.path = path.to_string_lossy().to_string();
        } else {
            let id = format!("person-{}", uuid::Uuid::new_v4());
            let path = memory_path(&directory, &id)?;
            let mut memory = ParticipantMemory {
                schema: SCHEMA,
                id,
                display_name,
                emails: vec![email],
                aliases: Vec::new(),
                organization: None,
                role: None,
                confirmed_fields: vec!["display_name".into(), "emails".into()],
                source_receipts: vec![ParticipantSourceReceipt {
                    provider: "microsoft".into(),
                    source_kind: "calendar-event".into(),
                    source_id: external.calendar_event_id.clone(),
                    note_id: meeting_id.clone(),
                    observed_at: observed_at.clone(),
                }],
                created_at: observed_at.clone(),
                updated_at: observed_at.clone(),
                merged_into: None,
                notes: String::new(),
                hypotheses: String::new(),
                path: path.to_string_lossy().to_string(),
            };
            write_memory(&path, &memory)?;
            memory.path = path.to_string_lossy().to_string();
            memories.push(memory);
        }
    }
    let _ = crate::knowledge::api_reindex_knowledge(app, state).await?;
    list_memories(&directory)
}

#[tauri::command]
pub async fn api_save_participant_memory<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    update: ParticipantMemoryUpdate,
) -> Result<ParticipantMemory, String> {
    let _guard = MEMORY_LOCK.lock().await;
    let directory = people_directory(&app).await?;
    let path = memory_path(&directory, &update.id)?;
    let mut memory = read_memory(&path)?;
    if memory.updated_at != update.expected_updated_at {
        return Err(
            "PARTICIPANT_CONFLICT: A memória mudou no disco. Reabra antes de salvar.".into(),
        );
    }
    let display_name = update.display_name.trim();
    if display_name.is_empty() || display_name.len() > 200 {
        return Err("Nome inválido".into());
    }
    memory.display_name = display_name.into();
    memory.emails = update
        .emails
        .into_iter()
        .map(|email| normalize_email(&email))
        .collect::<Result<Vec<_>, _>>()?;
    memory.emails.sort();
    memory.emails.dedup();
    memory.aliases = normalize_text_list(update.aliases);
    memory.organization = update
        .organization
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    memory.role = update
        .role
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    memory.notes = update.notes;
    memory.hypotheses = update.hypotheses;
    memory.confirmed_fields = vec![
        "display_name".into(),
        "emails".into(),
        "aliases".into(),
        "organization".into(),
        "role".into(),
        "notes".into(),
    ];
    memory.updated_at = chrono::Utc::now().to_rfc3339();
    write_memory(&path, &memory)?;
    let _ = crate::knowledge::api_reindex_knowledge(app, state).await?;
    read_memory(&path)
}

#[tauri::command]
pub async fn api_delete_participant_memory<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    participant_id: String,
) -> Result<(), String> {
    let _guard = MEMORY_LOCK.lock().await;
    let directory = people_directory(&app).await?;
    let source = memory_path(&directory, &participant_id)?;
    if !source.exists() {
        return Err("Memória de participante não encontrada".into());
    }
    let trash = directory
        .parent()
        .ok_or_else(|| "Workspace inválido".to_string())?
        .join(".empathy-trash")
        .join("participants");
    fs::create_dir_all(&trash).map_err(|error| error.to_string())?;
    let suffix = chrono::Utc::now().timestamp_millis();
    fs::rename(&source, trash.join(format!("{participant_id}-{suffix}.md")))
        .map_err(|error| error.to_string())?;
    let _ = crate::knowledge::api_reindex_knowledge(app, state).await?;
    Ok(())
}

#[tauri::command]
pub async fn api_merge_participant_memories<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    source_id: String,
    target_id: String,
) -> Result<ParticipantMemory, String> {
    let _guard = MEMORY_LOCK.lock().await;
    if source_id == target_id {
        return Err("Escolha duas pessoas diferentes".into());
    }
    let directory = people_directory(&app).await?;
    let source_path = memory_path(&directory, &source_id)?;
    let target_path = memory_path(&directory, &target_id)?;
    let source = read_memory(&source_path)?;
    let mut target = read_memory(&target_path)?;
    let original_target = fs::read_to_string(&target_path).map_err(|error| error.to_string())?;
    target.emails.extend(source.emails);
    target.emails.sort();
    target.emails.dedup();
    target.aliases.extend(source.aliases);
    target.aliases.push(source.display_name);
    target.aliases = normalize_text_list(target.aliases);
    for receipt in source.source_receipts {
        if !target.source_receipts.contains(&receipt) {
            target.source_receipts.push(receipt);
        }
    }
    if !source.notes.trim().is_empty() {
        target.notes = if target.notes.trim().is_empty() {
            source.notes
        } else {
            format!("{}\n\n{}", target.notes.trim(), source.notes.trim())
        };
    }
    if !source.hypotheses.trim().is_empty() {
        target.hypotheses = if target.hypotheses.trim().is_empty() {
            source.hypotheses
        } else {
            format!(
                "{}\n\n{}",
                target.hypotheses.trim(),
                source.hypotheses.trim()
            )
        };
    }
    target.updated_at = chrono::Utc::now().to_rfc3339();
    write_memory(&target_path, &target)?;
    let trash = directory
        .parent()
        .ok_or_else(|| "Workspace inválido".to_string())?
        .join(".empathy-trash")
        .join("participants");
    fs::create_dir_all(&trash).map_err(|error| error.to_string())?;
    if let Err(error) = fs::rename(
        &source_path,
        trash.join(format!(
            "{}-merged-{}.md",
            source_id,
            chrono::Utc::now().timestamp_millis()
        )),
    ) {
        let _ = atomic_write(&target_path, &original_target);
        return Err(error.to_string());
    }
    let _ = crate::knowledge::api_reindex_knowledge(app, state).await?;
    read_memory(&target_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ParticipantMemory {
        ParticipantMemory {
            schema: 1,
            id: "person-123".into(),
            display_name: "Maria".into(),
            emails: vec!["maria@example.com".into()],
            aliases: vec!["Mari".into()],
            organization: Some("Example".into()),
            role: None,
            confirmed_fields: vec!["display_name".into()],
            source_receipts: Vec::new(),
            created_at: "2026-08-05T12:00:00Z".into(),
            updated_at: "2026-08-05T12:00:00Z".into(),
            merged_into: None,
            notes: "Prefere decisões explícitas.".into(),
            hypotheses: "Pode liderar a pauta; confirmar.".into(),
            path: String::new(),
        }
    }

    #[test]
    fn participant_memory_round_trip_keeps_confirmed_and_hypothesis_separate() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("person-123.md");
        write_memory(&path, &sample()).unwrap();
        let loaded = read_memory(&path).unwrap();
        assert_eq!(loaded.notes, "Prefere decisões explícitas.");
        assert_eq!(loaded.hypotheses, "Pode liderar a pauta; confirmar.");
        assert!(fs::read_to_string(path).unwrap().contains("type: person"));
    }

    #[test]
    fn identifiers_cannot_escape_people_directory() {
        assert!(memory_path(Path::new("/tmp/People"), "../person-1").is_err());
        assert!(memory_path(Path::new("/tmp/People"), "person-123").is_ok());
    }

    #[test]
    fn email_normalization_rejects_search_or_markup_payloads() {
        assert_eq!(
            normalize_email("Maria@Example.com").unwrap(),
            "maria@example.com"
        );
        assert!(normalize_email("maria@example.com OR from:boss").is_err());
        assert!(normalize_email("<maria@example.com>").is_err());
    }
}
