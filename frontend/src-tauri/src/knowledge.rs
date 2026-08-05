//! Markdown-first knowledge workspace.
//!
//! Everything in this module is derived from user-owned files. SQLite is only a
//! disposable search/index cache and can be rebuilt at any time.

use crate::audio::recording_preferences::load_recording_preferences;
use crate::state::AppState;
use chrono::Utc;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, Transaction};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Runtime, State};
use walkdir::WalkDir;

const SUPPORTED_IMPORT_EXTENSIONS: &[&str] = &[
    "md", "markdown", "txt", "html", "htm", "vtt", "srt", "csv", "json",
];
const SUPPORTED_ATTACHMENT_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "pdf", "mp3", "m4a", "wav", "flac", "ogg",
];
static REINDEX_LOCK: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

#[derive(Default)]
pub struct KnowledgeWatcherState(pub Mutex<Option<RecommendedWatcher>>);

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Frontmatter {
    empathy_schema: Option<u32>,
    id: Option<String>,
    meeting_id: Option<String>,
    document: Option<String>,
    #[serde(rename = "type")]
    document_type: Option<String>,
    title: Option<String>,
    project: Option<String>,
    participants: Vec<String>,
    speakers: Vec<String>,
    tags: Vec<String>,
    status: Option<String>,
}

#[derive(Debug, Clone)]
struct IndexedDocument {
    path: String,
    meeting_id: Option<String>,
    kind: String,
    title: String,
    project: Option<String>,
    participants: Vec<String>,
    tags: Vec<String>,
    status: Option<String>,
    content: String,
    modified_ms: i64,
    links: Vec<(String, Option<String>)>,
    tasks: Vec<IndexedTask>,
    decisions: Vec<(usize, String)>,
}

#[derive(Debug, Clone)]
struct IndexedTask {
    line: usize,
    text: String,
    owner: Option<String>,
    completed: bool,
}

#[derive(Debug, Serialize)]
pub struct ReindexResult {
    pub root: String,
    pub documents: usize,
    pub meetings: usize,
    pub links: usize,
    pub tasks: usize,
    pub decisions: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct KnowledgeDocument {
    pub path: String,
    pub meeting_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub project: Option<String>,
    pub participants_json: String,
    pub tags_json: String,
    pub status: Option<String>,
    pub modified_ms: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KnowledgeGraphNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub meeting_id: Option<String>,
    pub path: Option<String>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KnowledgeGraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: String,
    pub weight: usize,
}

#[derive(Debug, Serialize)]
pub struct KnowledgeGraph {
    pub nodes: Vec<KnowledgeGraphNode>,
    pub edges: Vec<KnowledgeGraphEdge>,
    pub truncated: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct KnowledgeTask {
    pub id: String,
    pub meeting_id: Option<String>,
    pub document_path: String,
    pub text: String,
    pub owner: Option<String>,
    pub completed: bool,
    pub line_number: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct KnowledgeDecision {
    pub id: String,
    pub meeting_id: Option<String>,
    pub document_path: String,
    pub text: String,
    pub line_number: i64,
}

#[derive(Debug, Serialize)]
pub struct KnowledgeDashboard {
    pub documents: i64,
    pub meetings: i64,
    pub projects: Vec<CountedValue>,
    pub participants: Vec<CountedValue>,
    pub tags: Vec<CountedValue>,
    pub open_tasks: Vec<KnowledgeTask>,
    pub recent_decisions: Vec<KnowledgeDecision>,
}

#[derive(Debug, Serialize)]
pub struct CountedValue {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct KnowledgeSearchResult {
    pub path: String,
    pub meeting_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub project: Option<String>,
    pub snippet: String,
    pub score: usize,
}

#[derive(Debug, Serialize)]
pub struct KnowledgeDocumentContent {
    pub path: String,
    pub title: String,
    pub kind: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct RelatedMeeting {
    pub meeting_id: String,
    pub title: String,
    pub path: String,
    pub reasons: Vec<String>,
    pub score: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MeetingProperties {
    pub project: Option<String>,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WebContextInput {
    pub title: String,
    pub url: String,
    pub content: String,
    pub project: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub action: String,
    #[serde(default)]
    pub config: Value,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct KnowledgeExtension {
    pub id: String,
    pub name: String,
    pub description: String,
    pub action: String,
    pub config_json: String,
    pub enabled: bool,
    pub source_path: String,
}

fn split_frontmatter(content: &str) -> (Frontmatter, &str) {
    if let Some(rest) = content.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---") {
            let yaml = &rest[..end];
            let body_start = end + 4;
            let body = rest
                .get(body_start..)
                .unwrap_or_default()
                .trim_start_matches('\n');
            return (serde_yaml::from_str(yaml).unwrap_or_default(), body);
        }
    }
    (Frontmatter::default(), content)
}

fn first_heading(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned)
}

fn infer_kind(path: &Path, frontmatter: &Frontmatter) -> String {
    frontmatter
        .document_type
        .clone()
        .or_else(|| frontmatter.document.clone())
        .unwrap_or_else(|| match path.file_name().and_then(|name| name.to_str()) {
            Some("meeting.md") => "meeting".into(),
            Some("transcript.md") => "transcript".into(),
            Some("summary.md") => "summary".into(),
            _ => "note".into(),
        })
}

fn stable_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn extract_links(body: &str) -> Vec<(String, Option<String>)> {
    let markdown = Regex::new(r"\[([^\]]+)\]\(([^)]+\.md(?:#[^)]*)?)\)").unwrap();
    let wikilink = Regex::new(r"\[\[([^\]|#]+)(?:#[^\]|]+)?(?:\|([^\]]+))?\]\]").unwrap();
    let mut links = BTreeMap::<String, Option<String>>::new();
    for capture in markdown.captures_iter(body) {
        links.insert(capture[2].to_string(), Some(capture[1].to_string()));
    }
    for capture in wikilink.captures_iter(body) {
        links.insert(
            capture[1].to_string(),
            capture.get(2).map(|value| value.as_str().to_string()),
        );
    }
    links.into_iter().collect()
}

fn extract_tasks_and_decisions(body: &str) -> (Vec<IndexedTask>, Vec<(usize, String)>) {
    let task = Regex::new(r"^\s*[-*]\s*\[([ xX])\]\s*(.+)$").unwrap();
    let owner = Regex::new(r"^(?:@([^:—-]+)|([^:—-]{2,40}))\s*[:—-]\s*(.+)$").unwrap();
    let mut tasks = Vec::new();
    let mut decisions = Vec::new();
    let mut in_decisions = false;

    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let heading = trimmed.trim_start_matches('#').trim().to_lowercase();
            in_decisions = heading.contains("decision") || heading.contains("decis");
            continue;
        }
        if let Some(capture) = task.captures(line) {
            let raw_text = capture[2].trim().to_string();
            let owner_name = owner.captures(&raw_text).and_then(|parts| {
                parts
                    .get(1)
                    .or_else(|| parts.get(2))
                    .map(|value| value.as_str().trim().to_string())
            });
            tasks.push(IndexedTask {
                line: index + 1,
                text: raw_text,
                owner: owner_name,
                completed: !capture[1].trim().is_empty(),
            });
        } else if in_decisions {
            let text = trimmed
                .strip_prefix("- ")
                .or_else(|| trimmed.strip_prefix("* "));
            if let Some(text) = text.filter(|value| !value.trim().is_empty()) {
                decisions.push((index + 1, text.trim().to_string()));
            }
        }
    }
    (tasks, decisions)
}

fn index_file(path: &Path) -> Result<IndexedDocument, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let (frontmatter, body) = split_frontmatter(&content);
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default();
    let title = frontmatter
        .title
        .clone()
        .or_else(|| first_heading(body))
        .or_else(|| path.file_stem()?.to_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "Documento sem título".into());
    let meeting_id = frontmatter.meeting_id.clone().or_else(|| {
        frontmatter
            .id
            .clone()
            .filter(|_| infer_kind(path, &frontmatter) == "meeting")
    });
    let mut participants = frontmatter.participants.clone();
    participants.extend(frontmatter.speakers.clone());
    participants.sort();
    participants.dedup();
    let (tasks, decisions) = extract_tasks_and_decisions(body);
    let links = extract_links(body);

    Ok(IndexedDocument {
        path: path.to_string_lossy().to_string(),
        meeting_id,
        kind: infer_kind(path, &frontmatter),
        title,
        project: frontmatter.project.filter(|value| !value.trim().is_empty()),
        participants,
        tags: frontmatter.tags,
        status: frontmatter.status,
        content,
        modified_ms,
        links,
        tasks,
        decisions,
    })
}

async fn workspace_root<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let preferences = load_recording_preferences(app)
        .await
        .map_err(|error| format!("Não foi possível carregar a pasta do workspace: {error}"))?;
    fs::create_dir_all(&preferences.save_folder)
        .map_err(|error| format!("Não foi possível criar a pasta do workspace: {error}"))?;
    Ok(preferences.save_folder)
}

fn scan_workspace(root: &Path) -> (Vec<IndexedDocument>, Vec<String>) {
    let mut documents = Vec::new();
    let mut errors = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            name != ".empathy-trash" && name != ".git" && name != "node_modules"
        })
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                errors.push(error.to_string());
                continue;
            }
        };
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("md")
        {
            continue;
        }
        match index_file(path) {
            Ok(document) => documents.push(document),
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    (documents, errors)
}

