//! Provider-neutral meeting-agent state machine and portable audit trail.
//! Provider adapters may append events only through `append_event`, which
//! rejects impossible transitions such as transcribing before consent.
use crate::database::repositories::meeting::MeetingsRepository;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime, State};

const SCHEMA: u32 = 1;
const AUDIT_FILE: &str = "agent-audit.md";
const SERVICE_CONFIG_FILE: &str = "agent-service.json";
const KEYRING_SERVICE: &str = "ai.empathy.desktop";
const KEYRING_ACCOUNT: &str = "meeting-agent-service";
const VISIBLE_AGENT_NAME: &str = "Empathy AI — gravação e transcrição";
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentServiceConfig {
    schema: u32,
    endpoint: String,
    paired_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentServiceReadiness {
    pub configured: bool,
    pub endpoint: Option<String>,
    pub reachable: bool,
    pub ready: bool,
    pub missing: Vec<String>,
    pub service_error: Option<String>,
    pub visible_name: &'static str,
}

#[derive(Debug, Deserialize)]
struct ServiceReadinessResponse {
    ready: bool,
    #[serde(default)]
    missing: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CreateAgentSessionRequest<'a> {
    schema: u32,
    session_id: &'a str,
    meeting_id: &'a str,
    provider: &'static str,
    join_url: &'a str,
    visible_name: &'static str,
    requester_confirmed_visible_disclosure: bool,
}

#[derive(Debug, Deserialize)]
struct AgentSessionResponse {
    session_id: String,
    #[serde(default)]
    events: Vec<MeetingAgentEvent>,
}

#[derive(Debug, Deserialize)]
struct ExternalMeetingForAgent {
    meeting_provider: Option<String>,
    join_url: Option<String>,
}

fn service_config_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("integrations").join(SERVICE_CONFIG_FILE))
        .map_err(|error| error.to_string())
}

fn validate_service_endpoint(value: &str) -> Result<url::Url, String> {
    let mut endpoint =
        url::Url::parse(value).map_err(|_| "Endpoint do serviço inválido".to_string())?;
    let local_development = cfg!(debug_assertions)
        && endpoint.scheme() == "http"
        && matches!(endpoint.host_str(), Some("127.0.0.1" | "localhost"));
    if (endpoint.scheme() != "https" && !local_development)
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err("Use um endpoint HTTPS sem credenciais, query ou fragmento".into());
    }
    let normalized_path = endpoint.path().trim_end_matches('/').to_string();
    endpoint.set_path(&normalized_path);
    Ok(endpoint)
}

fn read_service_config<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<AgentServiceConfig>, String> {
    let path = service_config_path(app)?;
    if !path.exists() {
        return Ok(None);
    }
    let config: AgentServiceConfig =
        serde_json::from_str(&fs::read_to_string(path).map_err(|error| error.to_string())?)
            .map_err(|error| format!("Configuração do serviço do agente inválida: {error}"))?;
    if config.schema != SCHEMA {
        return Err("Schema do serviço do agente não suportado".into());
    }
    validate_service_endpoint(&config.endpoint)?;
    Ok(Some(config))
}

fn write_service_config<R: Runtime>(
    app: &AppHandle<R>,
    config: &AgentServiceConfig,
) -> Result<(), String> {
    let path = service_config_path(app)?;
    let json = format!(
        "{}\n",
        serde_json::to_string_pretty(config).map_err(|error| error.to_string())?
    );
    let parent = path
        .parent()
        .ok_or_else(|| "Pasta de integrações inválida".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(json.as_bytes())
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

fn service_token() -> Result<Option<String>, String> {
    let entry =
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(|error| error.to_string())?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!(
            "Não foi possível ler a credencial do serviço: {error}"
        )),
    }
}

fn save_service_token(token: &str) -> Result<(), String> {
    if token.trim().len() < 24 || token.len() > 4096 {
        return Err("Token de pareamento inválido".into());
    }
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|error| error.to_string())?
        .set_password(token)
        .map_err(|error| format!("Não foi possível salvar a credencial do serviço: {error}"))
}

