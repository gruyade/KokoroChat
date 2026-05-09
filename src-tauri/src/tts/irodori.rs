// Irodori-TTS API実装

use reqwest::Client;
use serde::Serialize;

use crate::error::AppError;
use crate::models::config::TTSGlobalConfig;
use crate::models::tts::{IrodoriMode, TTSConfig};

/// IrodoriTTSのベースURLを解決する
/// 優先順位: グローバル設定のモード別URL > キャラクター個別(モード別) > キャラクター個別(共通) > グローバル共通URL
pub fn resolve_irodori_base_url(
    char_config: &TTSConfig,
    global_config: &TTSGlobalConfig,
) -> Option<String> {
    // 1. グローバル設定のモード別URL（キャラクターのモード選択に基づく）
    let global_mode_url = match char_config.irodori_mode {
        Some(IrodoriMode::Caption) => global_config.irodori_caption_base_url.clone(),
        Some(IrodoriMode::ReferenceAudio) => global_config.irodori_reference_audio_base_url.clone(),
        None => None,
    };
    if global_mode_url.is_some() {
        return global_mode_url;
    }

    // 2. キャラクター個別のモード別URL
    let char_mode_url = match char_config.irodori_mode {
        Some(IrodoriMode::Caption) => char_config.caption_base_url.clone(),
        Some(IrodoriMode::ReferenceAudio) => char_config.reference_audio_base_url.clone(),
        None => None,
    };
    if char_mode_url.is_some() {
        return char_mode_url;
    }

    // 3. キャラクター個別の共通ベースURL
    if char_config.base_url.is_some() {
        return char_config.base_url.clone();
    }

    // 4. グローバル設定の共通ベースURL
    global_config.irodori_base_url.clone()
}

/// Irodori-TTS APIリクエストボディ
#[derive(Debug, Clone, Serialize)]
pub(crate) struct IrodoriSynthesizeRequest {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_audio_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

/// Irodori-TTS APIハンドラ
pub struct IrodoriTTSHandler<'a> {
    http_client: &'a Client,
}

impl<'a> IrodoriTTSHandler<'a> {
    pub fn new(http_client: &'a Client) -> Self {
        Self { http_client }
    }

    /// TTSConfigからリクエストボディを構築
    pub(crate) fn build_request_body(text: &str, config: &TTSConfig) -> IrodoriSynthesizeRequest {
        IrodoriSynthesizeRequest {
            text: text.to_string(),
            reference_audio_path: config.reference_audio_path.clone(),
            caption: config.caption.clone(),
        }
    }

    /// 合成エンドポイントURLを構築
    fn build_synthesize_url(config: &TTSConfig) -> String {
        let base = config.base_url.as_deref().unwrap_or("").trim_end_matches('/');
        format!("{}/synthesize", base)
    }

    /// 音声合成リクエスト送信
    pub async fn synthesize(&self, text: &str, config: &TTSConfig) -> Result<Vec<u8>, AppError> {
        let url = Self::build_synthesize_url(config);
        let body = Self::build_request_body(text, config);

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Tts(format!("Irodori-TTS request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Tts(format!(
                "Irodori-TTS returned status {}: {}",
                status, error_body
            )));
        }

        let audio_bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::Tts(format!("Irodori-TTS response read failed: {}", e)))?;

        Ok(audio_bytes.to_vec())
    }

    /// 接続テスト（軽量リクエストで疎通確認）
    pub async fn test_connection(&self, config: &TTSConfig) -> Result<(), AppError> {
        let url = Self::build_synthesize_url(config);
        let body = Self::build_request_body("test", config);

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Tts(format!("Irodori-TTS connection test failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Tts(format!(
                "Irodori-TTS connection test failed (status {}): {}",
                status, error_body
            )));
        }

        Ok(())
    }
}
