// TTS Tauri Commands — 音声合成操作

use tauri::State;

use crate::error::AppError;
use crate::models::tts::TTSConfig;
use crate::state::AppState;
#[allow(unused_imports)]
use crate::tts::connector::TTSConnector;

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
    state.tts_connector.synthesize(&text, &config).await
}

/// TTS接続テスト
///
/// 指定TTSConfigでプロバイダーへの接続テストを実行する。
#[tauri::command]
pub async fn test_tts_connection(
    config: TTSConfig,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.tts_connector.test_connection(&config).await
}

// TTS音声イベント定義（チャットエンジンからTTS有効時にemitされる）
// app_handle.emit("tts:audio", TTSAudioEvent { data: base64_audio })
// 将来のチャット統合時に使用予定
