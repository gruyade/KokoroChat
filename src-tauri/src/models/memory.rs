use serde::{Deserialize, Serialize};

/// 記憶（LLMによる会話要約）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub character_id: String,
    /// LLMによる要約テキスト
    pub content: String,
    pub source_session_id: Option<String>,
    pub source_message_from: Option<String>,
    pub source_message_to: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
