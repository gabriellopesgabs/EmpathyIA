//! Provider-neutral meeting-agent state machine and portable audit trail.
//! Provider adapters may append events only through `append_event`, which
//! rejects impossible transitions such as transcribing before consent.
use crate::database::repositories::meeting::MeetingsRepository;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::State;

const SCHEMA: u32 = 1;
const AUDIT_FILE: &str = "agent-audit.md";
static AUDIT_LOCK: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MeetingAgentProvider {
    MicrosoftTeams,
    Zoom,
    GoogleMeet,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MeetingAgentState {
    Planned,
    Invited,
    Waiting,
    Joined,
    ConsentRequested,
    ConsentGranted,
    ConsentDenied,
    Transcribing,
    Paused,
    Leaving,
    Left,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeetingAgentEvent {
    pub schema: u32,
    pub event_id: String,
    pub session_id: String,
    pub meeting_id: String,
    pub provider: MeetingAgentProvider,
    pub state: MeetingAgentState,
    pub occurred_at: String,
    pub actor: String,
    pub details: Option<String>,
    pub service_event_id: Option<String>,
    pub recording_status_confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingAgentAudit {
    pub meeting_id: String,
    pub current_state: Option<MeetingAgentState>,
    pub events: Vec<MeetingAgentEvent>,
    pub path: String,
}

fn allowed_transition(previous: Option<MeetingAgentState>, next: MeetingAgentState) -> bool {
    use MeetingAgentState::*;
    matches!(
        (previous, next),
        (None, Planned)
            | (Some(Planned), Invited | Error | Left)
            | (Some(Invited), Waiting | Joined | Error | Leaving | Left)
            | (Some(Waiting), Joined | Error | Leaving | Left)
            | (Some(Joined), ConsentRequested | Error | Leaving | Left)
            | (
                Some(ConsentRequested),
                ConsentGranted | ConsentDenied | Error | Leaving | Left
            )
            | (
                Some(ConsentGranted),
                Transcribing | Paused | Error | Leaving | Left
            )
            | (Some(ConsentDenied), Leaving | Left)
            | (Some(Transcribing), Paused | Error | Leaving | Left)
            | (Some(Paused), Transcribing | Error | Leaving | Left)
            | (Some(Error), Leaving | Left)
            | (Some(Leaving), Left | Error)
    )
}

fn validate_event(event: &MeetingAgentEvent) -> Result<(), String> {
    if event.schema != SCHEMA
        || event.event_id.trim().is_empty()
        || event.session_id.trim().is_empty()
        || event.meeting_id.trim().is_empty()
        || event.actor.trim().is_empty()
        || event.event_id.len() > 200
        || event.session_id.len() > 200
        || event.meeting_id.len() > 200
        || event.actor.len() > 200
        || event
            .details
            .as_deref()
            .is_some_and(|value| value.len() > 4_000)
    {
        return Err("Evento de auditoria do agente inválido".into());
    }
    if event.state == MeetingAgentState::Transcribing && !event.recording_status_confirmed {
        return Err("A transcrição exige confirmação do estado de gravação do provedor".into());
    }
    chrono::DateTime::parse_from_rfc3339(&event.occurred_at)
        .map_err(|_| "Data do evento do agente inválida".to_string())?;
    Ok(())
}

fn audit_path(folder: &Path) -> PathBuf {
    folder.join(AUDIT_FILE)
}

fn parse_events(path: &Path) -> Result<Vec<MeetingAgentEvent>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut events = Vec::new();
    let mut remaining = content.as_str();
    const OPEN: &str = "<!-- empathy-agent-event\n";
    const CLOSE: &str = "\n-->";
    while let Some((_, after_open)) = remaining.split_once(OPEN) {
        let Some((json, after_close)) = after_open.split_once(CLOSE) else {
            return Err("Auditoria do agente está incompleta".into());
        };
        let event: MeetingAgentEvent = serde_json::from_str(json)
            .map_err(|error| format!("Evento do agente inválido: {error}"))?;
        validate_event(&event)?;
        events.push(event);
        remaining = after_close;
    }
    Ok(events)
}

fn write_audit(path: &Path, meeting_id: &str, events: &[MeetingAgentEvent]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Pasta de auditoria inválida".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut markdown = format!("---\nempathy_schema: 2\ntype: agent-audit\nmeeting_id: {}\ntitle: Auditoria do Agente Empathy\n---\n\n# Auditoria do Agente Empathy\n\nRegistro portátil e somente acrescentável dos estados visíveis do agente.\n", serde_json::to_string(meeting_id).map_err(|error| error.to_string())?);
    for event in events {
        let json = serde_json::to_string(event).map_err(|error| error.to_string())?;
        markdown.push_str(&format!(
            "\n<!-- empathy-agent-event\n{json}\n-->\n## {} — {:?}\n\n{}\n",
            event.occurred_at,
            event.state,
            event
                .details
                .as_deref()
                .unwrap_or("Sem detalhes adicionais.")
        ));
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(markdown.as_bytes())
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

pub async fn append_event(
    folder: &Path,
    event: MeetingAgentEvent,
) -> Result<Vec<MeetingAgentEvent>, String> {
    let _guard = AUDIT_LOCK.lock().await;
    validate_event(&event)?;
    let path = audit_path(folder);
    let mut events = parse_events(&path)?;
    if events.iter().any(|existing| {
        existing.event_id == event.event_id
            || event.service_event_id.is_some()
                && existing.service_event_id == event.service_event_id
    }) {
        return Ok(events);
    }
    if events.last().is_some_and(|last| {
        last.session_id != event.session_id && last.state != MeetingAgentState::Left
    }) {
        return Err("A sessão anterior do agente ainda não foi encerrada".into());
    }
    let previous = events
        .iter()
        .rev()
        .find(|existing| existing.session_id == event.session_id)
        .map(|existing| existing.state);
    if !allowed_transition(previous, event.state) {
        return Err(format!(
            "Transição inválida do agente: {previous:?} → {:?}",
            event.state
        ));
    }
    events.push(event);
    write_audit(
        &path,
        events
            .last()
            .map(|event| event.meeting_id.as_str())
            .unwrap_or_default(),
        &events,
    )?;
    Ok(events)
}

async fn meeting_folder(state: &State<'_, AppState>, meeting_id: &str) -> Result<PathBuf, String> {
    let meeting = MeetingsRepository::get_meeting_metadata(state.db_manager.pool(), meeting_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Nota não encontrada".to_string())?;
    meeting
        .folder_path
        .map(PathBuf::from)
        .ok_or_else(|| "Nota sem pasta Markdown".to_string())
}

#[tauri::command]
pub async fn api_get_meeting_agent_audit(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<MeetingAgentAudit, String> {
    let folder = meeting_folder(&state, &meeting_id).await?;
    let path = audit_path(&folder);
    let events = parse_events(&path)?;
    Ok(MeetingAgentAudit {
        meeting_id,
        current_state: events.last().map(|event| event.state),
        events,
        path: path.to_string_lossy().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(session: &str, state: MeetingAgentState, recording: bool) -> MeetingAgentEvent {
        MeetingAgentEvent {
            schema: 1,
            event_id: uuid::Uuid::new_v4().to_string(),
            session_id: session.into(),
            meeting_id: "meeting-1".into(),
            provider: MeetingAgentProvider::MicrosoftTeams,
            state,
            occurred_at: chrono::Utc::now().to_rfc3339(),
            actor: "Empathy AI — gravação e transcrição".into(),
            details: None,
            service_event_id: None,
            recording_status_confirmed: recording,
        }
    }

    #[test]
    fn state_machine_never_transcribes_before_consent() {
        assert!(!allowed_transition(
            Some(MeetingAgentState::Joined),
            MeetingAgentState::Transcribing
        ));
        assert!(allowed_transition(
            Some(MeetingAgentState::ConsentGranted),
            MeetingAgentState::Transcribing
        ));
        assert!(validate_event(&event("s1", MeetingAgentState::Transcribing, false)).is_err());
    }

    #[tokio::test]
    async fn audit_is_append_only_idempotent_and_portable() {
        let directory = tempfile::tempdir().unwrap();
        let planned = event("s1", MeetingAgentState::Planned, false);
        let duplicate = planned.clone();
        assert_eq!(
            append_event(directory.path(), planned).await.unwrap().len(),
            1
        );
        assert_eq!(
            append_event(directory.path(), duplicate)
                .await
                .unwrap()
                .len(),
            1
        );
        let invited = event("s1", MeetingAgentState::Invited, false);
        assert_eq!(
            append_event(directory.path(), invited).await.unwrap().len(),
            2
        );
        assert!(fs::read_to_string(directory.path().join(AUDIT_FILE))
            .unwrap()
            .contains("empathy-agent-event"));
    }

    #[tokio::test]
    async fn impossible_transition_does_not_modify_audit() {
        let directory = tempfile::tempdir().unwrap();
        append_event(
            directory.path(),
            event("s1", MeetingAgentState::Planned, false),
        )
        .await
        .unwrap();
        let before = fs::read_to_string(directory.path().join(AUDIT_FILE)).unwrap();
        assert!(append_event(
            directory.path(),
            event("s1", MeetingAgentState::Transcribing, true)
        )
        .await
        .is_err());
        assert_eq!(
            fs::read_to_string(directory.path().join(AUDIT_FILE)).unwrap(),
            before
        );
    }
}
