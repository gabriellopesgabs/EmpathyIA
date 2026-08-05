//! Provider-neutral contracts and fail-closed feature gates for meeting integrations.
//!
//! Tokens never belong in these files. OAuth credentials are stored in the operating
//! system credential vault; portable metadata and audit receipts remain file-first.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::PathBuf, process::Command, time::Duration};
use tauri::{AppHandle, Manager, Runtime};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::timeout,
};

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
    pub token_expires_at: Option<String>,
    pub connected_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MicrosoftAuthReadiness {
    pub configured: bool,
    pub tenant: String,
    pub requested_scopes: Vec<&'static str>,
    pub missing: Vec<&'static str>,
}

#[derive(Debug, Deserialize)]
struct MicrosoftTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MicrosoftGraphProfile {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    mail: Option<String>,
    #[serde(rename = "userPrincipalName")]
    user_principal_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutlookEventParticipant {
    pub display_name: String,
    pub email: String,
    pub response: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutlookCalendarEvent {
    pub id: String,
    pub title: String,
    pub organizer: Option<OutlookEventParticipant>,
    pub attendees: Vec<OutlookEventParticipant>,
    pub starts_at: String,
    pub ends_at: String,
    pub location: Option<String>,
    pub join_url: Option<String>,
    pub meeting_provider: Option<String>,
    pub web_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreparedOutlookNote {
    pub note_id: String,
    pub folder_path: String,
    pub event: OutlookCalendarEvent,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutlookMailCandidate {
    pub id: String,
    pub subject: String,
    pub sender: Option<OutlookEventParticipant>,
    pub to: Vec<OutlookEventParticipant>,
    pub cc: Vec<OutlookEventParticipant>,
    pub sent_at: Option<String>,
    pub received_at: Option<String>,
    pub conversation_id: Option<String>,
    pub has_attachments: bool,
    pub web_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutlookSelectedMail {
    #[serde(flatten)]
    pub message: OutlookMailCandidate,
    pub body_text: String,
    pub source_receipt: ContextSourceReceipt,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextSourceReceipt {
    pub schema: u32,
    pub source_id: String,
    pub source_kind: &'static str,
    pub provider: IntegrationProvider,
    pub title: String,
    pub occurred_at: Option<String>,
    pub selected_by_user: bool,
    pub content_included: bool,
}

#[derive(Debug, Deserialize)]
struct GraphEmailAddress {
    name: Option<String>,
    address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphRecipient {
    #[serde(rename = "emailAddress")]
    email_address: GraphEmailAddress,
}

#[derive(Debug, Deserialize)]
struct GraphResponseStatus {
    response: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphAttendee {
    #[serde(rename = "emailAddress")]
    email_address: GraphEmailAddress,
    status: Option<GraphResponseStatus>,
}

#[derive(Debug, Deserialize)]
struct GraphDateTime {
    #[serde(rename = "dateTime")]
    date_time: String,
}

#[derive(Debug, Deserialize)]
struct GraphLocation {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphOnlineMeeting {
    #[serde(rename = "joinUrl")]
    join_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphCalendarEvent {
    id: String,
    subject: Option<String>,
    organizer: Option<GraphRecipient>,
    #[serde(default)]
    attendees: Vec<GraphAttendee>,
    start: GraphDateTime,
    end: GraphDateTime,
    location: Option<GraphLocation>,
    #[serde(rename = "onlineMeetingUrl")]
    online_meeting_url: Option<String>,
    #[serde(rename = "onlineMeeting")]
    online_meeting: Option<GraphOnlineMeeting>,
    #[serde(rename = "webLink")]
    web_link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphCalendarPage {
    value: Vec<GraphCalendarEvent>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphMessageBody {
    #[serde(rename = "contentType")]
    content_type: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphMailMessage {
    id: String,
    subject: Option<String>,
    #[serde(rename = "from")]
    sender: Option<GraphRecipient>,
    #[serde(rename = "toRecipients", default)]
    to_recipients: Vec<GraphRecipient>,
    #[serde(rename = "ccRecipients", default)]
    cc_recipients: Vec<GraphRecipient>,
    #[serde(rename = "sentDateTime")]
    sent_at: Option<String>,
    #[serde(rename = "receivedDateTime")]
    received_at: Option<String>,
    #[serde(rename = "conversationId")]
    conversation_id: Option<String>,
    #[serde(rename = "hasAttachments", default)]
    has_attachments: bool,
    #[serde(rename = "webLink")]
    web_url: Option<String>,
    body: Option<GraphMessageBody>,
}

#[derive(Debug, Deserialize)]
struct GraphMailPage {
    value: Vec<GraphMailMessage>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
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

pub(crate) fn meeting_agent_feature_enabled<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<bool, String> {
    Ok(read_flags(app)?.teams_agent)
}

pub(crate) fn set_meeting_agent_feature_enabled<R: Runtime>(
    app: &AppHandle<R>,
    enabled: bool,
) -> Result<(), String> {
    let mut flags = read_flags(app)?;
    flags.teams_agent = enabled;
    write_flags(app, &flags)
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

fn save_token(
    provider: &IntegrationProvider,
    account_id: &str,
    token_kind: &str,
    token: &str,
) -> Result<(), String> {
    if token.trim().is_empty() {
        return Err("A Microsoft retornou uma credencial vazia".into());
    }
    let account = keyring_account(provider, account_id, token_kind)?;
    keyring::Entry::new(KEYRING_SERVICE, &account)
        .map_err(|error| error.to_string())?
        .set_password(token)
        .map_err(|error| format!("Não foi possível salvar a credencial segura: {error}"))
}

fn get_token(
    provider: &IntegrationProvider,
    account_id: &str,
    token_kind: &str,
) -> Result<Option<String>, String> {
    let account = keyring_account(provider, account_id, token_kind)?;
    let entry =
        keyring::Entry::new(KEYRING_SERVICE, &account).map_err(|error| error.to_string())?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("Não foi possível ler a credencial segura: {error}")),
    }
}

fn restore_token(
    provider: &IntegrationProvider,
    account_id: &str,
    token_kind: &str,
    previous: Option<&str>,
) {
    if let Some(token) = previous {
        let _ = save_token(provider, account_id, token_kind, token);
    } else {
        let _ = delete_token(provider, account_id, token_kind);
    }
}

fn microsoft_client_id() -> Option<String> {
    std::env::var("EMPATHY_MICROSOFT_CLIENT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| option_env!("EMPATHY_MICROSOFT_CLIENT_ID").map(str::to_string))
}

fn microsoft_tenant() -> String {
    std::env::var("EMPATHY_MICROSOFT_TENANT")
        .ok()
        .filter(|value| validate_identifier(value, "tenant Microsoft").is_ok())
        .unwrap_or_else(|| "common".into())
}

const MICROSOFT_BASE_SCOPES: &[&str] = &["openid", "profile", "offline_access", "User.Read"];

fn microsoft_scopes_for_permissions(permissions: &[ConnectorPermission]) -> Vec<&'static str> {
    let mut scopes = MICROSOFT_BASE_SCOPES.to_vec();
    if permissions.contains(&ConnectorPermission::CalendarBasic) {
        scopes.push("Calendars.ReadBasic");
    }
    if permissions.contains(&ConnectorPermission::MailContent) {
        // Mail.Read includes message metadata, so avoid requesting the redundant
        // Mail.ReadBasic scope once the user has explicitly approved content.
        scopes.push("Mail.Read");
    } else if permissions.contains(&ConnectorPermission::MailMetadata) {
        scopes.push("Mail.ReadBasic");
    }
    scopes
}

fn scope_was_granted(token: &MicrosoftTokenResponse, expected: &str) -> bool {
    token
        .scope
        .as_deref()
        .unwrap_or_default()
        .split_whitespace()
        .any(|scope| scope.eq_ignore_ascii_case(expected))
}

fn random_urlsafe(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    OsRng.fill_bytes(&mut value);
    URL_SAFE_NO_PAD.encode(value)
}

fn open_system_browser(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|_| "URL de autenticação inválida".to_string())?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("login.microsoftonline.com") {
        return Err("O login só pode ser aberto no domínio da Microsoft".into());
    }
    let status = if cfg!(target_os = "windows") {
        Command::new("explorer.exe").arg(url).status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).status()
    } else {
        Command::new("xdg-open").arg(url).status()
    }
    .map_err(|error| format!("Não foi possível abrir o navegador: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("O navegador terminou com status {status}"))
    }
}

async fn receive_oauth_callback(
    listener: TcpListener,
    expected_state: &str,
) -> Result<String, String> {
    let (mut stream, _) = timeout(Duration::from_secs(300), listener.accept())
        .await
        .map_err(|_| "O login Microsoft expirou após cinco minutos".to_string())?
        .map_err(|error| error.to_string())?;
    let mut buffer = vec![0_u8; 16 * 1024];
    let read = timeout(Duration::from_secs(10), stream.read(&mut buffer))
        .await
        .map_err(|_| "A resposta do login expirou".to_string())?
        .map_err(|error| error.to_string())?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| "Callback Microsoft inválido".to_string())?;
    let callback = url::Url::parse(&format!("http://localhost{path}"))
        .map_err(|_| "Callback Microsoft inválido".to_string())?;
    let query = callback
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    let state_matches = query
        .get("state")
        .is_some_and(|value| value.as_ref() == expected_state);
    let result = if !state_matches {
        Err("O estado do login não corresponde à solicitação original".to_string())
    } else if let Some(error) = query
        .get("error_description")
        .or_else(|| query.get("error"))
    {
        Err(format!("Login Microsoft cancelado: {error}"))
    } else {
        query
            .get("code")
            .map(|value| value.to_string())
            .ok_or_else(|| "A Microsoft não retornou o código de autorização".to_string())
    };
    let success = result.is_ok();
    let heading = if success {
        "Conta conectada"
    } else {
        "Não foi possível conectar"
    };
    let message = if success {
        "Você pode voltar ao Empathy."
    } else {
        "Volte ao Empathy para revisar o erro."
    };
    let body = format!("<!doctype html><meta charset=\"utf-8\"><title>Empathy</title><style>body{{font:16px -apple-system,BlinkMacSystemFont,sans-serif;background:#f5f5f7;color:#1d1d1f;display:grid;place-items:center;min-height:100vh;margin:0}}main{{max-width:420px;padding:32px;border-radius:20px;background:white;box-shadow:0 12px 40px #0001}}h1{{font-size:24px}}</style><main><h1>{heading}</h1><p>{message}</p></main>");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    result
}

async fn exchange_microsoft_code(
    tenant: &str,
    client_id: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
    scopes: &[&str],
) -> Result<MicrosoftTokenResponse, String> {
    let scope = scopes.join(" ");
    let response = reqwest::Client::new()
        .post(format!(
            "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token"
        ))
        .form(&[
            ("client_id", client_id),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
            ("scope", scope.as_str()),
        ])
        .send()
        .await
        .map_err(|error| format!("Falha ao contatar a Microsoft: {error}"))?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("error_description")
                    .and_then(|item| item.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(format!("A Microsoft recusou o login: {detail}"));
    }
    serde_json::from_str(&body).map_err(|error| format!("Resposta de token inválida: {error}"))
}

async fn refresh_microsoft_token(
    tenant: &str,
    client_id: &str,
    refresh_token: &str,
    scopes: &[&str],
) -> Result<MicrosoftTokenResponse, String> {
    let scope = scopes.join(" ");
    let response = reqwest::Client::new()
        .post(format!(
            "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token"
        ))
        .form(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", scope.as_str()),
        ])
        .send()
        .await
        .map_err(|error| format!("Falha ao renovar a sessão Microsoft: {error}"))?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("error_description")
                    .and_then(|item| item.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(format!(
            "A sessão Microsoft precisa ser conectada novamente: {detail}"
        ));
    }
    serde_json::from_str(&body).map_err(|error| format!("Resposta de renovação inválida: {error}"))
}

async fn microsoft_access_token<R: Runtime>(
    app: &AppHandle<R>,
    account_id: &str,
) -> Result<String, String> {
    validate_identifier(account_id, "conta")?;
    let mut accounts = read_accounts(app)?;
    let index = accounts
        .iter()
        .position(|account| {
            account.id == account_id && account.provider == IntegrationProvider::Microsoft
        })
        .ok_or_else(|| "Conta Microsoft não encontrada".to_string())?;
    let account = &accounts[index];
    let scopes = microsoft_scopes_for_permissions(&account.granted_permissions);
    let valid_until = account
        .token_expires_at
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok());
    if valid_until
        .is_some_and(|expires| expires > chrono::Utc::now() + chrono::Duration::minutes(2))
    {
        if let Some(access) = get_token(&account.provider, &account.id, "access")? {
            return Ok(access);
        }
    }
    let refresh = get_token(&account.provider, &account.id, "refresh")?
        .ok_or_else(|| "A conta Microsoft precisa ser conectada novamente".to_string())?;
    let client_id = microsoft_client_id().ok_or_else(|| {
        "O Client ID público do Empathy não está disponível nesta instalação".to_string()
    })?;
    let tenant = account.tenant_id.as_deref().unwrap_or("common");
    let token = refresh_microsoft_token(tenant, &client_id, &refresh, &scopes).await?;
    save_token(
        &account.provider,
        &account.id,
        "access",
        &token.access_token,
    )?;
    if let Some(rotated_refresh) = token.refresh_token.as_deref() {
        save_token(&account.provider, &account.id, "refresh", rotated_refresh)?;
    }
    let now = chrono::Utc::now();
    accounts[index].token_expires_at =
        Some((now + chrono::Duration::seconds(token.expires_in)).to_rfc3339());
    accounts[index].updated_at = now.to_rfc3339();
    write_accounts(app, &accounts)?;
    Ok(token.access_token)
}

async fn microsoft_profile(access_token: &str) -> Result<MicrosoftGraphProfile, String> {
    let response = reqwest::Client::new()
        .get("https://graph.microsoft.com/v1.0/me?$select=id,displayName,mail,userPrincipalName")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| format!("Falha ao consultar o perfil Microsoft: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "A Microsoft recusou a leitura do perfil: HTTP {}",
            response.status()
        ));
    }
    response
        .json()
        .await
        .map_err(|error| format!("Perfil Microsoft inválido: {error}"))
}

async fn authorize_microsoft(
    tenant: &str,
    client_id: &str,
    scopes: &[&str],
) -> Result<(MicrosoftTokenResponse, MicrosoftGraphProfile), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/oauth/callback");
    let verifier = random_urlsafe(64);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_urlsafe(32);
    let mut authorization = url::Url::parse(&format!(
        "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize"
    ))
    .map_err(|error| error.to_string())?;
    authorization
        .query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", &scopes.join(" "))
        .append_pair("state", &state)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    open_system_browser(authorization.as_str())?;
    let code = receive_oauth_callback(listener, &state).await?;
    let token =
        exchange_microsoft_code(tenant, client_id, &code, &verifier, &redirect_uri, scopes).await?;
    let profile = microsoft_profile(&token.access_token).await?;
    Ok((token, profile))
}

fn merged_permissions(
    existing: &[ConnectorPermission],
    additions: &[ConnectorPermission],
) -> Vec<ConnectorPermission> {
    let mut permissions = existing.to_vec();
    for permission in additions {
        if !permissions.contains(permission) {
            permissions.push(permission.clone());
        }
    }
    permissions
}

fn normalize_graph_datetime(value: &str) -> Result<String, String> {
    if let Ok(date) = chrono::DateTime::parse_from_rfc3339(value) {
        return Ok(date.with_timezone(&chrono::Utc).to_rfc3339());
    }
    let naive = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .map_err(|_| format!("Data inválida retornada pelo Outlook: {value}"))?;
    Ok(naive.and_utc().to_rfc3339())
}

fn participant_from_graph(
    email: GraphEmailAddress,
    response: Option<String>,
) -> Option<OutlookEventParticipant> {
    let address = email.address?.trim().to_string();
    if address.is_empty() {
        return None;
    }
    Some(OutlookEventParticipant {
        display_name: email
            .name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| address.clone()),
        email: address,
        response,
    })
}

fn mail_participant_from_graph(recipient: GraphRecipient) -> Option<OutlookEventParticipant> {
    participant_from_graph(recipient.email_address, None)
}

fn normalize_mail_message(message: GraphMailMessage) -> Result<OutlookMailCandidate, String> {
    let normalize_optional_date = |value: Option<String>| {
        value
            .map(|date| normalize_graph_datetime(&date))
            .transpose()
    };
    Ok(OutlookMailCandidate {
        id: message.id,
        subject: message
            .subject
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Sem assunto".into()),
        sender: message.sender.and_then(mail_participant_from_graph),
        to: message
            .to_recipients
            .into_iter()
            .filter_map(mail_participant_from_graph)
            .collect(),
        cc: message
            .cc_recipients
            .into_iter()
            .filter_map(mail_participant_from_graph)
            .collect(),
        sent_at: normalize_optional_date(message.sent_at)?,
        received_at: normalize_optional_date(message.received_at)?,
        conversation_id: message.conversation_id,
        has_attachments: message.has_attachments,
        web_url: message.web_url,
    })
}

fn validate_mail_search_address(value: &str) -> Result<String, String> {
    let address = value.trim().to_ascii_lowercase();
    if address.is_empty()
        || address.len() > 254
        || address.matches('@').count() != 1
        || address.starts_with('@')
        || address.ends_with('@')
        || !address
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "@._+-".contains(character))
    {
        return Err(format!("E-mail de participante inválido: {value}"));
    }
    Ok(address)
}

fn validate_graph_message_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 1024
        || value.chars().any(|character| character.is_control())
    {
        return Err("Identificador de mensagem inválido".into());
    }
    Ok(())
}

fn microsoft_account_with_permission<R: Runtime>(
    app: &AppHandle<R>,
    account_id: &str,
    permission: ConnectorPermission,
) -> Result<ConnectedAccount, String> {
    validate_identifier(account_id, "conta")?;
    let account = read_accounts(app)?
        .into_iter()
        .find(|account| {
            account.id == account_id && account.provider == IntegrationProvider::Microsoft
        })
        .ok_or_else(|| "Conta Microsoft não encontrada".to_string())?;
    if !account.granted_permissions.contains(&permission) {
        let label = match permission {
            ConnectorPermission::MailMetadata => "Mail.ReadBasic",
            ConnectorPermission::MailContent => "Mail.Read",
            _ => "a permissão necessária",
        };
        return Err(format!(
            "Autorize {label} explicitamente antes de continuar"
        ));
    }
    Ok(account)
}

fn meeting_provider(join_url: Option<&str>) -> Option<String> {
    let host = join_url
        .and_then(|value| url::Url::parse(value).ok())
        .and_then(|value| value.host_str().map(str::to_ascii_lowercase));
    match host.as_deref() {
        Some(host) if host == "teams.microsoft.com" || host.ends_with(".teams.microsoft.com") => {
            Some("microsoft-teams".into())
        }
        Some(host) if host == "zoom.us" || host.ends_with(".zoom.us") => Some("zoom".into()),
        Some("meet.google.com") => Some("google-meet".into()),
        Some(_) => Some("other".into()),
        None => None,
    }
}

fn normalize_calendar_event(event: GraphCalendarEvent) -> Result<OutlookCalendarEvent, String> {
    let join_url = event
        .online_meeting
        .and_then(|meeting| meeting.join_url)
        .or(event.online_meeting_url);
    Ok(OutlookCalendarEvent {
        id: event.id,
        title: event
            .subject
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Reunião sem título".into()),
        organizer: event
            .organizer
            .and_then(|recipient| participant_from_graph(recipient.email_address, None)),
        attendees: event
            .attendees
            .into_iter()
            .filter_map(|attendee| {
                participant_from_graph(
                    attendee.email_address,
                    attendee.status.and_then(|status| status.response),
                )
            })
            .collect(),
        starts_at: normalize_graph_datetime(&event.start.date_time)?,
        ends_at: normalize_graph_datetime(&event.end.date_time)?,
        location: event
            .location
            .and_then(|location| location.display_name)
            .filter(|value| !value.trim().is_empty()),
        meeting_provider: meeting_provider(join_url.as_deref()),
        join_url,
        web_url: event.web_link,
    })
}

async fn fetch_outlook_event(
    access_token: &str,
    event_id: &str,
) -> Result<OutlookCalendarEvent, String> {
    if event_id.trim().is_empty() || event_id.len() > 2048 {
        return Err("Identificador de evento inválido".into());
    }
    let mut url = url::Url::parse("https://graph.microsoft.com/v1.0/me/events/")
        .map_err(|error| error.to_string())?;
    url.path_segments_mut()
        .map_err(|_| "Endpoint de evento inválido".to_string())?
        .push(event_id);
    url.query_pairs_mut().append_pair(
        "$select",
        "id,subject,organizer,attendees,start,end,location,onlineMeetingUrl,onlineMeeting,webLink",
    );
    let response = reqwest::Client::new()
        .get(url)
        .bearer_auth(access_token)
        .header("Prefer", "outlook.timezone=\"UTC\"")
        .send()
        .await
        .map_err(|error| format!("Falha ao buscar o evento selecionado: {error}"))?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "O evento selecionado não está mais disponível: HTTP {status}"
        ));
    }
    let event: GraphCalendarEvent = serde_json::from_str(&body)
        .map_err(|error| format!("Evento do Outlook inválido: {error}"))?;
    normalize_calendar_event(event)
}

