-- Copilot conversation threads
CREATE TABLE conversations (
    id         TEXT PRIMARY KEY,
    title      TEXT NOT NULL DEFAULT 'New conversation',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Individual messages within a thread
CREATE TABLE conversation_messages (
    id               TEXT PRIMARY KEY,
    conversation_id  TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role             TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
    content          TEXT NOT NULL,
    tool_trace       TEXT,               -- JSON array: ["spending_by_category", …]
    action_bundle_id TEXT,               -- nullable FK into action_bundles
    branch_parent_id TEXT,               -- ID of user message this branches from (nullable)
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_conversation_messages_conv ON conversation_messages(conversation_id);
CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC);
