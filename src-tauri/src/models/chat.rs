use serde::{Deserialize, Serialize};

use super::plugin::ToolCall;

/// チャットセッション
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub character_id: String,
    pub title: Option<String>,
    pub last_message_at: Option<String>,
    pub last_message_preview: Option<String>,
    pub created_at: String,
}

/// チャットメッセージレコード（DB保存用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRecord {
    pub id: String,
    pub session_id: String,
    pub role: ChatRole,
    pub content: String,
    /// 添付ファイル情報
    pub attachments: Option<Vec<MessageAttachment>>,
    /// tool_callリクエスト（role=assistant時）
    pub tool_calls: Option<Vec<ToolCall>>,
    /// tool結果のtool_call参照ID（role=tool時）
    pub tool_call_id: Option<String>,
    pub created_at: String,
}

/// チャットメッセージのロール
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    User,
    Assistant,
    Spontaneous,
    Tool,
}

/// メッセージに添付されたファイル情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAttachment {
    pub file_name: String,
    pub attachment_type: String,
    pub extracted_text: Option<String>,
    pub base64_data: Option<String>,
}