#[tauri::command]
pub fn api_get_integration_capabilities() -> Vec<IntegrationCapability> {
    capabilities()
}

#[tauri::command]
pub fn api_get_microsoft_auth_readiness() -> MicrosoftAuthReadiness {
    let configured = microsoft_client_id().is_some();
    let permissions = [ConnectorPermission::CalendarBasic];
    MicrosoftAuthReadiness {
        configured,
        tenant: microsoft_tenant(),
        requested_scopes: microsoft_scopes_for_permissions(&permissions),
        missing: if configured {
            Vec::new()
        } else {
            vec!["EMPATHY_MICROSOFT_CLIENT_ID"]
        },
    }
}

#[tauri::command]
pub async fn api_connect_microsoft_calendar<R: Runtime>(
    app: AppHandle<R>,
) -> Result<ConnectedAccount, String> {
    let client_id = microsoft_client_id().ok_or_else(|| {
        "O Client ID público do Empathy ainda não foi configurado no Microsoft Entra".to_string()
    })?;
    let tenant = microsoft_tenant();
    let permissions = [ConnectorPermission::CalendarBasic];
    let scopes = microsoft_scopes_for_permissions(&permissions);
    let (token, profile) = authorize_microsoft(&tenant, &client_id, &scopes).await?;
    if !scope_was_granted(&token, "Calendars.ReadBasic") {
        return Err("A Microsoft não concedeu a permissão mínima Calendars.ReadBasic".into());
    }
    let refresh_token = token
        .refresh_token
        .as_deref()
        .ok_or_else(|| "A Microsoft não retornou autorização para renovar a sessão".to_string())?;
    let account_id = format!("microsoft-{}", profile.id);
    let now = chrono::Utc::now();
    let account = ConnectedAccount {
        schema: SCHEMA,
        id: account_id.clone(),
        provider: IntegrationProvider::Microsoft,
        subject: profile.id,
        tenant_id: (tenant != "common").then_some(tenant),
        email: profile.mail.unwrap_or(profile.user_principal_name),
        display_name: profile.display_name,
        granted_permissions: vec![ConnectorPermission::CalendarBasic],
        token_expires_at: Some((now + chrono::Duration::seconds(token.expires_in)).to_rfc3339()),
        connected_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    };
    account.validate()?;
    save_token(
        &account.provider,
        &account_id,
        "access",
        &token.access_token,
    )?;
    if let Err(error) = save_token(&account.provider, &account_id, "refresh", refresh_token) {
        let _ = delete_token(&account.provider, &account_id, "access");
        return Err(error);
    }
    let mut accounts = read_accounts(&app)?;
    accounts.retain(|candidate| {
        !(candidate.provider == IntegrationProvider::Microsoft
            && candidate.subject == account.subject)
    });
    accounts.push(account.clone());
    if let Err(error) = write_accounts(&app, &accounts) {
        let _ = delete_token(&account.provider, &account_id, "access");
        let _ = delete_token(&account.provider, &account_id, "refresh");
        return Err(error);
    }
    let mut flags = read_flags(&app)?;
    flags.outlook_calendar = true;
    if let Err(error) = write_flags(&app, &flags) {
        accounts.retain(|candidate| candidate.id != account_id);
        let _ = write_accounts(&app, &accounts);
        let _ = delete_token(&account.provider, &account_id, "access");
        let _ = delete_token(&account.provider, &account_id, "refresh");
        return Err(error);
    }
    Ok(account)
}