async fn persist_document(
    transaction: &mut Transaction<'_, Sqlite>,
    document: &IndexedDocument,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO knowledge_documents
         (path, meeting_id, kind, title, project, participants_json, tags_json, status, content, modified_ms, indexed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&document.path)
    .bind(&document.meeting_id)
    .bind(&document.kind)
    .bind(&document.title)
    .bind(&document.project)
    .bind(serde_json::to_string(&document.participants).unwrap_or_else(|_| "[]".into()))
    .bind(serde_json::to_string(&document.tags).unwrap_or_else(|_| "[]".into()))
    .bind(&document.status)
    .bind(&document.content)
    .bind(document.modified_ms)
    .bind(Utc::now().to_rfc3339())
    .execute(&mut **transaction)
    .await?;

    for (target, label) in &document.links {
        sqlx::query(
            "INSERT OR IGNORE INTO knowledge_links (source_path, target, label) VALUES (?, ?, ?)",
        )
        .bind(&document.path)
        .bind(target)
        .bind(label)
        .execute(&mut **transaction)
        .await?;
    }
    for task in &document.tasks {
        let id = stable_hash(&[&document.path, &task.line.to_string(), &task.text]);
        sqlx::query(
            "INSERT INTO knowledge_tasks
             (id, meeting_id, document_path, text, owner, completed, line_number)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&document.meeting_id)
        .bind(&document.path)
        .bind(&task.text)
        .bind(&task.owner)
        .bind(task.completed)
        .bind(task.line as i64)
        .execute(&mut **transaction)
        .await?;
    }
    for (line, text) in &document.decisions {
        let id = stable_hash(&[&document.path, &line.to_string(), text]);
        sqlx::query(
            "INSERT INTO knowledge_decisions (id, meeting_id, document_path, text, line_number)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&document.meeting_id)
        .bind(&document.path)
        .bind(text)
        .bind(*line as i64)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn api_reindex_knowledge<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<ReindexResult, String> {
    let _guard = REINDEX_LOCK.lock().await;
    let root = workspace_root(&app).await?;
    let (documents, errors) = scan_workspace(&root);
    let mut transaction = state
        .db_manager
        .pool()
        .begin()
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("DELETE FROM knowledge_links")
        .execute(&mut *transaction)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("DELETE FROM knowledge_tasks")
        .execute(&mut *transaction)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("DELETE FROM knowledge_decisions")
        .execute(&mut *transaction)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("DELETE FROM knowledge_documents")
        .execute(&mut *transaction)
        .await
        .map_err(|error| error.to_string())?;
    for document in &documents {
        persist_document(&mut transaction, document)
            .await
            .map_err(|error| error.to_string())?;
    }
    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    Ok(ReindexResult {
        root: root.to_string_lossy().to_string(),
        documents: documents.len(),
        meetings: documents
            .iter()
            .filter(|document| document.kind == "meeting")
            .count(),
        links: documents.iter().map(|document| document.links.len()).sum(),
        tasks: documents.iter().map(|document| document.tasks.len()).sum(),
        decisions: documents
            .iter()
            .map(|document| document.decisions.len())
            .sum(),
        errors,
    })
}

fn counted_values(values: impl Iterator<Item = String>) -> Vec<CountedValue> {
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values.filter(|value| !value.trim().is_empty()) {
        *counts.entry(value).or_default() += 1;
    }
    let mut result = counts
        .into_iter()
        .map(|(value, count)| CountedValue { value, count })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then(left.value.cmp(&right.value))
    });
    result
}

fn graph_entity_id(kind: &str, value: &str) -> String {
    format!("{kind}:{}", stable_hash(&[value]))
}

fn insert_graph_node(nodes: &mut BTreeMap<String, KnowledgeGraphNode>, node: KnowledgeGraphNode) {
    nodes
        .entry(node.id.clone())
        .and_modify(|existing| existing.count = existing.count.saturating_add(node.count))
        .or_insert(node);
}

fn insert_graph_edge(
    edges: &mut BTreeMap<String, KnowledgeGraphEdge>,
    source: &str,
    target: &str,
    kind: &str,
    weight: usize,
) {
    if source == target {
        return;
    }
    let id = stable_hash(&[source, target, kind]);
    edges
        .entry(id.clone())
        .and_modify(|existing| existing.weight = existing.weight.saturating_add(weight))
        .or_insert_with(|| KnowledgeGraphEdge {
            id,
            source: source.to_string(),
            target: target.to_string(),
            kind: kind.to_string(),
            weight,
        });
}

fn build_knowledge_graph(
    documents: &[KnowledgeDocument],
    link_rows: &[(String, String)],
    tasks: &[KnowledgeTask],
    decisions: &[KnowledgeDecision],
    meeting_filter: Option<&str>,
) -> KnowledgeGraph {
    const GLOBAL_DOCUMENT_LIMIT: usize = 240;
    const MEETING_DOCUMENT_LIMIT: usize = 120;
    const DETAIL_LIMIT: usize = 80;

    let document_limit = if meeting_filter.is_some() {
        MEETING_DOCUMENT_LIMIT
    } else {
        GLOBAL_DOCUMENT_LIMIT
    };
    let matching_documents = documents
        .iter()
        .filter(|document| {
            meeting_filter
                .is_none_or(|meeting_id| document.meeting_id.as_deref() == Some(meeting_id))
        })
        .collect::<Vec<_>>();
    let truncated = matching_documents.len() > document_limit;
    let selected_documents = matching_documents
        .into_iter()
        .take(document_limit)
        .collect::<Vec<_>>();

    let mut nodes = BTreeMap::<String, KnowledgeGraphNode>::new();
    let mut edges = BTreeMap::<String, KnowledgeGraphEdge>::new();
    let mut document_nodes = BTreeMap::<String, String>::new();
    let mut meeting_anchors = BTreeMap::<String, String>::new();

    for document in &selected_documents {
        // A portable person document is the canonical version of the same
        // graph entity referenced by meeting participant names.
        let id = if document.kind == "person" {
            graph_entity_id("person", &document.title)
        } else {
            graph_entity_id("document", &document.path)
        };
        document_nodes.insert(document.path.clone(), id.clone());
        insert_graph_node(
            &mut nodes,
            KnowledgeGraphNode {
                id: id.clone(),
                label: document.title.clone(),
                kind: document.kind.clone(),
                meeting_id: document.meeting_id.clone(),
                path: Some(document.path.clone()),
                count: 1,
            },
        );
        if let Some(meeting_id) = &document.meeting_id {
            let anchor = meeting_anchors
                .entry(meeting_id.clone())
                .or_insert_with(|| id.clone());
            if document.kind == "meeting" {
                *anchor = id;
            }
        }
    }

    for document in &selected_documents {
        let Some(document_id) = document_nodes.get(&document.path) else {
            continue;
        };
        let anchor = document
            .meeting_id
            .as_ref()
            .and_then(|meeting_id| meeting_anchors.get(meeting_id))
            .cloned()
            .unwrap_or_else(|| document_id.clone());
        if &anchor != document_id {
            insert_graph_edge(&mut edges, &anchor, document_id, "contains", 2);
        }

        if let Some(project) = document
            .project
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let project_id = graph_entity_id("project", project);
            insert_graph_node(
                &mut nodes,
                KnowledgeGraphNode {
                    id: project_id.clone(),
                    label: project.to_string(),
                    kind: "project".into(),
                    meeting_id: None,
                    path: None,
                    count: 1,
                },
            );
            insert_graph_edge(&mut edges, &anchor, &project_id, "project", 3);
        }
        for participant in serde_json::from_str::<Vec<String>>(&document.participants_json)
            .unwrap_or_default()
            .into_iter()
            .filter(|value| !value.trim().is_empty())
        {
            let participant_id = graph_entity_id("person", &participant);
            insert_graph_node(
                &mut nodes,
                KnowledgeGraphNode {
                    id: participant_id.clone(),
                    label: participant,
                    kind: "person".into(),
                    meeting_id: None,
                    path: None,
                    count: 1,
                },
            );
            insert_graph_edge(&mut edges, &anchor, &participant_id, "participant", 2);
        }
        for tag in serde_json::from_str::<Vec<String>>(&document.tags_json)
            .unwrap_or_default()
            .into_iter()
            .filter(|value| !value.trim().is_empty())
        {
            let tag_id = graph_entity_id("tag", &tag);
            insert_graph_node(
                &mut nodes,
                KnowledgeGraphNode {
                    id: tag_id.clone(),
                    label: tag,
                    kind: "tag".into(),
                    meeting_id: None,
                    path: None,
                    count: 1,
                },
            );
            insert_graph_edge(&mut edges, &anchor, &tag_id, "tag", 1);
        }
    }

    for (source_path, target) in link_rows {
        let Some(source_id) = document_nodes.get(source_path) else {
            continue;
        };
        if let Some((_target_path, target_id)) = document_nodes
            .iter()
            .find(|(path, _)| link_targets_document(source_path, target, path))
        {
            insert_graph_edge(&mut edges, source_id, target_id, "link", 4);
        }
    }

    for task in tasks
        .iter()
        .filter(|task| !task.completed)
        .filter(|task| meeting_filter.is_none_or(|value| task.meeting_id.as_deref() == Some(value)))
        .take(DETAIL_LIMIT)
    {
        let Some(anchor) = task
            .meeting_id
            .as_ref()
            .and_then(|meeting_id| meeting_anchors.get(meeting_id))
            .cloned()
            .or_else(|| document_nodes.get(&task.document_path).cloned())
        else {
            continue;
        };
        let task_id = format!("task:{}", task.id);
        insert_graph_node(
            &mut nodes,
            KnowledgeGraphNode {
                id: task_id.clone(),
                label: task.text.clone(),
                kind: "task".into(),
                meeting_id: task.meeting_id.clone(),
                path: Some(task.document_path.clone()),
                count: 1,
            },
        );
        insert_graph_edge(&mut edges, &anchor, &task_id, "task", 2);
    }

    for decision in decisions
        .iter()
        .filter(|decision| {
            meeting_filter.is_none_or(|value| decision.meeting_id.as_deref() == Some(value))
        })
        .take(DETAIL_LIMIT)
    {
        let Some(anchor) = decision
            .meeting_id
            .as_ref()
            .and_then(|meeting_id| meeting_anchors.get(meeting_id))
            .cloned()
            .or_else(|| document_nodes.get(&decision.document_path).cloned())
        else {
            continue;
        };
        let decision_id = format!("decision:{}", decision.id);
        insert_graph_node(
            &mut nodes,
            KnowledgeGraphNode {
                id: decision_id.clone(),
                label: decision.text.clone(),
                kind: "decision".into(),
                meeting_id: decision.meeting_id.clone(),
                path: Some(decision.document_path.clone()),
                count: 1,
            },
        );
        insert_graph_edge(&mut edges, &anchor, &decision_id, "decision", 3);
    }

    KnowledgeGraph {
        nodes: nodes.into_values().collect(),
        edges: edges.into_values().collect(),
        truncated,
    }
}