fn delete_service_token() -> Result<(), String> {
    let entry =
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(|error| error.to_string())?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Não foi possível remover a credencial do serviço: {error}"
        )),
    }
}

fn service_url(config: &AgentServiceConfig, path: &str) -> Result<url::Url, String> {
    let mut endpoint = validate_service_endpoint(&config.endpoint)?;
    let base_path = endpoint.path().trim_end_matches('/');
    endpoint.set_path(&format!("{base_path}{path}"));
    Ok(endpoint)
}

fn validate_service_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 100
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(format!("Identificador de {label} inválido"));
    }
    Ok(())
}

async fn fetch_service_readiness(
    config: &AgentServiceConfig,
    token: &str,
) -> Result<ServiceReadinessResponse, String> {
    let response = reqwest::Client::new()
        .get(service_url(config, "/v1/readiness")?)
        .bearer_auth(token)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|error| format!("Serviço do agente indisponível: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Serviço do agente recusou a conexão: HTTP {status}"
        ));
    }
    response
        .json()
        .await
        .map_err(|error| format!("Resposta do serviço do agente inválida: {error}"))
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
    validate_event_sequence(&events)?;
    Ok(events)
}

fn validate_event_sequence(events: &[MeetingAgentEvent]) -> Result<(), String> {
    let mut previous: Option<&MeetingAgentEvent> = None;
    let mut event_ids = std::collections::BTreeSet::new();
    let mut service_ids = std::collections::BTreeSet::new();
    for event in events {
        if !event_ids.insert(event.event_id.as_str())
            || event
                .service_event_id
                .as_deref()
                .is_some_and(|id| !service_ids.insert(id))
        {
            return Err("Auditoria do agente contém eventos duplicados".into());
        }
        let previous_state = match previous {
            Some(last) if last.session_id == event.session_id => Some(last.state),
            Some(last) if last.state == MeetingAgentState::Left => None,
            Some(_) => return Err("Auditoria contém sessões simultâneas".into()),
            None => None,
        };
        if !allowed_transition(previous_state, event.state) {
            return Err(format!(
                "Auditoria contém transição inválida: {previous_state:?} → {:?}",
                event.state
            ));
        }
        previous = Some(event);
    }
    Ok(())
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

#[tauri::command]
pub async fn api_get_agent_service_readiness<R: Runtime>(
    app: AppHandle<R>,
) -> Result<AgentServiceReadiness, String> {
    let Some(config) = read_service_config(&app)? else {
        return Ok(AgentServiceReadiness {
            configured: false,
            endpoint: None,
            reachable: false,
            ready: false,
            missing: vec![
                "endpoint".into(),
                "pairing-token".into(),
                "tenant-admin-consent".into(),
                "windows-media-service".into(),
            ],
            service_error: None,
            visible_name: VISIBLE_AGENT_NAME,
        });
    };
    let Some(token) = service_token()? else {
        return Ok(AgentServiceReadiness {
            configured: true,
            endpoint: Some(config.endpoint),
            reachable: false,
            ready: false,
            missing: vec!["pairing-token".into()],
            service_error: Some("Credencial de pareamento ausente".into()),
            visible_name: VISIBLE_AGENT_NAME,
        });
    };
    match fetch_service_readiness(&config, &token).await {
        Ok(status) => Ok(AgentServiceReadiness {
            configured: true,
            endpoint: Some(config.endpoint),
            reachable: true,
            ready: status.ready,
            missing: status.missing,
            service_error: None,
            visible_name: VISIBLE_AGENT_NAME,
        }),
        Err(error) => Ok(AgentServiceReadiness {
            configured: true,
            endpoint: Some(config.endpoint),
            reachable: false,
            ready: false,
            missing: Vec::new(),
            service_error: Some(error),
            visible_name: VISIBLE_AGENT_NAME,
        }),
    }
}

#[tauri::command]
pub async fn api_pair_agent_service<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    endpoint: String,
    pairing_token: String,
) -> Result<AgentServiceReadiness, String> {
    ensure_no_active_agent_sessions(&state).await?;
    let endpoint = validate_service_endpoint(&endpoint)?.to_string();
    if pairing_token.trim().len() < 24 || pairing_token.len() > 4096 {
        return Err("Token de pareamento inválido".into());
    }
    let now = chrono::Utc::now().to_rfc3339();
    let config = AgentServiceConfig {
        schema: SCHEMA,
        endpoint,
        paired_at: now.clone(),
        updated_at: now,
    };
    // Authenticate before persisting anything locally.
    let status = fetch_service_readiness(&config, pairing_token.trim()).await?;
    let previous_config = read_service_config(&app)?;
    let previous_token = service_token()?;
    save_service_token(pairing_token.trim())?;
    if let Err(error) = write_service_config(&app, &config) {
        if let Some(token) = previous_token.as_deref() {
            let _ = save_service_token(token);
        } else {
            let _ = delete_service_token();
        }
        return Err(error);
    }
    if let Err(error) = crate::integrations::set_meeting_agent_feature_enabled(&app, status.ready) {
        if let Some(previous) = previous_config.as_ref() {
            let _ = write_service_config(&app, previous);
        } else {
            let _ = fs::remove_file(service_config_path(&app)?);
        }
        if let Some(token) = previous_token.as_deref() {
            let _ = save_service_token(token);
        } else {
            let _ = delete_service_token();
        }
        return Err(error);
    }
    Ok(AgentServiceReadiness {
        configured: true,
        endpoint: Some(config.endpoint),
        reachable: true,
        ready: status.ready,
        missing: status.missing,
        service_error: None,
        visible_name: VISIBLE_AGENT_NAME,
    })
}