#[tauri::command]
pub async fn api_authorize_microsoft_mail<R: Runtime>(
    app: AppHandle<R>,
    account_id: String,
    include_content: bool,
) -> Result<ConnectedAccount, String> {
    validate_identifier(&account_id, "conta")?;
    let client_id = microsoft_client_id().ok_or_else(|| {
        "O Client ID público do Empathy ainda não foi configurado no Microsoft Entra".to_string()
    })?;
    let mut accounts = read_accounts(&app)?;
    let index = accounts
        .iter()
        .position(|account| {
            account.id == account_id && account.provider == IntegrationProvider::Microsoft
        })
        .ok_or_else(|| "Conta Microsoft não encontrada".to_string())?;
    let previous_account = accounts[index].clone();
    let previous_flags = if include_content {
        Some(read_flags(&app)?)
    } else {
        None
    };
    let additions = if include_content {
        vec![
            ConnectorPermission::MailMetadata,
            ConnectorPermission::MailContent,
        ]
    } else {
        vec![ConnectorPermission::MailMetadata]
    };
    let permissions = merged_permissions(&previous_account.granted_permissions, &additions);
    let scopes = microsoft_scopes_for_permissions(&permissions);
    let tenant = previous_account
        .tenant_id
        .clone()
        .unwrap_or_else(|| "common".into());
    let (token, profile) = authorize_microsoft(&tenant, &client_id, &scopes).await?;
    if profile.id != previous_account.subject {
        return Err(format!(
            "A autorização foi feita com outra conta. Entre como {}.",
            previous_account.email
        ));
    }
    let required_scope = if include_content {
        "Mail.Read"
    } else {
        "Mail.ReadBasic"
    };
    if !scope_was_granted(&token, required_scope)
        && !(required_scope == "Mail.ReadBasic" && scope_was_granted(&token, "Mail.Read"))
    {
        return Err(format!(
            "A Microsoft não concedeu a permissão solicitada {required_scope}"
        ));
    }
    let refresh_token = token
        .refresh_token
        .as_deref()
        .ok_or_else(|| "A Microsoft não retornou autorização para renovar a sessão".to_string())?;
    let previous_access = get_token(&previous_account.provider, &account_id, "access")?;
    let previous_refresh = get_token(&previous_account.provider, &account_id, "refresh")?;
    save_token(
        &previous_account.provider,
        &account_id,
        "access",
        &token.access_token,
    )?;
    if let Err(error) = save_token(
        &previous_account.provider,
        &account_id,
        "refresh",
        refresh_token,
    ) {
        restore_token(
            &previous_account.provider,
            &account_id,
            "access",
            previous_access.as_deref(),
        );
        return Err(error);
    }
    let now = chrono::Utc::now();
    accounts[index].granted_permissions = permissions;
    accounts[index].token_expires_at =
        Some((now + chrono::Duration::seconds(token.expires_in)).to_rfc3339());
    accounts[index].updated_at = now.to_rfc3339();
    if let Err(error) = write_accounts(&app, &accounts) {
        restore_token(
            &previous_account.provider,
            &account_id,
            "access",
            previous_access.as_deref(),
        );
        restore_token(
            &previous_account.provider,
            &account_id,
            "refresh",
            previous_refresh.as_deref(),
        );
        return Err(error);
    }
    if include_content {
        let previous_flags = previous_flags
            .ok_or_else(|| "Estado de privacidade do Outlook indisponível".to_string())?;
        let mut flags = previous_flags.clone();
        flags.outlook_mail_context = true;
        if let Err(error) = write_flags(&app, &flags) {
            let _ = write_accounts(
                &app,
                &[
                    accounts[..index].to_vec(),
                    vec![previous_account.clone()],
                    accounts[index + 1..].to_vec(),
                ]
                .concat(),
            );
            let _ = write_flags(&app, &previous_flags);
            restore_token(
                &previous_account.provider,
                &account_id,
                "access",
                previous_access.as_deref(),
            );
            restore_token(
                &previous_account.provider,
                &account_id,
                "refresh",
                previous_refresh.as_deref(),
            );
            return Err(error);
        }
    }
    Ok(accounts[index].clone())
}