fn augment_graph_with_skill_results(
    graph: &mut KnowledgeGraph,
    contents: &[(String, Option<String>, String)],
) {
    let expression =
        Regex::new(r"(?s)<!-- empathy-skill-result\n(.*?)-->\n(.*?)<!-- /empathy-skill-result -->")
            .unwrap();
    let title_expression = Regex::new(r"(?m)^##\s+(.+)$").unwrap();
    for (path, meeting_id, content) in contents {
        let Some(anchor) = graph
            .nodes
            .iter()
            .find(|node| node.path.as_deref() == Some(path) && node.kind == "meeting")
            .map(|node| node.id.clone())
        else {
            continue;
        };
        for capture in expression.captures_iter(content) {
            let metadata = capture[1]
                .lines()
                .filter_map(|line| line.split_once(':'))
                .map(|(key, value)| (key.trim(), value.trim()))
                .collect::<BTreeMap<_, _>>();
            let result_key = metadata.get("id").copied().unwrap_or("unknown");
            let result_id = format!("skill-result:{result_key}");
            let skill_key = metadata.get("skill_id").copied().unwrap_or("skill");
            let skill_id = format!("skill:{skill_key}");
            let skill_name = metadata.get("skill_name").copied().unwrap_or(skill_key);
            let layer = metadata.get("layer").copied().unwrap_or("artificial");
            let title = title_expression
                .captures(&capture[2])
                .map(|value| value[1].trim().to_string())
                .unwrap_or_else(|| skill_name.to_string());
            graph.nodes.push(KnowledgeGraphNode {
                id: result_id.clone(),
                label: title,
                kind: layer.into(),
                meeting_id: meeting_id.clone(),
                path: Some(path.clone()),
                count: 1,
            });
            if !graph.nodes.iter().any(|node| node.id == skill_id) {
                graph.nodes.push(KnowledgeGraphNode {
                    id: skill_id.clone(),
                    label: skill_name.into(),
                    kind: "skill".into(),
                    meeting_id: None,
                    path: None,
                    count: 1,
                });
            }
            graph.edges.push(KnowledgeGraphEdge {
                id: stable_hash(&[&anchor, &result_id, "contains"]),
                source: anchor.clone(),
                target: result_id.clone(),
                kind: "contains".into(),
                weight: 1,
            });
            graph.edges.push(KnowledgeGraphEdge {
                id: stable_hash(&[&result_id, &skill_id, "generated_by"]),
                source: result_id.clone(),
                target: skill_id,
                kind: "generated_by".into(),
                weight: 1,
            });
            if let Some(raw) = metadata.get("context_documents") {
                for context in serde_json::from_str::<Vec<String>>(raw).unwrap_or_default() {
                    if let Some(target) = graph
                        .nodes
                        .iter()
                        .find(|node| {
                            node.meeting_id.as_deref() == Some(&context)
                                || node.path.as_deref() == Some(&context)
                        })
                        .map(|node| node.id.clone())
                    {
                        graph.edges.push(KnowledgeGraphEdge {
                            id: stable_hash(&[&result_id, &target, "used_context"]),
                            source: result_id.clone(),
                            target,
                            kind: "used_context".into(),
                            weight: 1,
                        });
                    }
                }
            }
        }
    }
}

