-- Rebuildable knowledge index. Markdown files remain the source of truth.
CREATE TABLE IF NOT EXISTS knowledge_documents (
    path TEXT PRIMARY KEY,
    meeting_id TEXT,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    project TEXT,
    participants_json TEXT NOT NULL DEFAULT '[]',
    tags_json TEXT NOT NULL DEFAULT '[]',
    status TEXT,
    content TEXT NOT NULL,
    modified_ms INTEGER NOT NULL,
    indexed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_knowledge_documents_meeting_id
    ON knowledge_documents(meeting_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_documents_project
    ON knowledge_documents(project);
CREATE INDEX IF NOT EXISTS idx_knowledge_documents_kind
    ON knowledge_documents(kind);

CREATE TABLE IF NOT EXISTS knowledge_links (
    source_path TEXT NOT NULL,
    target TEXT NOT NULL,
    label TEXT,
    PRIMARY KEY (source_path, target),
    FOREIGN KEY (source_path) REFERENCES knowledge_documents(path) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_knowledge_links_target ON knowledge_links(target);

CREATE TABLE IF NOT EXISTS knowledge_tasks (
    id TEXT PRIMARY KEY,
    meeting_id TEXT,
    document_path TEXT NOT NULL,
    text TEXT NOT NULL,
    owner TEXT,
    completed INTEGER NOT NULL DEFAULT 0,
    line_number INTEGER NOT NULL,
    FOREIGN KEY (document_path) REFERENCES knowledge_documents(path) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_knowledge_tasks_completed ON knowledge_tasks(completed);
CREATE INDEX IF NOT EXISTS idx_knowledge_tasks_meeting_id ON knowledge_tasks(meeting_id);

CREATE TABLE IF NOT EXISTS knowledge_decisions (
    id TEXT PRIMARY KEY,
    meeting_id TEXT,
    document_path TEXT NOT NULL,
    text TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    FOREIGN KEY (document_path) REFERENCES knowledge_documents(path) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_knowledge_decisions_meeting_id
    ON knowledge_decisions(meeting_id);

CREATE TABLE IF NOT EXISTS knowledge_extensions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    action TEXT NOT NULL,
    config_json TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 0,
    source_path TEXT NOT NULL
);