#[tauri::command]
pub async fn api_list_outlook_events<R: Runtime>(
    app: AppHandle<R>,
    account_id: String,
    starts_at: String,
    ends_at: String,
) -> Result<Vec<OutlookCalendarEvent>, String> {
    let start = chrono::DateTime::parse_from_rfc3339(&starts_at)
        .map_err(|_| "Início do período inválido".to_string())?;
    let end = chrono::DateTime::parse_from_rfc3339(&ends_at)
        .map_err(|_| "Fim do período inválido".to_string())?;
    if end <= start {
        return Err("O fim do período deve ser posterior ao início".into());
    }
    if end - start > chrono::Duration::days(92) {
        return Err("Consulte no máximo 92 dias por vez".into());
    }
    let access_token = microsoft_access_token(&app, &account_id).await?;
    let mut url = url::Url::parse("https://graph.microsoft.com/v1.0/me/calendarView")
        .map_err(|error| error.to_string())?;
    url.query_pairs_mut()
        .append_pair("startDateTime", &start.to_rfc3339())
        .append_pair("endDateTime", &end.to_rfc3339())
        .append_pair("$top", "100")
        .append_pair("$select", "id,subject,organizer,attendees,start,end,location,onlineMeetingUrl,onlineMeeting,webLink");
    let client = reqwest::Client::new();
    let mut events = Vec::new();
    for _ in 0..10 {
        if url.scheme() != "https" || url.host_str() != Some("graph.microsoft.com") {
            return Err("A Microsoft retornou uma paginação fora do domínio esperado".into());
        }
        let response = client
            .get(url.clone())
            .bearer_auth(&access_token)
            .header("Prefer", "outlook.timezone=\"UTC\"")
            .send()
            .await
            .map_err(|error| format!("Falha ao consultar o calendário: {error}"))?;
        let status = response.status();
        let body = response.text().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(format!(
                "O Outlook recusou a consulta do calendário: HTTP {status}"
            ));
        }
        let page: GraphCalendarPage = serde_json::from_str(&body)
            .map_err(|error| format!("Resposta de calendário inválida: {error}"))?;
        for event in page.value {
            events.push(normalize_calendar_event(event)?);
        }
        let Some(next) = page.next_link else { break };
        url = url::Url::parse(&next).map_err(|_| "Link de paginação inválido".to_string())?;
    }
    events.sort_by(|left, right| left.starts_at.cmp(&right.starts_at));
    Ok(events)
}