async fn ensure_no_active_agent_sessions(state: &State<'_, AppState>) -> Result<(), String> {
    let folders: Vec<(Option<String>,)> =
        sqlx::query_as("SELECT folder_path FROM meetings WHERE folder_path IS NOT NULL")
            .fetch_all(state.db_manager.pool())
            .await
            .map_err(|error| error.to_string())?;
    for (folder,) in folders {
        let Some(folder) = folder else { continue };
        let path = audit_path(Path::new(&folder));
        let events = parse_events(&path)?;
        if events
            .last()
            .is_some_and(|event| event.state != MeetingAgentState::Left)
        {
            return Err(
                "Encerre todas as sessões do agente antes de trocar ou desconectar o serviço"
                    .into(),
            );
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn api_disconnect_agent_service<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    ensure_no_active_agent_sessions(&state).await?;
    delete_service_token()?;
    let path = service_config_path(&app)?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    crate::integrations::set_meeting_agent_feature_enabled(&app, false)
}

async fn external_meeting_for_agent(
    state: &State<'_, AppState>,
    meeting_id: &str,
) -> Result<(PathBuf, ExternalMeetingForAgent), String> {
    let folder = meeting_folder(state, meeting_id).await?;
    let note =
        crate::meeting_files::read_note_document(&folder).map_err(|error| error.to_string())?;
    let external = note
        .external_meeting
        .ok_or_else(|| "A nota não está ligada a uma reunião externa".to_string())?;
    let meeting: ExternalMeetingForAgent = serde_json::from_value(external)
        .map_err(|error| format!("Metadados da reunião inválidos: {error}"))?;
    Ok((folder, meeting))
}

fn validate_teams_join_url(value: &str) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|_| "Link do Teams inválido".to_string())?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https"
        || !(host == "teams.microsoft.com" || host.ends_with(".teams.microsoft.com"))
    {
        return Err(
            "O agente Teams aceita somente links HTTPS do domínio teams.microsoft.com".into(),
        );
    }
    Ok(())
}

fn ensure_service_event(
    event: &MeetingAgentEvent,
    meeting_id: &str,
    session_id: &str,
) -> Result<(), String> {
    if event.meeting_id != meeting_id
        || event.session_id != session_id
        || event.provider != MeetingAgentProvider::MicrosoftTeams
    {
        return Err("O serviço retornou um evento para outra sessão ou reunião".into());
    }
    Ok(())
}