#[tauri::command]
pub async fn api_get_knowledge_graph(
    state: State<'_, AppState>,
    meeting_id: Option<String>,
) -> Result<KnowledgeGraph, String> {
    let documents = sqlx::query_as::<_, KnowledgeDocument>(
        "SELECT path, meeting_id, kind, title, project, participants_json, tags_json, status, modified_ms
         FROM knowledge_documents ORDER BY modified_ms DESC",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    let link_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT source_path, target FROM knowledge_links ORDER BY rowid DESC",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    let tasks = sqlx::query_as::<_, KnowledgeTask>(
        "SELECT id, meeting_id, document_path, text, owner, completed, line_number
         FROM knowledge_tasks ORDER BY rowid DESC LIMIT 240",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    let decisions = sqlx::query_as::<_, KnowledgeDecision>(
        "SELECT id, meeting_id, document_path, text, line_number
         FROM knowledge_decisions ORDER BY rowid DESC LIMIT 240",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    let mut graph = build_knowledge_graph(
        &documents,
        &link_rows,
        &tasks,
        &decisions,
        meeting_id.as_deref(),
    );
    let contents = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT path, meeting_id, content FROM knowledge_documents WHERE kind = 'meeting'",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    augment_graph_with_skill_results(&mut graph, &contents);
    Ok(graph)
}

#[tauri::command]
pub async fn api_get_knowledge_dashboard(
    state: State<'_, AppState>,
) -> Result<KnowledgeDashboard, String> {
    let documents = sqlx::query_as::<_, KnowledgeDocument>(
        "SELECT path, meeting_id, kind, title, project, participants_json, tags_json, status, modified_ms
         FROM knowledge_documents ORDER BY modified_ms DESC",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    let open_tasks = sqlx::query_as::<_, KnowledgeTask>(
        "SELECT id, meeting_id, document_path, text, owner, completed, line_number
         FROM knowledge_tasks WHERE completed = 0 ORDER BY rowid DESC LIMIT 100",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    let recent_decisions = sqlx::query_as::<_, KnowledgeDecision>(
        "SELECT id, meeting_id, document_path, text, line_number
         FROM knowledge_decisions ORDER BY rowid DESC LIMIT 100",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    let projects = counted_values(
        documents
            .iter()
            .filter_map(|document| document.project.clone()),
    );
    let participants = counted_values(documents.iter().flat_map(|document| {
        serde_json::from_str::<Vec<String>>(&document.participants_json).unwrap_or_default()
    }));
    let tags = counted_values(documents.iter().flat_map(|document| {
        serde_json::from_str::<Vec<String>>(&document.tags_json).unwrap_or_default()
    }));
    Ok(KnowledgeDashboard {
        documents: documents.len() as i64,
        meetings: documents
            .iter()
            .filter(|document| document.kind == "meeting")
            .count() as i64,
        projects,
        participants,
        tags,
        open_tasks,
        recent_decisions,
    })
}

#[derive(Default)]
struct SearchFilters {
    project: Option<String>,
    person: Option<String>,
    tag: Option<String>,
    kind: Option<String>,
    has: Option<String>,
    terms: Vec<String>,
}

fn parse_search(query: &str) -> SearchFilters {
    let mut filters = SearchFilters::default();
    let tokenizer = Regex::new(r#"[^\s\"]+:\"[^\"]+\"|\S+"#).unwrap();
    for matched in tokenizer.find_iter(query) {
        let token = matched.as_str();
        let Some((key, value)) = token.split_once(':') else {
            filters.terms.push(token.trim_matches('"').to_lowercase());
            continue;
        };
        let value = value.trim_matches('"');
        match key.to_lowercase().as_str() {
            "project" | "projeto" => filters.project = Some(value.to_lowercase()),
            "person" | "pessoa" => filters.person = Some(value.to_lowercase()),
            "tag" => filters.tag = Some(value.to_lowercase()),
            "kind" | "tipo" => filters.kind = Some(value.to_lowercase()),
            "has" | "tem" => filters.has = Some(value.to_lowercase()),
            _ => filters.terms.push(token.to_lowercase()),
        }
    }
    filters
}

fn make_snippet(content: &str, terms: &[String]) -> String {
    let plain = content
        .lines()
        .filter(|line| !line.starts_with("---") && !line.starts_with("<!--"))
        .collect::<Vec<_>>()
        .join(" ");
    let lower = plain.to_lowercase();
    let position = terms
        .iter()
        .filter_map(|term| lower.find(term))
        .min()
        .unwrap_or_default();
    let start = position.saturating_sub(70);
    plain.chars().skip(start).take(220).collect()
}

#[tauri::command]
pub async fn api_search_knowledge(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<KnowledgeSearchResult>, String> {
    let filters = parse_search(&query);
    let rows = sqlx::query(
        "SELECT path, meeting_id, kind, title, project, participants_json, tags_json, content
         FROM knowledge_documents ORDER BY modified_ms DESC",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    use sqlx::Row;
    let mut results = Vec::new();
    for row in rows {
        let path: String = row.get("path");
        let meeting_id: Option<String> = row.get("meeting_id");
        let kind: String = row.get("kind");
        let title: String = row.get("title");
        let project: Option<String> = row.get("project");
        let participants: String = row.get("participants_json");
        let tags: String = row.get("tags_json");
        let content: String = row.get("content");
        let haystack =
            format!("{title} {project:?} {participants} {tags} {content}").to_lowercase();
        if filters.project.as_ref().is_some_and(|value| {
            !project
                .as_deref()
                .unwrap_or_default()
                .to_lowercase()
                .contains(value)
        }) || filters
            .person
            .as_ref()
            .is_some_and(|value| !participants.to_lowercase().contains(value))
            || filters
                .tag
                .as_ref()
                .is_some_and(|value| !tags.to_lowercase().contains(value))
            || filters
                .kind
                .as_ref()
                .is_some_and(|value| kind.to_lowercase() != *value)
            || filters.terms.iter().any(|term| !haystack.contains(term))
        {
            continue;
        }
        if let Some(has) = &filters.has {
            let (table, predicate) = match has.as_str() {
                "task" | "tarefa" => ("knowledge_tasks", ""),
                "open-task" => ("knowledge_tasks", " AND completed = 0"),
                "decision" | "decisao" | "decisão" => ("knowledge_decisions", ""),
                _ => ("", ""),
            };
            if !table.is_empty() {
                let sql =
                    format!("SELECT COUNT(*) FROM {table} WHERE document_path = ?{predicate}");
                let count: (i64,) = sqlx::query_as(&sql)
                    .bind(&path)
                    .fetch_one(state.db_manager.pool())
                    .await
                    .map_err(|error| error.to_string())?;
                if count.0 == 0 {
                    continue;
                }
            }
        }
        let score = filters
            .terms
            .iter()
            .map(|term| haystack.matches(term).count())
            .sum();
        results.push(KnowledgeSearchResult {
            path,
            meeting_id,
            kind,
            title,
            project,
            snippet: make_snippet(&content, &filters.terms),
            score,
        });
    }
    results.sort_by_key(|result| Reverse(result.score));
    results.truncate(100);
    Ok(results)
}

#[tauri::command]
pub async fn api_read_knowledge_document(
    state: State<'_, AppState>,
    path: String,
) -> Result<KnowledgeDocumentContent, String> {
    let row: Option<(String, String, String, String)> =
        sqlx::query_as("SELECT path, title, kind, content FROM knowledge_documents WHERE path = ?")
            .bind(&path)
            .fetch_optional(state.db_manager.pool())
            .await
            .map_err(|error| error.to_string())?;
    row.map(|(path, title, kind, content)| KnowledgeDocumentContent {
        path,
        title,
        kind,
        content,
    })
    .ok_or_else(|| "Documento não encontrado no índice".to_string())
}

fn link_targets_document(source_path: &str, target: &str, document_path: &str) -> bool {
    let decoded_target = target
        .split('#')
        .next()
        .unwrap_or_default()
        .replace("%20", " ")
        .trim_start_matches("./")
        .to_string();
    let target = decoded_target.to_lowercase();
    let path = Path::new(document_path);
    if target.contains('/') || target.ends_with(".md") {
        let resolved = Path::new(source_path)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(&decoded_target);
        if let (Ok(resolved), Ok(document)) = (resolved.canonicalize(), path.canonicalize()) {
            return resolved == document;
        }
    }
    let file = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    target == file || target == stem || target.ends_with(&format!("/{file}"))
}

#[tauri::command]
pub async fn api_get_related_meetings(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<RelatedMeeting>, String> {
    let documents = sqlx::query_as::<_, KnowledgeDocument>(
        "SELECT path, meeting_id, kind, title, project, participants_json, tags_json, status, modified_ms
         FROM knowledge_documents WHERE kind = 'meeting'",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    let Some(current) = documents
        .iter()
        .find(|document| document.meeting_id.as_deref() == Some(&meeting_id))
    else {
        return Ok(Vec::new());
    };
    let current_people =
        serde_json::from_str::<BTreeSet<String>>(&current.participants_json).unwrap_or_default();
    let current_tags =
        serde_json::from_str::<BTreeSet<String>>(&current.tags_json).unwrap_or_default();
    let link_rows: Vec<(String, String)> =
        sqlx::query_as("SELECT source_path, target FROM knowledge_links")
            .fetch_all(state.db_manager.pool())
            .await
            .map_err(|error| error.to_string())?;
    let mut related = Vec::new();
    for candidate in documents
        .iter()
        .filter(|document| document.meeting_id.as_deref() != Some(&meeting_id))
    {
        let mut reasons = Vec::new();
        let mut score = 0;
        let explicitly_linked = link_rows.iter().any(|(source, target)| {
            (source == &current.path && link_targets_document(source, target, &candidate.path))
                || (source == &candidate.path
                    && link_targets_document(source, target, &current.path))
        });
        if explicitly_linked {
            reasons.push("Link ou backlink explícito".into());
            score += 8;
        }
        if current.project.is_some() && current.project == candidate.project {
            reasons.push(format!(
                "Projeto {}",
                current.project.as_deref().unwrap_or_default()
            ));
            score += 5;
        }
        let people = serde_json::from_str::<BTreeSet<String>>(&candidate.participants_json)
            .unwrap_or_default();
        let shared_people = current_people
            .intersection(&people)
            .cloned()
            .collect::<Vec<_>>();
        if !shared_people.is_empty() {
            reasons.push(format!("Participantes: {}", shared_people.join(", ")));
            score += shared_people.len() * 2;
        }
        let tags =
            serde_json::from_str::<BTreeSet<String>>(&candidate.tags_json).unwrap_or_default();
        let shared_tags = current_tags
            .intersection(&tags)
            .cloned()
            .collect::<Vec<_>>();
        if !shared_tags.is_empty() {
            reasons.push(format!("Tags: {}", shared_tags.join(", ")));
            score += shared_tags.len();
        }
        if score > 0 {
            related.push(RelatedMeeting {
                meeting_id: candidate.meeting_id.clone().unwrap_or_default(),
                title: candidate.title.clone(),
                path: candidate.path.clone(),
                reasons,
                score,
            });
        }
    }
    related.sort_by_key(|meeting| Reverse(meeting.score));
    related.truncate(12);
    Ok(related)
}

async fn meeting_document_path(
    state: &State<'_, AppState>,
    meeting_id: &str,
) -> Result<PathBuf, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT path FROM knowledge_documents
         WHERE kind = 'meeting' AND meeting_id = ? LIMIT 1",
    )
    .bind(meeting_id)
    .fetch_optional(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    row.map(|value| PathBuf::from(value.0))
        .ok_or_else(|| "O meeting.md desta reunião ainda não foi indexado".to_string())
}

#[tauri::command]
pub async fn api_get_meeting_properties(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<MeetingProperties, String> {
    let path = meeting_document_path(&state, &meeting_id).await?;
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let (frontmatter, _) = split_frontmatter(&content);
    Ok(MeetingProperties {
        project: frontmatter.project,
        participants: frontmatter.participants,
        tags: frontmatter.tags,
        status: frontmatter.status,
    })
}

fn yaml_key(key: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(key.to_string())
}

#[tauri::command]
pub async fn api_update_meeting_properties(
    state: State<'_, AppState>,
    meeting_id: String,
    properties: MeetingProperties,
) -> Result<(), String> {
    let path = meeting_document_path(&state, &meeting_id).await?;
    let content = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let Some(rest) = content.strip_prefix("---\n") else {
        return Err("O meeting.md não possui frontmatter válido".into());
    };
    let Some(end) = rest.find("\n---") else {
        return Err("O frontmatter do meeting.md não foi encerrado".into());
    };
    let yaml = &rest[..end];
    let body = rest
        .get(end + 4..)
        .unwrap_or_default()
        .trim_start_matches('\n');
    let mut metadata: serde_yaml::Mapping =
        serde_yaml::from_str(yaml).map_err(|error| error.to_string())?;
    metadata.insert(
        yaml_key("empathy_schema"),
        serde_yaml::Value::Number(2.into()),
    );
    metadata.insert(
        yaml_key("type"),
        serde_yaml::Value::String("meeting".into()),
    );
    metadata.insert(
        yaml_key("project"),
        serde_yaml::to_value(properties.project.unwrap_or_default())
            .map_err(|error| error.to_string())?,
    );
    metadata.insert(
        yaml_key("participants"),
        serde_yaml::to_value(properties.participants).map_err(|error| error.to_string())?,
    );
    metadata.insert(
        yaml_key("tags"),
        serde_yaml::to_value(properties.tags).map_err(|error| error.to_string())?,
    );
    metadata.insert(
        yaml_key("status"),
        serde_yaml::to_value(properties.status.unwrap_or_else(|| "active".into()))
            .map_err(|error| error.to_string())?,
    );
    metadata.insert(
        yaml_key("updated_at"),
        serde_yaml::Value::String(Utc::now().to_rfc3339()),
    );
    let yaml = serde_yaml::to_string(&metadata).map_err(|error| error.to_string())?;
    let updated = format!("---\n{}---\n\n{}", yaml, body);
    let parent = path
        .parent()
        .ok_or_else(|| "Caminho inválido".to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    use std::io::Write;
    temporary
        .write_all(updated.as_bytes())
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(&path)
        .map_err(|error| error.error.to_string())?;
    crate::meeting_files::mark_meeting_written(parent, &Utc::now().to_rfc3339())
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn slugify(value: &str) -> String {
    let mut slug = value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug.trim_matches('-').chars().take(80).collect()
}

#[tauri::command]
pub async fn api_save_web_context<R: Runtime>(
    app: AppHandle<R>,
    input: WebContextInput,
) -> Result<String, String> {
    if input.title.trim().is_empty() || input.url.trim().is_empty() {
        return Err("Título e URL são obrigatórios".into());
    }
    if input.title.chars().count() > 200 || input.content.len() > 2_000_000 || input.tags.len() > 50
    {
        return Err("O contexto excede os limites de título, conteúdo ou tags".into());
    }
    let parsed =
        url::Url::parse(&input.url).map_err(|_| "A URL informada é inválida".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Somente URLs HTTP ou HTTPS são aceitas".into());
    }
    let root = workspace_root(&app).await?;
    let context_dir = root.join("Contextos");
    fs::create_dir_all(&context_dir).map_err(|error| error.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let path = context_dir.join(format!("{}-{}.md", slugify(&input.title), &id[..8]));
    let tags = if input.tags.is_empty() {
        vec!["contexto".to_string()]
    } else {
        input.tags
    };
    let markdown = format!(
        "---\nempathy_schema: 2\nid: {:?}\ntype: context\ntitle: {:?}\nsource_url: {:?}\ncaptured_at: {:?}\nproject: {:?}\ntags: {}\n---\n\n# {}\n\nFonte: [{}]({})\n\n{}\n",
        id,
        input.title,
        input.url,
        Utc::now().to_rfc3339(),
        input.project.unwrap_or_default(),
        serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into()),
        input.title,
        input.url,
        input.url,
        input.content.trim()
    );
    fs::write(&path, markdown).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn imported_markdown(source: &Path, content: &str, id: &str) -> String {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let title = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Importado");
    let body = if matches!(extension, "html" | "htm") {
        let scripts = Regex::new(r"(?is)<(script|style)[^>]*>.*?</(script|style)>").unwrap();
        let tags = Regex::new(r"(?s)<[^>]+>").unwrap();
        tags.replace_all(&scripts.replace_all(content, ""), " ")
            .to_string()
    } else if matches!(extension, "vtt" | "srt") {
        let timing = Regex::new(
            r"(?m)^\s*(?:\d+\s*$|(?:\d{2}:)?\d{2}:\d{2}[.,]\d{3}\s+-->\s+(?:\d{2}:)?\d{2}:\d{2}[.,]\d{3}.*$|WEBVTT\s*$)",
        )
        .unwrap();
        timing.replace_all(content, "").trim().to_string()
    } else if extension == "csv" {
        let rows = content
            .lines()
            .map(|line| {
                format!(
                    "| {} |",
                    line.split(',')
                        .map(|cell| cell.trim().replace('|', "\\|"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                )
            })
            .collect::<Vec<_>>();
        if let Some(first) = rows.first() {
            let columns = first.matches('|').count().saturating_sub(1);
            let separator = format!("|{}|", vec![" --- "; columns].join("|"));
            let mut table = rows;
            table.insert(1, separator);
            table.join("\n")
        } else {
            String::new()
        }
    } else if extension == "json" {
        let formatted = serde_json::from_str::<Value>(content)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok())
            .unwrap_or_else(|| content.to_string());
        format!("```json\n{}\n```", formatted)
    } else {
        content.to_string()
    };
    format!(
        "---\nempathy_schema: 2\nid: {:?}\ntype: imported\ntitle: {:?}\nsource_path: {:?}\nimported_at: {:?}\ntags: [importado]\n---\n\n# {}\n\n{}\n",
        id,
        title,
        source.to_string_lossy(),
        Utc::now().to_rfc3339(),
        title,
        body.trim()
    )
}

#[tauri::command]
pub async fn api_import_knowledge_folder<R: Runtime>(
    app: AppHandle<R>,
    source_path: String,
) -> Result<Vec<String>, String> {
    let source = PathBuf::from(&source_path);
    if !source.is_dir() {
        return Err("Selecione uma pasta válida para importar".into());
    }
    let root = workspace_root(&app).await?;
    let source_canonical = source.canonicalize().map_err(|error| error.to_string())?;
    let root_canonical = root.canonicalize().map_err(|error| error.to_string())?;
    if root_canonical.starts_with(&source_canonical) {
        return Err("A pasta importada não pode conter o workspace atual".into());
    }
    let batch_id = uuid::Uuid::new_v4().to_string();
    let destination = root.join("Importados").join(format!(
        "{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        &batch_id[..8]
    ));
    fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    let mut imported = Vec::new();
    let mut visited_files = 0usize;
    for entry in WalkDir::new(&source)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        visited_files += 1;
        if visited_files > 10_000 {
            return Err("A importação excede o limite de 10.000 arquivos".into());
        }
        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_lowercase();
        if !SUPPORTED_IMPORT_EXTENSIONS.contains(&extension.as_str())
            && !SUPPORTED_ATTACHMENT_EXTENSIONS.contains(&extension.as_str())
        {
            continue;
        }
        let size = entry.metadata().map_err(|error| error.to_string())?.len();
        if SUPPORTED_IMPORT_EXTENSIONS.contains(&extension.as_str()) && size > 20_000_000 {
            return Err(format!(
                "{} excede o limite de 20 MB para texto",
                entry.path().display()
            ));
        }
        if SUPPORTED_ATTACHMENT_EXTENSIONS.contains(&extension.as_str()) && size > 500_000_000 {
            return Err(format!(
                "{} excede o limite de 500 MB para anexos",
                entry.path().display()
            ));
        }
        let relative = entry
            .path()
            .strip_prefix(&source)
            .map_err(|error| error.to_string())?;
        let mut path = destination.join(relative);
        if extension == "markdown" {
            path.set_extension("md");
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        if SUPPORTED_ATTACHMENT_EXTENSIONS.contains(&extension.as_str()) {
            fs::copy(entry.path(), &path).map_err(|error| error.to_string())?;
            continue;
        }
        let content = fs::read_to_string(entry.path()).map_err(|error| error.to_string())?;
        let id = uuid::Uuid::new_v4().to_string();
        if matches!(extension.as_str(), "md" | "markdown") {
            fs::write(&path, content).map_err(|error| error.to_string())?;
        } else {
            path.set_extension("md");
            fs::write(&path, imported_markdown(entry.path(), &content, &id))
                .map_err(|error| error.to_string())?;
        }
        imported.push(path.to_string_lossy().to_string());
    }
    Ok(imported)
}

#[tauri::command]
pub async fn api_export_json_canvas<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    project: Option<String>,
) -> Result<String, String> {
    let documents = sqlx::query_as::<_, KnowledgeDocument>(
        "SELECT path, meeting_id, kind, title, project, participants_json, tags_json, status, modified_ms
         FROM knowledge_documents WHERE kind = 'meeting' ORDER BY modified_ms",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?
    .into_iter()
    .filter(|document| project.as_ref().is_none_or(|wanted| document.project.as_ref() == Some(wanted)))
    .collect::<Vec<_>>();
    let root = workspace_root(&app).await?;
    let nodes = documents
        .iter()
        .enumerate()
        .map(|(index, document)| {
            let file = Path::new(&document.path)
                .strip_prefix(&root)
                .map(|relative| Path::new("..").join(relative))
                .unwrap_or_else(|_| PathBuf::from(&document.path));
            json!({
                "id": stable_hash(&[&document.path])[..16].to_string(),
                "type": "file",
                "file": file.to_string_lossy(),
                "x": ((index % 4) * 420) as i64,
                "y": ((index / 4) * 260) as i64,
                "width": 360,
                "height": 220
            })
        })
        .collect::<Vec<_>>();
    let edges = documents
        .windows(2)
        .enumerate()
        .map(|(index, pair)| {
            json!({
                "id": stable_hash(&["canvas-edge", &index.to_string()])[..16].to_string(),
                "fromNode": stable_hash(&[&pair[0].path])[..16].to_string(),
                "toNode": stable_hash(&[&pair[1].path])[..16].to_string(),
                "fromSide": "right",
                "toSide": "left"
            })
        })
        .collect::<Vec<_>>();
    let canvas = json!({ "nodes": nodes, "edges": edges });
    let canvas_dir = root.join("Canvases");
    fs::create_dir_all(&canvas_dir).map_err(|error| error.to_string())?;
    let name = project
        .as_deref()
        .map(slugify)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "todas-as-reunioes".into());
    let path = canvas_dir.join(format!("{name}.canvas"));
    fs::write(
        &path,
        serde_json::to_string_pretty(&canvas).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn supported_extension_action(action: &str) -> bool {
    matches!(action, "reindex" | "export_canvas" | "create_digest")
}

#[tauri::command]
pub async fn api_discover_extensions<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<Vec<KnowledgeExtension>, String> {
    let root = workspace_root(&app).await?;
    let extensions_dir = root.join(".empathy").join("extensions");
    fs::create_dir_all(&extensions_dir).map_err(|error| error.to_string())?;
    sqlx::query("DELETE FROM knowledge_extensions")
        .execute(state.db_manager.pool())
        .await
        .map_err(|error| error.to_string())?;
    for entry in WalkDir::new(&extensions_dir)
        .max_depth(2)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_name() != "manifest.json" || !entry.file_type().is_file() {
            continue;
        }
        let content = fs::read_to_string(entry.path()).map_err(|error| error.to_string())?;
        if content.len() > 65_536 {
            return Err(format!(
                "Manifesto muito grande em {}",
                entry.path().display()
            ));
        }
        let manifest: ExtensionManifest = serde_json::from_str(&content).map_err(|error| {
            format!("Manifesto inválido em {}: {error}", entry.path().display())
        })?;
        if !supported_extension_action(&manifest.action) {
            return Err(format!(
                "A extensão {} solicitou uma ação não permitida",
                manifest.id
            ));
        }
        sqlx::query(
            "INSERT INTO knowledge_extensions
             (id, name, description, action, config_json, enabled, source_path)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&manifest.id)
        .bind(&manifest.name)
        .bind(&manifest.description)
        .bind(&manifest.action)
        .bind(serde_json::to_string(&manifest.config).unwrap_or_else(|_| "{}".into()))
        .bind(manifest.enabled)
        .bind(entry.path().to_string_lossy().to_string())
        .execute(state.db_manager.pool())
        .await
        .map_err(|error| error.to_string())?;
    }
    sqlx::query_as::<_, KnowledgeExtension>(
        "SELECT id, name, description, action, config_json, enabled, source_path
         FROM knowledge_extensions ORDER BY name",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())
}

async fn create_daily_digest<R: Runtime>(
    app: &AppHandle<R>,
    state: &State<'_, AppState>,
) -> Result<String, String> {
    let tasks = sqlx::query_as::<_, KnowledgeTask>(
        "SELECT id, meeting_id, document_path, text, owner, completed, line_number
         FROM knowledge_tasks WHERE completed = 0 ORDER BY rowid DESC LIMIT 100",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    let decisions = sqlx::query_as::<_, KnowledgeDecision>(
        "SELECT id, meeting_id, document_path, text, line_number
         FROM knowledge_decisions ORDER BY rowid DESC LIMIT 50",
    )
    .fetch_all(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?;
    let root = workspace_root(app).await?;
    let digest_dir = root.join("Digests");
    fs::create_dir_all(&digest_dir).map_err(|error| error.to_string())?;
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let path = digest_dir.join(format!("{date}.md"));
    let mut markdown = format!(
        "---\nempathy_schema: 2\ntype: digest\ntitle: {:?}\ndate: {:?}\ntags: [digest]\n---\n\n# Digest de {}\n\n## Ações pendentes\n\n",
        format!("Digest de {date}"),
        date,
        date
    );
    if tasks.is_empty() {
        markdown.push_str("_Nenhuma ação pendente._\n");
    }
    for task in tasks {
        let owner = task
            .owner
            .map(|value| format!(" **{}:**", value))
            .unwrap_or_default();
        markdown.push_str(&format!("- [ ]{} {}\n", owner, task.text));
    }
    markdown.push_str("\n## Decisões recentes\n\n");
    if decisions.is_empty() {
        markdown.push_str("_Nenhuma decisão indexada._\n");
    }
    for decision in decisions {
        markdown.push_str(&format!("- {}\n", decision.text));
    }
    fs::write(&path, markdown).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn api_set_extension_enabled<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    extension_id: String,
    enabled: bool,
) -> Result<(), String> {
    let source_path: Option<(String,)> =
        sqlx::query_as("SELECT source_path FROM knowledge_extensions WHERE id = ?")
            .bind(&extension_id)
            .fetch_optional(state.db_manager.pool())
            .await
            .map_err(|error| error.to_string())?;
    let Some((source_path,)) = source_path else {
        return Err("Extensão não encontrada".into());
    };
    let source = PathBuf::from(&source_path);
    let metadata = fs::symlink_metadata(&source).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("Manifestos de extensão não podem ser links simbólicos".into());
    }
    let root = workspace_root(&app)
        .await?
        .join(".empathy")
        .join("extensions")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let canonical_source = source.canonicalize().map_err(|error| error.to_string())?;
    if !canonical_source.starts_with(root) {
        return Err("O manifesto está fora da pasta segura de extensões".into());
    }
    let content = fs::read_to_string(&source_path).map_err(|error| error.to_string())?;
    let mut manifest: ExtensionManifest =
        serde_json::from_str(&content).map_err(|error| error.to_string())?;
    manifest.enabled = enabled;
    fs::write(
        &source_path,
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    sqlx::query("UPDATE knowledge_extensions SET enabled = ? WHERE id = ?")
        .bind(enabled)
        .bind(extension_id)
        .execute(state.db_manager.pool())
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn api_run_extension<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    extension_id: String,
) -> Result<String, String> {
    let extension = sqlx::query_as::<_, KnowledgeExtension>(
        "SELECT id, name, description, action, config_json, enabled, source_path
         FROM knowledge_extensions WHERE id = ?",
    )
    .bind(extension_id)
    .fetch_optional(state.db_manager.pool())
    .await
    .map_err(|error| error.to_string())?
    .ok_or_else(|| "Extensão não encontrada".to_string())?;
    if !extension.enabled {
        return Err("Ative a extensão antes de executá-la".into());
    }
    match extension.action.as_str() {
        "reindex" => api_reindex_knowledge(app, state)
            .await
            .map(|result| result.root),
        "export_canvas" => {
            let config: Value = serde_json::from_str(&extension.config_json).unwrap_or_default();
            let project = config
                .get("project")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            api_export_json_canvas(app, state, project).await
        }
        "create_digest" => create_daily_digest(&app, &state).await,
        _ => Err("Ação de extensão não permitida".into()),
    }
}

#[tauri::command]
pub async fn api_start_knowledge_watcher<R: Runtime>(
    app: AppHandle<R>,
    watcher_state: State<'_, KnowledgeWatcherState>,
) -> Result<String, String> {
    let root = workspace_root(&app).await?;
    let event_app = app.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            if event.paths.iter().any(|path| {
                matches!(
                    path.extension().and_then(|value| value.to_str()),
                    Some("md" | "canvas" | "json")
                )
            }) {
                let _ = event_app.emit("knowledge-files-changed", event.paths);
            }
        }
    })
    .map_err(|error| error.to_string())?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| error.to_string())?;
    *watcher_state.0.lock().map_err(|error| error.to_string())? = Some(watcher);
    Ok(root.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_filters() {
        let parsed = parse_search("project:\"Empathy IA\" person:Gabriel has:decision atualização");
        assert_eq!(parsed.project.as_deref(), Some("empathy ia"));
        assert_eq!(parsed.person.as_deref(), Some("gabriel"));
        assert_eq!(parsed.has.as_deref(), Some("decision"));
        assert_eq!(parsed.terms, vec!["atualização"]);
    }

    #[test]
    fn extracts_portable_links_tasks_and_decisions() {
        let body = "[Plano](../plano.md) e [[Outra reunião]]\n\n## Decisões\n- Publicar a versão\n\n## Ações\n- [ ] Gabriel: revisar o release\n";
        assert_eq!(extract_links(body).len(), 2);
        let (tasks, decisions) = extract_tasks_and_decisions(body);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].owner.as_deref(), Some("Gabriel"));
        assert_eq!(decisions.len(), 1);
    }

    #[test]
    fn indexes_schema_v2_properties() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("meeting.md");
        fs::write(
            &path,
            "---\nempathy_schema: 2\ntype: meeting\nid: meeting-1\ntitle: Produto\nproject: EmpathyIA\nparticipants: [Gabriel, Maria]\ntags: [produto]\nstatus: active\n---\n\n# Produto\n\n## Decisões\n- Publicar\n\n## Ações\n- [ ] Maria: revisar\n",
        )
        .unwrap();
        let document = index_file(&path).unwrap();
        assert_eq!(document.meeting_id.as_deref(), Some("meeting-1"));
        assert_eq!(document.project.as_deref(), Some("EmpathyIA"));
        assert_eq!(document.participants, vec!["Gabriel", "Maria"]);
        assert_eq!(document.tasks.len(), 1);
        assert_eq!(document.decisions.len(), 1);
    }

    #[test]
    fn builds_scoped_graph_from_rebuildable_index() {
        let documents = vec![
            KnowledgeDocument {
                path: "/workspace/product/meeting.md".into(),
                meeting_id: Some("meeting-1".into()),
                kind: "meeting".into(),
                title: "Produto".into(),
                project: Some("EmpathyIA".into()),
                participants_json: r#"["Gabriel"]"#.into(),
                tags_json: r#"["release"]"#.into(),
                status: Some("completed".into()),
                modified_ms: 2,
            },
            KnowledgeDocument {
                path: "/workspace/product/transcript.md".into(),
                meeting_id: Some("meeting-1".into()),
                kind: "transcript".into(),
                title: "Transcrição — Produto".into(),
                project: None,
                participants_json: "[]".into(),
                tags_json: "[]".into(),
                status: None,
                modified_ms: 1,
            },
        ];
        let tasks = vec![KnowledgeTask {
            id: "task-1".into(),
            meeting_id: Some("meeting-1".into()),
            document_path: "/workspace/product/meeting.md".into(),
            text: "Revisar release".into(),
            owner: Some("Gabriel".into()),
            completed: false,
            line_number: 12,
        }];
        let graph = build_knowledge_graph(&documents, &[], &tasks, &[], Some("meeting-1"));
        assert!(graph.nodes.iter().any(|node| node.kind == "meeting"));
        assert!(graph.nodes.iter().any(|node| node.kind == "transcript"));
        assert!(graph.nodes.iter().any(|node| node.kind == "project"));
        assert!(graph.nodes.iter().any(|node| node.kind == "person"));
        assert!(graph.nodes.iter().any(|node| node.kind == "task"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "contains"));
    }

    #[test]
    fn portable_person_document_enriches_the_existing_participant_node() {
        let documents = vec![
            KnowledgeDocument {
                path: "/workspace/meeting/meeting.md".into(),
                meeting_id: Some("meeting-1".into()),
                kind: "meeting".into(),
                title: "Produto".into(),
                project: None,
                participants_json: r#"["Maria"]"#.into(),
                tags_json: "[]".into(),
                status: Some("active".into()),
                modified_ms: 1,
            },
            KnowledgeDocument {
                path: "/workspace/People/person-1.md".into(),
                meeting_id: None,
                kind: "person".into(),
                title: "Maria".into(),
                project: None,
                participants_json: "[]".into(),
                tags_json: r#"["person"]"#.into(),
                status: Some("active".into()),
                modified_ms: 2,
            },
        ];
        let graph = build_knowledge_graph(&documents, &[], &[], &[], None);
        let people = graph
            .nodes
            .iter()
            .filter(|node| node.kind == "person" && node.label == "Maria")
            .collect::<Vec<_>>();
        assert_eq!(people.len(), 1);
        assert_eq!(
            people[0].path.as_deref(),
            Some("/workspace/People/person-1.md")
        );
        assert!(graph
            .edges
            .iter()
            .any(|edge| { edge.kind == "participant" && edge.target == people[0].id }));
    }

    #[test]
    fn skill_results_extend_rebuildable_graph() {
        let path = "/workspace/note/meeting.md".to_string();
        let document = KnowledgeDocument {
            path: path.clone(),
            meeting_id: Some("note-1".into()),
            kind: "meeting".into(),
            title: "Nota".into(),
            project: None,
            participants_json: "[]".into(),
            tags_json: "[]".into(),
            status: None,
            modified_ms: 1,
        };
        let mut graph = build_knowledge_graph(&[document], &[], &[], &[], None);
        let markdown = "<!-- empathy-skill-result\nid: result-1\nskill_id: connect-memory\nskill_name: Conectar com a memória\nlayer: collective\ncontext_documents: [\"note-1\"]\n-->\n## Conexões\n\nTexto\n<!-- /empathy-skill-result -->".to_string();
        augment_graph_with_skill_results(&mut graph, &[(path, Some("note-1".into()), markdown)]);
        assert!(graph.nodes.iter().any(|node| node.kind == "collective"));
        assert!(graph.nodes.iter().any(|node| node.kind == "skill"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "generated_by"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "used_context"));
    }

    #[test]
    fn converts_caption_and_csv_imports_to_markdown() {
        let caption = imported_markdown(
            Path::new("meeting.vtt"),
            "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nOlá equipe",
            "id-1",
        );
        assert!(caption.contains("Olá equipe"));
        assert!(!caption.contains("-->"));
        let csv = imported_markdown(Path::new("tasks.csv"), "owner,task\nAna,Revisar", "id-2");
        assert!(csv.contains("| owner | task |"));
        assert!(csv.contains("| Ana | Revisar |"));
    }
}