#[tauri::command]
pub async fn api_search_outlook_mail_context<R: Runtime>(
    app: AppHandle<R>,
    account_id: String,
    event_id: String,
    participant_emails: Vec<String>,
    limit: Option<u32>,
) -> Result<Vec<OutlookMailCandidate>, String> {
    let account = read_accounts(&app)?
        .into_iter()
        .find(|candidate| {
            candidate.id == account_id && candidate.provider == IntegrationProvider::Microsoft
        })
        .ok_or_else(|| "Conta Microsoft não encontrada".to_string())?;
    if !account
        .granted_permissions
        .contains(&ConnectorPermission::MailMetadata)
        && !account
            .granted_permissions
            .contains(&ConnectorPermission::MailContent)
    {
        return Err("Autorize Mail.ReadBasic explicitamente antes de pesquisar e-mails".into());
    }
    if participant_emails.is_empty() || participant_emails.len() > 20 {
        return Err("Escolha entre 1 e 20 participantes para pesquisar".into());
    }
    let mut participants = participant_emails
        .iter()
        .map(|email| validate_mail_search_address(email))
        .collect::<Result<Vec<_>, _>>()?;
    participants.sort();
    participants.dedup();
    let access_token = microsoft_access_token(&app, &account_id).await?;
    let event = fetch_outlook_event(&access_token, &event_id).await?;
    let mut allowed_participants = event
        .attendees
        .iter()
        .map(|participant| participant.email.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    if let Some(organizer) = &event.organizer {
        allowed_participants.push(organizer.email.trim().to_ascii_lowercase());
    }
    allowed_participants.retain(|email| !email.eq_ignore_ascii_case(&account.email));
    if participants
        .iter()
        .any(|email| !allowed_participants.contains(email))
    {
        return Err("Pesquise apenas participantes do evento selecionado".into());
    }
    let capped_limit = limit.unwrap_or(25).clamp(1, 50) as usize;
    let search = format!(
        "\"{}\"",
        participants
            .iter()
            .map(|email| format!("participants:{email}"))
            .collect::<Vec<_>>()
            .join(" OR ")
    );
    let mut url = url::Url::parse("https://graph.microsoft.com/v1.0/me/messages")
        .map_err(|error| error.to_string())?;
    url.query_pairs_mut()
        .append_pair("$search", &search)
        .append_pair("$top", &capped_limit.min(25).to_string())
        .append_pair("$select", "id,subject,from,toRecipients,ccRecipients,sentDateTime,receivedDateTime,conversationId,hasAttachments,webLink");
    let client = reqwest::Client::new();
    let mut messages = Vec::new();
    for _ in 0..4 {
        if url.scheme() != "https" || url.host_str() != Some("graph.microsoft.com") {
            return Err("A Microsoft retornou uma paginação fora do domínio esperado".into());
        }
        let response = client
            .get(url.clone())
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|error| format!("Falha ao pesquisar e-mails no Outlook: {error}"))?;
        let status = response.status();
        let body = response.text().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(format!(
                "O Outlook recusou a pesquisa de e-mails: HTTP {status}"
            ));
        }
        let page: GraphMailPage = serde_json::from_str(&body)
            .map_err(|error| format!("Resposta de e-mail inválida: {error}"))?;
        for message in page.value {
            messages.push(normalize_mail_message(message)?);
            if messages.len() >= capped_limit {
                break;
            }
        }
        if messages.len() >= capped_limit {
            break;
        }
        let Some(next) = page.next_link else { break };
        url = url::Url::parse(&next).map_err(|_| "Link de paginação inválido".to_string())?;
    }
    Ok(messages)
}

