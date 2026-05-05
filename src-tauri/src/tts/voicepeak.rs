// VoicePeak API実装

use reqwest::Client;
use serde::Serialize;

use crate::error::AppError;
use crate::models::tts::{EmotionParams, TTSConfig};

/// VoicePeak APIリクエストボディ
#[derive(Debug, Clone, Serialize)]
pub(crate) struct VoicePeakSynthesizeRequest {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub narrator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emotion: Option<VoicePeakEmotion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f32>,
}

/// VoicePeak感情パラメータ（リクエスト用）
#[derive(Debug, Clone, Serialize)]
pub(crate) struct VoicePeakEmotion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub happy: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fun: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angry: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sad: Option<i32>,
}

impl From<&EmotionParams> for VoicePeakEmotion {
    fn from(params: &EmotionParams) -> Self {
        Self {
            happy: params.happy,
            fun: params.fun,
            angry: params.angry,
            sad: params.sad,
        }
    }
}

/// VoicePeak APIハンドラ
pub struct VoicePeakHandler<'a> {
    http_client: &'a Client,
}

impl<'a> VoicePeakHandler<'a> {
    pub fn new(http_client: &'a Client) -> Self {
        Self { http_client }
    }

    /// TTSConfigからリクエストボディを構築
    pub(crate) fn build_request_body(text: &str, config: &TTSConfig) -> VoicePeakSynthesizeRequest {
        VoicePeakSynthesizeRequest {
            text: text.to_string(),
            narrator: config.narrator.clone(),
            emotion: config.emotion.as_ref().map(VoicePeakEmotion::from),
            speed: config.speed,
            pitch: config.pitch,
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
            .map_err(|e| AppError::Tts(format!("VoicePeak request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Tts(format!(
                "VoicePeak returned status {}: {}",
                status, error_body
            )));
        }

        let audio_bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::Tts(format!("VoicePeak response read failed: {}", e)))?;

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
            .map_err(|e| AppError::Tts(format!("VoicePeak connection test failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(AppError::Tts(format!(
                "VoicePeak connection test failed (status {}): {}",
                status, error_body
            )));
        }

        Ok(())
    }
}
