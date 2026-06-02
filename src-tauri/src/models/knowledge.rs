use serde::{Deserialize, Serialize};

/// DB上のKnowledge_Entry完全表現
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: String,
    pub session_id: String,
    pub file_name: String,
    pub content: String,
    pub size_bytes: i64,
    pub enabled: bool,
    pub injection_mode: String,
    pub created_at: String,
}

/// フロントエンド向け軽量表現（content除外）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntryMeta {
    pub id: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub enabled: bool,
    pub injection_mode: String,
    pub created_at: String,
}