#[tauri::command]
pub async fn api_get_selected_outlook_mail<R: Runtime>(
    app: AppHandle<R>,
    account_id: String,
    message_ids: Vec<String>,
) -> Result<Vec<OutlookSelectedMail>, String> {
    microsoft_account_with_permission(&app, &account_id, ConnectorPermission::MailContent)?;
    if message_ids.is_empty() || message_ids.len() > 10 {
        return Err("Selecione entre 1 e 10 mensagens".into());
    }
    let mut unique_ids = Vec::new();
    for message_id in message_ids {
        validate_graph_message_id(&message_id)?;
        if !unique_ids.contains(&message_id) {
            unique_ids.push(message_id);
        }
    }
    let access_token = microsoft_access_token(&app, &account_id).await?;
    let client = reqwest::Client::new();
    let mut selected = Vec::with_capacity(unique_ids.len());
    for message_id in unique_ids {
        let mut url = url::Url::parse("https://graph.microsoft.com/v1.0/me/messages/")
            .map_err(|error| error.to_string())?;
        url.path_segments_mut()
            .map_err(|_| "URL de mensagem inválida".to_string())?
            .pop_if_empty()
            .push(&message_id);
        url.query_pairs_mut().append_pair(
            "$select",
            "id,subject,from,toRecipients,ccRecipients,sentDateTime,receivedDateTime,conversationId,hasAttachments,webLink,body",
        );
        let response = client
            .get(url)
            .bearer_auth(&access_token)
            .header("Prefer", "outlook.body-content-type=\"text\"")
            .send()
            .await
            .map_err(|error| format!("Falha ao ler a mensagem selecionada: {error}"))?;
        let status = response.status();
        let body = response.text().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(format!(
                "A mensagem selecionada não está mais disponível: HTTP {status}"
            ));
        }
        let mut graph_message: GraphMailMessage = serde_json::from_str(&body)
            .map_err(|error| format!("Mensagem selecionada inválida: {error}"))?;
        let message_body = graph_message
            .body
            .take()
            .ok_or_else(|| "A mensagem selecionada não retornou conteúdo".to_string())?;
        if message_body
            .content_type
            .as_deref()
            .is_some_and(|content_type| !content_type.eq_ignore_ascii_case("text"))
        {
            return Err("O Outlook não retornou o conteúdo selecionado como texto".into());
        }
        let body_text = message_body.content.unwrap_or_default();
        let message = normalize_mail_message(graph_message)?;
        let receipt = ContextSourceReceipt {
            schema: SCHEMA,
            source_id: message.id.clone(),
            source_kind: "mail-message",
            provider: IntegrationProvider::Microsoft,
            title: message.subject.clone(),
            occurred_at: message
                .sent_at
                .clone()
                .or_else(|| message.received_at.clone()),
            selected_by_user: true,
            content_included: true,
        };
        selected.push(OutlookSelectedMail {
            message,
            body_text,
            source_receipt: receipt,
        });
    }
    Ok(selected)
}