async fn append_service_events(
    folder: &Path,
    meeting_id: &str,
    session_id: &str,
    events: Vec<MeetingAgentEvent>,
) -> Result<(), String> {
    for event in events {
        ensure_service_event(&event, meeting_id, session_id)?;
        append_event(folder, event).await?;
    }
    Ok(())
}

fn local_event(
    meeting_id: &str,
    session_id: &str,
    state: MeetingAgentState,
    details: Option<String>,
) -> MeetingAgentEvent {
    MeetingAgentEvent {
        schema: SCHEMA,
        event_id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.into(),
        meeting_id: meeting_id.into(),
        provider: MeetingAgentProvider::MicrosoftTeams,
        state,
        occurred_at: chrono::Utc::now().to_rfc3339(),
        actor: VISIBLE_AGENT_NAME.into(),
        details,
        service_event_id: None,
        recording_status_confirmed: false,
    }
}

#[tauri::command]
pub async fn api_request_teams_agent<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    meeting_id: String,
    requester_confirmed_visible_disclosure: bool,
) -> Result<MeetingAgentAudit, String> {
    if !requester_confirmed_visible_disclosure {
        return Err(
            "Confirme que o agente será um participante visível antes de convidá-lo".into(),
        );
    }
    if !crate::integrations::meeting_agent_feature_enabled(&app)? {
        return Err("O serviço do Agente Empathy ainda não está pronto".into());
    }
    let config = read_service_config(&app)?
        .ok_or_else(|| "Serviço do agente não configurado".to_string())?;
    let token = service_token()?.ok_or_else(|| "Credencial do serviço ausente".to_string())?;
    let readiness = fetch_service_readiness(&config, &token).await?;
    if !readiness.ready {
        crate::integrations::set_meeting_agent_feature_enabled(&app, false)?;
        return Err(format!(
            "O serviço do agente não está pronto: {}",
            readiness.missing.join(", ")
        ));
    }
    let (folder, external) = external_meeting_for_agent(&state, &meeting_id).await?;
    if external.meeting_provider.as_deref() != Some("microsoft-teams") {
        return Err("Esta nota não está ligada a uma reunião do Teams".into());
    }
    let join_url = external
        .join_url
        .as_deref()
        .ok_or_else(|| "A reunião não possui link de entrada".to_string())?;
    validate_teams_join_url(join_url)?;
    let session_id = uuid::Uuid::new_v4().to_string();
    append_event(&folder, local_event(&meeting_id, &session_id, MeetingAgentState::Planned, Some("O usuário solicitou um agente visível; consentimento dos participantes ainda não foi obtido.".into()))).await?;
    let request = CreateAgentSessionRequest {
        schema: SCHEMA,
        session_id: &session_id,
        meeting_id: &meeting_id,
        provider: "microsoft-teams",
        join_url,
        visible_name: VISIBLE_AGENT_NAME,
        requester_confirmed_visible_disclosure: true,
    };
    let response = reqwest::Client::new()
        .post(service_url(&config, "/v1/sessions")?)
        .bearer_auth(&token)
        .json(&request)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await;
    let result = match response {
        Ok(response) if response.status().is_success() => response
            .json::<AgentSessionResponse>()
            .await
            .map_err(|error| format!("Resposta de sessão inválida: {error}")),
        Ok(response) => Err(format!(
            "O serviço recusou o agente: HTTP {}",
            response.status()
        )),
        Err(error) => Err(format!(
            "Não foi possível contatar o serviço do agente: {error}"
        )),
    };
    match result {
        Ok(session) => {
            let validation = if session.session_id != session_id {
                Err("O serviço retornou outra sessão".to_string())
            } else if session.events.is_empty() {
                Err("O serviço não confirmou o convite do agente".to_string())
            } else {
                append_service_events(&folder, &meeting_id, &session_id, session.events).await
            };
            if let Err(error) = validation {
                append_event(
                    &folder,
                    local_event(
                        &meeting_id,
                        &session_id,
                        MeetingAgentState::Error,
                        Some(error.clone()),
                    ),
                )
                .await?;
                return Err(error);
            }
        }
        Err(error) => {
            append_event(
                &folder,
                local_event(
                    &meeting_id,
                    &session_id,
                    MeetingAgentState::Error,
                    Some(error.clone()),
                ),
            )
            .await?;
            return Err(error);
        }
    }
    api_get_meeting_agent_audit(state, meeting_id).await
}

