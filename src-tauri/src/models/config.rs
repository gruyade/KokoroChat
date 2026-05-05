use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// モデルの用途
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelPurpose {
    Chat,
    Memory,
    Thought,
    CharacterGeneration,
}

/// LLMモデル接続設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettings {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub temperature: f32,
}

/// アプリケーション全体設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub models: HashMap<ModelPurpose, ModelSettings>,
    pub spontaneous: SpontaneousConfig,
    pub thought: ThoughtConfig,
    pub memory: MemoryConfig,
    pub tts: TTSGlobalConfig,
    pub ui: UIConfig,
    pub plugins: PluginsConfig,
    pub attachment: AttachmentConfig,
}

/// 自発的発話設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpontaneousConfig {
    pub enabled: bool,
    pub min_interval_seconds: u64,
    /// 自発的発話の発生確率（0.0〜1.0、デフォルト0.3）
    #[serde(default = "default_spontaneous_probability")]
    pub probability: f32,
}

fn default_spontaneous_probability() -> f32 {
    0.3
}

/// 独自思考設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtConfig {
    pub enabled: bool,
    pub interval_minutes: u64,
}

/// 記憶管理設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 圧縮トリガーとなるメッセージ数閾値
    pub compression_threshold: u32,
}

/// TTS全体設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTSGlobalConfig {
    pub enabled: bool,
}

/// UI設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    pub theme: Theme,
    pub language: String,
}

/// テーマ
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

/// プラグイン設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    pub enabled_plugins: Vec<String>,
    /// プラグイン名 → 固有設定
    pub plugin_settings: HashMap<String, Value>,
}

/// 添付ファイル設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentConfig {
    /// デフォルト10MB
    pub max_file_size_bytes: u64,
    pub allowed_extensions: Vec<String>,
}