#[tauri::command]
pub async fn api_create_note_from_outlook_event<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, crate::state::AppState>,
    account_id: String,
    event_id: String,
) -> Result<PreparedOutlookNote, String> {
    let access_token = microsoft_access_token(&app, &account_id).await?;
    // Fetch the concrete event again at the moment of the write. A stale list
    // item is never used to create participants or source metadata.
    let event = fetch_outlook_event(&access_token, &event_id).await?;
    let receipt = format!(
        "<!-- empathy-source-receipt\nschema: 1\nsource_kind: calendar-event\nprovider: microsoft\nsource_id: {}\nselected_by_user: true\ncontent_included: true\n-->\n\n# {}\n\n## Preparação da reunião\n\nUse a Skill **Preparar reunião** para desenvolver o contexto deste encontro.\n",
        event.id, event.title
    );
    let pool = state.db_manager.pool().clone();
    let created = crate::api::api_create_note(
        app,
        state,
        event.title.clone(),
        receipt,
        Some(event.starts_at.clone()),
    )
    .await?;
    let mut participants = event
        .attendees
        .iter()
        .map(|participant| participant.display_name.clone())
        .collect::<Vec<_>>();
    if let Some(organizer) = &event.organizer {
        participants.push(organizer.display_name.clone());
    }
    participants.sort_by_key(|value| value.to_lowercase());
    participants.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    let linked_at = chrono::Utc::now().to_rfc3339();
    let external_meeting = serde_yaml::to_value(serde_json::json!({
        "schema": 1,
        "provider": "microsoft",
        "account_id": account_id,
        "calendar_event_id": event.id,
        "meeting_provider": event.meeting_provider,
        "starts_at": event.starts_at,
        "ends_at": event.ends_at,
        "join_url": event.join_url,
        "organizer": event.organizer,
        "attendees": event.attendees,
        "linked_at": linked_at,
    }))
    .map_err(|error| error.to_string())?;
    if let Err(error) = crate::meeting_files::attach_external_meeting(
        std::path::Path::new(&created.folder_path),
        external_meeting,
        &participants,
        &linked_at,
    ) {
        let _ = sqlx::query("DELETE FROM meetings WHERE id = ?")
            .bind(&created.id)
            .execute(&pool)
            .await;
        let _ = std::fs::remove_dir_all(&created.folder_path);
        return Err(format!(
            "Não foi possível associar o evento à Nota: {error}"
        ));
    }
    Ok(PreparedOutlookNote {
        note_id: created.id,
        folder_path: created.folder_path,
        event,
    })
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
    if account.provider == IntegrationProvider::Microsoft
        && !accounts
            .iter()
            .any(|candidate| candidate.provider == IntegrationProvider::Microsoft)
    {
        let mut flags = read_flags(&app)?;
        flags.outlook_calendar = false;
        flags.outlook_mail_context = false;
        write_flags(&app, &flags)?;
    }
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
            token_expires_at: None,
            connected_at: "2026-08-05T12:00:00Z".into(),
            updated_at: "2026-08-05T12:00:00Z".into(),
        };
        assert!(account.validate().is_err());
    }

    #[test]
    fn pkce_material_is_url_safe_and_unpredictable_per_run() {
        let first = random_urlsafe(64);
        let second = random_urlsafe(64);
        assert_ne!(first, second);
        assert!(first.len() >= 43);
        assert!(first
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character)));
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(first.as_bytes()));
        assert_eq!(challenge.len(), 43);
    }

    #[test]
    fn meeting_links_are_classified_without_trusting_the_path() {
        assert_eq!(
            meeting_provider(Some("https://teams.microsoft.com/l/meetup-join/abc")),
            Some("microsoft-teams".into())
        );
        assert_eq!(
            meeting_provider(Some("https://acme.zoom.us/j/123")),
            Some("zoom".into())
        );
        assert_eq!(
            meeting_provider(Some("https://meet.google.com/abc-defg-hij")),
            Some("google-meet".into())
        );
        assert_eq!(meeting_provider(Some("javascript:alert(1)")), None);
    }

    #[test]
    fn outlook_dates_are_normalized_to_utc() {
        assert_eq!(
            normalize_graph_datetime("2026-08-05T12:00:00-03:00").unwrap(),
            "2026-08-05T15:00:00+00:00"
        );
        assert_eq!(
            normalize_graph_datetime("2026-08-05T15:00:00.0000000").unwrap(),
            "2026-08-05T15:00:00+00:00"
        );
    }

    #[test]
    fn microsoft_scopes_expand_progressively_without_write_access() {
        let calendar = microsoft_scopes_for_permissions(&[ConnectorPermission::CalendarBasic]);
        assert!(calendar.contains(&"Calendars.ReadBasic"));
        assert!(!calendar.contains(&"Mail.ReadBasic"));
        assert!(!calendar.contains(&"Mail.Read"));

        let metadata = microsoft_scopes_for_permissions(&[
            ConnectorPermission::CalendarBasic,
            ConnectorPermission::MailMetadata,
        ]);
        assert!(metadata.contains(&"Mail.ReadBasic"));
        assert!(!metadata.contains(&"Mail.Read"));

        let content = microsoft_scopes_for_permissions(&[
            ConnectorPermission::CalendarBasic,
            ConnectorPermission::MailMetadata,
            ConnectorPermission::MailContent,
        ]);
        assert!(content.contains(&"Mail.Read"));
        assert!(!content.contains(&"Mail.ReadBasic"));
        assert!(content.iter().all(|scope| !scope.contains("ReadWrite")));
    }

    #[test]
    fn mail_search_addresses_are_restricted_before_kql_composition() {
        assert_eq!(
            validate_mail_search_address("Person.Name+tag@Example.com").unwrap(),
            "person.name+tag@example.com"
        );
        assert!(validate_mail_search_address("person@example.com OR from:boss").is_err());
        assert!(validate_mail_search_address("person\"@example.com").is_err());
        assert!(validate_mail_search_address("missing-domain@").is_err());
    }

    #[test]
    fn message_ids_allow_provider_values_but_reject_control_characters() {
        assert!(validate_graph_message_id("AAMkAGVmMDEzLTI=/AQMkAD==").is_ok());
        assert!(validate_graph_message_id("message\nnext").is_err());
        assert!(validate_graph_message_id("").is_err());
    }
}
