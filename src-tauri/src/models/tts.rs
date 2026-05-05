use serde::{Deserialize, Serialize};

/// TTS音声合成プロバイダー
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TTSProvider {
    IrodoriTts,
    Voicepeak,
}

/// TTS設定（キャラクター個別）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTSConfig {
    pub provider: TTSProvider,
    pub base_url: String,
    /// Irodori-TTS: 参照音声ファイルパス
    pub reference_audio_path: Option<String>,
    /// Irodori-TTS: キャプション
    pub caption: Option<String>,
    /// VoicePeak: ナレーター名
    pub narrator: Option<String>,
    /// VoicePeak: 感情パラメータ
    pub emotion: Option<EmotionParams>,
    /// 読み上げ速度
    pub speed: Option<f32>,
    /// ピッチ
    pub pitch: Option<f32>,
}

/// VoicePeak感情パラメータ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionParams {
    pub happy: Option<i32>,
    pub fun: Option<i32>,
    pub angry: Option<i32>,
    pub sad: Option<i32>,
}
