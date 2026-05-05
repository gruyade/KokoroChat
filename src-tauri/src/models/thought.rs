use serde::{Deserialize, Serialize};

/// キャラクターの独自思考
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thought {
    pub id: String,
    pub character_id: String,
    pub content: String,
    /// 思考生成時の参照コンテキスト概要
    pub context: Option<String>,
    pub created_at: String,
}
