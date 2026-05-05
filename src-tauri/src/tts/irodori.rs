// Irodori-TTS API実装

use reqwest::Client;
use serde::Serialize;

use crate::error::AppError;
use crate::models::tts::TTSConfig;

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
        let base = config.base_url.trim_end_matches('/');
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
