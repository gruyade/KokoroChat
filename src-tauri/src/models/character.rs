use serde::{Deserialize, Serialize};

use super::tts::TTSConfig;

/// AIキャラクター
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    /// UUID v4
    pub id: String,
    pub name: String,
    /// ユーザーが入力した概要説明
    pub description: String,
    /// LLM生成 or 手動編集されたシステムプロンプト
    pub system_prompt: String,
    pub avatar_path: Option<String>,
    pub tts_config: Option<TTSConfig>,
    /// ISO 8601
    pub created_at: String,
    /// ISO 8601
    pub updated_at: String,
}

/// キャラクター更新用（部分更新対応）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub avatar_path: Option<String>,
    pub tts_config: Option<TTSConfig>,
    /// trueの場合、avatar_pathをNULLに更新する
    pub clear_avatar: Option<bool>,
    /// trueの場合、tts_configをNULLに更新する
    pub clear_tts: Option<bool>,
}
