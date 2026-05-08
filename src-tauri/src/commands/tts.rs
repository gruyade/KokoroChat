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

/// 既存メッセージテキストからspeechを抽出してTTS音声を生成
///
/// LLMにテキストを送り、会話文（speech）のみを抽出させてからTTS音声合成を行う。
/// 返却値はbase64エンコードされたWAV音声データ。
#[tauri::command]
pub async fn generate_speech_for_message(
    text: String,
    character_id: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    use base64::Engine;
    use crate::db::repositories::character as char_repo;
    use crate::llm::client::{ChatMessage, LLMClientConfig, LLMResponse, MessageRole};
    use crate::models::config::ModelPurpose;

    // キャラクターのTTS設定を取得
    let tts_config = {
        let db = state.db.lock().map_err(|e| AppError::Database(format!("DB lock failed: {}", e)))?;
        let character = char_repo::get_character(db.connection(), &character_id)?
            .ok_or_else(|| AppError::NotFound(format!("Character not found: {}", character_id)))?;
        character.tts_config.ok_or_else(|| AppError::Validation("Character has no TTS config".to_string()))?
    };

    // LLMにspeech抽出を依頼
    let llm_config = state.config_manager
        .get_model_settings(&ModelPurpose::Chat)
        .map(|s| LLMClientConfig {
            base_url: s.base_url,
            model: s.model,
            api_key: s.api_key,
            temperature: s.temperature,
            provider: s.provider,
        })
        .unwrap_or(LLMClientConfig {
            base_url: String::new(),
            model: String::new(),
            api_key: None,
            temperature: 0.7,
            provider: None,
        });

    let messages = vec![
        ChatMessage {
            role: MessageRole::System,
            content: "以下のテキストから、声に出して話すセリフと心の声だけを抽出してください。動作描写、効果音、擬音語、ナレーション、状況説明は除外してください。抽出したテキストのみを返してください。説明や装飾は不要です。".to_string(),
            tool_call_id: None,
            images: None,
        },
        ChatMessage {
            role: MessageRole::User,
            content: text.clone(),
            tool_call_id: None,
            images: None,
        },
    ];

    let _llm_guard = state.llm_lock.lock().await;
    let response = state.llm_client.chat(&messages, &llm_config, None).await?;
    drop(_llm_guard);

    let speech_text = match response {
        LLMResponse::Text(t) => t.trim().to_string(),
        _ => text.clone(), // フォールバック: 全文使用
    };

    if speech_text.is_empty() {
        return Err(AppError::Tts("No speech text extracted".to_string()));
    }

    // TTS音声生成（イベント発行なし — フロントエンドが戻り値で直接再生）
    let app_tts_config = state.config_manager.get_config().tts.clone();
    let voicepeak_path = app_tts_config.voicepeak_path.as_deref();

    use crate::tts::text_splitter::{split_text, SplitConfig};
    use crate::tts::wav_concat::concatenate_wav;

    let split_config = SplitConfig { max_chunk_size: app_tts_config.max_chunk_size };
    let chunks = split_text(&speech_text, &split_config);

    if chunks.is_empty() {
        return Err(AppError::Tts("No text to synthesize".to_string()));
    }

    let mut audio_chunks: Vec<Vec<u8>> = Vec::new();
    for chunk in &chunks {
        let audio = state.tts_connector.synthesize(chunk, &tts_config, voicepeak_path).await?;
        audio_chunks.push(audio);
    }

    let audio_data = concatenate_wav(&audio_chunks)?;
    let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&audio_data);

    Ok(audio_base64)
}
