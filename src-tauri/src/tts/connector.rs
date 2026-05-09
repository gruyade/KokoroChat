// TTS Connector trait定義とDefaultTTSConnector実装

use async_trait::async_trait;
use reqwest::Client;

use crate::error::AppError;
use crate::models::tts::{TTSConfig, TTSProvider};

use super::irodori::IrodoriTTSHandler;
#[allow(unused_imports)]
use super::voicepeak::VoicePeakHandler;

/// TTS音声合成コネクタtrait
#[async_trait]
pub trait TTSConnector: Send + Sync {
    /// テキストを音声合成し、音声バイトデータを返す
    /// `voicepeak_path`: 全体設定のVoicePeak CLIパス
    async fn synthesize(
        &self,
        text: &str,
        config: &TTSConfig,
        voicepeak_path: Option<&str>,
    ) -> Result<Vec<u8>, AppError>;

    /// TTS接続テスト
    /// `voicepeak_path`: 全体設定のVoicePeak CLIパス
    async fn test_connection(
        &self,
        config: &TTSConfig,
        voicepeak_path: Option<&str>,
    ) -> Result<(), AppError>;
}

/// デフォルトTTSコネクタ — プロバイダーに応じてディスパッチ
pub struct DefaultTTSConnector {
    http_client: Client,
}

impl DefaultTTSConnector {
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
        }
    }

    /// Irodori-TTS APIで音声合成
    async fn synthesize_irodori(
        &self,
        text: &str,
        config: &TTSConfig,
    ) -> Result<Vec<u8>, AppError> {
        let handler = IrodoriTTSHandler::new(&self.http_client);
        handler.synthesize(text, config).await
    }

    /// VoicePeak CLIで音声合成
    async fn synthesize_voicepeak(
        &self,
        text: &str,
        config: &TTSConfig,
        voicepeak_path: Option<&str>,
    ) -> Result<Vec<u8>, AppError> {
        let handler = VoicePeakHandler::new();
        handler.synthesize(text, config, voicepeak_path).await
    }

    /// Irodori-TTS接続テスト
    async fn test_irodori(&self, config: &TTSConfig) -> Result<(), AppError> {
        let handler = IrodoriTTSHandler::new(&self.http_client);
        handler.test_connection(config).await
    }

    /// VoicePeak接続テスト
    async fn test_voicepeak(
        &self,
        config: &TTSConfig,
        voicepeak_path: Option<&str>,
    ) -> Result<(), AppError> {
        let handler = VoicePeakHandler::new();
        handler.test_connection(config, voicepeak_path).await
    }
}

impl Default for DefaultTTSConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TTSConnector for DefaultTTSConnector {
    async fn synthesize(
        &self,
        text: &str,
        config: &TTSConfig,
        voicepeak_path: Option<&str>,
    ) -> Result<Vec<u8>, AppError> {
        match config.provider {
            TTSProvider::IrodoriTts => self.synthesize_irodori(text, config).await,
            TTSProvider::Voicepeak => {
                self.synthesize_voicepeak(text, config, voicepeak_path)
                    .await
            }
        }
    }

    async fn test_connection(
        &self,
        config: &TTSConfig,
        voicepeak_path: Option<&str>,
    ) -> Result<(), AppError> {
        match config.provider {
            TTSProvider::IrodoriTts => self.test_irodori(config).await,
            TTSProvider::Voicepeak => self.test_voicepeak(config, voicepeak_path).await,
        }
    }
}
