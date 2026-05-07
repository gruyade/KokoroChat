use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// TTS音声合成プロバイダー
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TTSProvider {
    IrodoriTts,
    Voicepeak,
}

/// Irodori-TTSの動作モード
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IrodoriMode {
    Caption,
    ReferenceAudio,
}

/// TTS設定（キャラクター個別）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTSConfig {
    pub provider: TTSProvider,
    /// Irodori-TTS用ベースURL（VoicePeakでは不使用）
    #[serde(default)]
    pub base_url: Option<String>,
    /// Irodori-TTS: 参照音声ファイルパス
    pub reference_audio_path: Option<String>,
    /// Irodori-TTS: キャプション
    pub caption: Option<String>,
    /// VoicePeak: ナレーター名
    pub narrator: Option<String>,
    /// VoicePeak: 感情パラメータ
    pub emotion: Option<EmotionParams>,
    /// 読み上げ速度（VoicePeak: 50〜200）
    pub speed: Option<f32>,
    /// ピッチ（VoicePeak: -300〜300）
    pub pitch: Option<f32>,
    /// Irodori-TTS: caption mode vs reference audio mode
    #[serde(default)]
    pub irodori_mode: Option<IrodoriMode>,
}

/// VoicePeak感情パラメータ（ナレーターごとに異なるキーを持つ）
pub type EmotionParams = HashMap<String, i32>;

/// TTS生成開始イベント
#[derive(Clone, Serialize)]
pub struct TTSGeneratingEvent {
    pub session_id: String,
}

/// TTS完了イベント（テキスト+音声）
#[derive(Clone, Serialize)]
pub struct TTSCompleteEvent {
    pub session_id: String,
    pub text: String,
    pub audio: String, // Base64エンコード
}

/// TTSエラーイベント（フォールバック時）
#[derive(Clone, Serialize)]
pub struct TTSErrorEvent {
    pub session_id: String,
    pub text: String,
    pub error: String,
}
