// TTS Tauri Commands — 音声合成操作

use tauri::State;

use crate::error::AppError;
use crate::models::tts::TTSConfig;
use crate::state::AppState;
#[allow(unused_imports)]
use crate::tts::connector::TTSConnector;
use crate::tts::voicepeak::VoicePeakHandler;

/// テキスト音声合成
///
/// 指定テキストをTTSConfigに基づいて音声合成し、音声バイトデータを返す。
/// 返却されるVec<u8>はTauriのシリアライズによりbase64エンコードされる。
#[tauri::command]
pub async fn synthesize_speech(
    text: String,
    config: TTSConfig,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, AppError> {
    let voicepeak_path = state.config_manager.get_config().tts.voicepeak_path;
    state.tts_connector.synthesize(&text, &config, voicepeak_path.as_deref()).await
}

/// TTS接続テスト
///
/// 指定TTSConfigでプロバイダーへの接続テストを実行する。
#[tauri::command]
pub async fn test_tts_connection(
    config: TTSConfig,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let voicepeak_path = state.config_manager.get_config().tts.voicepeak_path;
    state.tts_connector.test_connection(&config, voicepeak_path.as_deref()).await
}

/// VoicePeakナレーターの利用可能な感情リストを取得
#[tauri::command]
pub async fn list_voicepeak_emotions(
    narrator: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let voicepeak_path = state.config_manager.get_config().tts.voicepeak_path;
    VoicePeakHandler::list_emotions(voicepeak_path.as_deref(), &narrator).await
}

// TTS音声イベント定義（チャットエンジンからTTS有効時にemitされる）
// app_handle.emit("tts:audio", TTSAudioEvent { data: base64_audio })
// 将来のチャット統合時に使用予定
