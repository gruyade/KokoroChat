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

/// LLMプロバイダー種別
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LLMProvider {
    Openai,
    Anthropic,
    Google,
    OpenaiCompatible,
}

/// LLMモデル接続設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettings {
    /// プロバイダー種別（後方互換のためOption型）
    #[serde(default)]
    pub provider: Option<LLMProvider>,
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
    /// 自動削除閾値（分）。0 = 無効（全保持）
    #[serde(default = "default_thought_auto_delete_threshold")]
    pub auto_delete_threshold_minutes: u64,
}

fn default_thought_auto_delete_threshold() -> u64 {
    1440 // 24時間
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
    /// VoicePeak CLI実行ファイルパス（例: "C:\\Program Files\\VOICEPEAK\\voicepeak.exe"）
    #[serde(default)]
    pub voicepeak_path: Option<String>,
    /// TTS生成タイムアウト（秒）。デフォルト: 60
    #[serde(default = "default_tts_timeout")]
    pub timeout_seconds: u64,
    /// テキスト分割の最大チャンクサイズ（文字数）。デフォルト: 140
    #[serde(default = "default_max_chunk_size")]
    pub max_chunk_size: usize,
    /// IrodoriTTSデフォルトベースURL（後方互換用）
    #[serde(default)]
    pub irodori_base_url: Option<String>,
    /// IrodoriTTS キャプションモード用ベースURL
    #[serde(default)]
    pub irodori_caption_base_url: Option<String>,
    /// IrodoriTTS 参照音源モード用ベースURL
    #[serde(default)]
    pub irodori_reference_audio_base_url: Option<String>,
}

fn default_tts_timeout() -> u64 {
    60
}

fn default_max_chunk_size() -> usize {
    140
}

/// メッセージ送信キー設定
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SendKey {
    #[default]
    Enter,
    CtrlEnter,
    ShiftEnter,
}

/// UI設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    pub theme: Theme,
    pub language: String,
    /// メッセージ送信キー設定
    #[serde(default)]
    pub send_key: SendKey,
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