#[tauri::command]
pub async fn api_refresh_teams_agent<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<MeetingAgentAudit, String> {
    let audit = api_get_meeting_agent_audit(state.clone(), meeting_id.clone()).await?;
    let session_id = audit
        .events
        .last()
        .map(|event| event.session_id.clone())
        .ok_or_else(|| "Nenhuma sessão do agente foi iniciada".to_string())?;
    validate_service_identifier(&session_id, "sessão")?;
    let config = read_service_config(&app)?
        .ok_or_else(|| "Serviço do agente não configurado".to_string())?;
    let token = service_token()?.ok_or_else(|| "Credencial do serviço ausente".to_string())?;
    let response = reqwest::Client::new()
        .get(service_url(&config, &format!("/v1/sessions/{session_id}"))?)
        .bearer_auth(token)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Não foi possível atualizar o agente: HTTP {}",
            response.status()
        ));
    }
    let session: AgentSessionResponse = response.json().await.map_err(|error| error.to_string())?;
    if session.session_id != session_id {
        return Err("O serviço retornou outra sessão".into());
    }
    let folder = meeting_folder(&state, &meeting_id).await?;
    append_service_events(&folder, &meeting_id, &session_id, session.events).await?;
    api_get_meeting_agent_audit(state, meeting_id).await
}

#[tauri::command]
pub async fn api_leave_teams_agent<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<MeetingAgentAudit, String> {
    let audit = api_get_meeting_agent_audit(state.clone(), meeting_id.clone()).await?;
    let session_id = audit
        .events
        .last()
        .map(|event| event.session_id.clone())
        .ok_or_else(|| "Nenhuma sessão do agente foi iniciada".to_string())?;
    validate_service_identifier(&session_id, "sessão")?;
    if matches!(audit.current_state, Some(MeetingAgentState::Left)) {
        return Ok(audit);
    }
    let config = read_service_config(&app)?
        .ok_or_else(|| "Serviço do agente não configurado".to_string())?;
    let token = service_token()?.ok_or_else(|| "Credencial do serviço ausente".to_string())?;
    let response = reqwest::Client::new()
        .post(service_url(
            &config,
            &format!("/v1/sessions/{session_id}/leave"),
        )?)
        .bearer_auth(token)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "O serviço não confirmou a saída do agente: HTTP {}",
            response.status()
        ));
    }
    let session: AgentSessionResponse = response.json().await.map_err(|error| error.to_string())?;
    if session.session_id != session_id {
        return Err("O serviço retornou outra sessão".into());
    }
    let folder = meeting_folder(&state, &meeting_id).await?;
    append_service_events(&folder, &meeting_id, &session_id, session.events).await?;
    api_get_meeting_agent_audit(state, meeting_id).await
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

    #[test]
    fn service_endpoints_are_https_and_cannot_embed_credentials() {
        assert!(validate_service_endpoint("https://agent.empathy.ai").is_ok());
        assert!(validate_service_endpoint("http://agent.empathy.ai").is_err());
        assert!(validate_service_endpoint("https://token@agent.empathy.ai").is_err());
        assert!(validate_service_endpoint("https://agent.empathy.ai?token=secret").is_err());
        assert!(
            validate_service_identifier("550e8400-e29b-41d4-a716-446655440000", "sessão").is_ok()
        );
        assert!(validate_service_identifier("../session", "sessão").is_err());
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

    #[test]
    fn tampered_audit_sequence_is_rejected_on_read() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(AUDIT_FILE);
        write_audit(
            &path,
            "meeting-1",
            &[event("s1", MeetingAgentState::Transcribing, true)],
        )
        .unwrap();
        assert!(parse_events(&path).is_err());
    }
}
