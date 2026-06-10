// Database migrations - スキーマ作成

/// 全テーブル作成SQLを返す。
/// IF NOT EXISTS を使用し、冪等に実行可能。
pub fn create_tables_sql() -> &'static str {
    r#"
CREATE TABLE IF NOT EXISTS characters (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT NOT NULL,
  system_prompt TEXT NOT NULL,
  avatar_path TEXT,
  tts_config TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS chat_sessions (
  id TEXT PRIMARY KEY,
  character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  title TEXT,
  last_message_at TEXT,
  last_message_preview TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS chat_messages (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'spontaneous', 'tool')),
  content TEXT NOT NULL,
  attachments TEXT,
  tool_calls TEXT,
  tool_call_id TEXT,
  thinking_content TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS memories (
  id TEXT PRIMARY KEY,
  character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  content TEXT NOT NULL,
  source_session_id TEXT REFERENCES chat_sessions(id),
  source_message_from TEXT,
  source_message_to TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS thoughts (
  id TEXT PRIMARY KEY,
  character_id TEXT NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  content TEXT NOT NULL,
  context TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS plugins (
  name TEXT PRIMARY KEY,
  description TEXT NOT NULL,
  version TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  config TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS attachments (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
  file_name TEXT NOT NULL,
  attachment_type TEXT NOT NULL CHECK(attachment_type IN ('text', 'pdf', 'image')),
  file_path TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  extracted_text TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS chat_tool_permissions (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
  tool_name TEXT NOT NULL,
  is_enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  UNIQUE(session_id, tool_name)
);

CREATE TABLE IF NOT EXISTS custom_tools (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  tool_type TEXT NOT NULL CHECK(tool_type IN ('http', 'cli')),
  description TEXT NOT NULL,
  parameters_schema TEXT NOT NULL,
  config_json TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS chat_plugin_configs (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  plugin_name TEXT NOT NULL,
  config_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE,
  UNIQUE(session_id, plugin_name)
);

CREATE TABLE IF NOT EXISTS session_knowledge (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  file_name TEXT NOT NULL,
  content TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  injection_mode TEXT NOT NULL DEFAULT 'system_prompt'
    CHECK(injection_mode IN ('system_prompt', 'tool_reference')),
  created_at TEXT NOT NULL,
  FOREIGN KEY(session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE,
  UNIQUE(session_id, file_name)
);
"#
}

/// インデックス作成SQLを返す。
/// IF NOT EXISTS を使用し、冪等に実行可能。
pub fn create_indexes_sql() -> &'static str {
    r#"
CREATE INDEX IF NOT EXISTS idx_chat_sessions_character ON chat_sessions(character_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_memories_character ON memories(character_id);
CREATE INDEX IF NOT EXISTS idx_thoughts_character ON thoughts(character_id);
CREATE INDEX IF NOT EXISTS idx_attachments_message ON attachments(message_id);
CREATE INDEX IF NOT EXISTS idx_chat_tool_permissions_session ON chat_tool_permissions(session_id);
CREATE INDEX IF NOT EXISTS idx_custom_tools_name ON custom_tools(name);
CREATE INDEX IF NOT EXISTS idx_chat_plugin_configs_session ON chat_plugin_configs(session_id);
CREATE INDEX IF NOT EXISTS idx_session_knowledge_session ON session_knowledge(session_id);
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tables_sql_not_empty() {
        let sql = create_tables_sql();
        assert!(!sql.is_empty());
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS characters"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS chat_sessions"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS chat_messages"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS memories"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS thoughts"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS plugins"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS attachments"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS chat_tool_permissions"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS custom_tools"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS chat_plugin_configs"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS session_knowledge"));
    }

    #[test]
    fn test_create_indexes_sql_not_empty() {
        let sql = create_indexes_sql();
        assert!(!sql.is_empty());
        assert!(sql.contains("idx_chat_sessions_character"));
        assert!(sql.contains("idx_chat_messages_session"));
        assert!(sql.contains("idx_memories_character"));
        assert!(sql.contains("idx_thoughts_character"));
        assert!(sql.contains("idx_attachments_message"));
        assert!(sql.contains("idx_chat_tool_permissions_session"));
        assert!(sql.contains("idx_custom_tools_name"));
        assert!(sql.contains("idx_chat_plugin_configs_session"));
        assert!(sql.contains("idx_session_knowledge_session"));
    }

    #[test]
    fn test_tables_have_foreign_keys() {
        let sql = create_tables_sql();
        assert!(sql.contains("REFERENCES characters(id) ON DELETE CASCADE"));
        assert!(sql.contains("REFERENCES chat_sessions(id) ON DELETE CASCADE"));
        assert!(sql.contains("REFERENCES chat_messages(id) ON DELETE CASCADE"));
    }

    #[test]
    fn test_chat_messages_role_check_constraint() {
        let sql = create_tables_sql();
        assert!(sql.contains("CHECK(role IN ('user', 'assistant', 'spontaneous', 'tool'))"));
    }

    #[test]
    fn test_attachments_type_check_constraint() {
        let sql = create_tables_sql();
        assert!(sql.contains("CHECK(attachment_type IN ('text', 'pdf', 'image'))"));
    }

    #[test]
    fn test_session_knowledge_check_constraint() {
        let sql = create_tables_sql();
        assert!(sql.contains("CHECK(injection_mode IN ('system_prompt', 'tool_reference'))"));
    }

    #[test]
    fn test_session_knowledge_unique_constraint() {
        let sql = create_tables_sql();
        assert!(sql.contains("UNIQUE(session_id, file_name)"));
    }
}
