// TTS Flow Controller - TTS有効時の音声生成フロー制御

use std::sync::Arc;
use std::time::Duration;

use crate::config::model_config::ModelConfigManager;
use crate::error::AppError;
use crate::llm::client::{LLMClient, LLMClientConfig};
use crate::models::config::ModelPurpose;
use crate::models::tts::{IrodoriMode, TTSConfig, TTSProvider};
use crate::tts::caption_generator::CaptionGenerator;
use crate::tts::connector::TTSConnector;
use crate::tts::emotion_generator::EmotionGenerator;
use crate::tts::text_splitter::{split_text, SplitConfig};
use crate::tts::wav_concat::concatenate_wav;

/// TTS Flow Controller — TTS有効時の音声生成フロー制御
pub struct TTSFlowController {
    tts_connector: Arc<dyn TTSConnector>,
    llm_client: Arc<dyn LLMClient>,
    config_manager: Arc<ModelConfigManager>,
}

/// TTS処理結果
#[derive(Debug)]
pub struct TTSResult {
    /// 結合済みWAVデータ
    pub audio_data: Vec<u8>,
    /// 元テキスト
    pub text: String,
}

impl TTSFlowController {
    /// 新規作成
    pub fn new(
        tts_connector: Arc<dyn TTSConnector>,
        llm_client: Arc<dyn LLMClient>,
        config_manager: Arc<ModelConfigManager>,
    ) -> Self {
        Self {
            tts_connector,
            llm_client,
            config_manager,
        }
    }

    /// TTS音声生成フロー全体を実行（タイムアウト付き）
    ///
    /// 1. 感情/キャプション生成（LLM呼び出し）
    /// 2. テキスト分割
    /// 3. 各チャンクの音声合成
    /// 4. WAV結合
    pub async fn process(
        &self,
        text: &str,
        tts_config: &TTSConfig,
        voicepeak_path: Option<&str>,
        timeout_seconds: u64,
    ) -> Result<TTSResult, AppError> {
        let result = tokio::time::timeout(
            Duration::from_secs(timeout_seconds),
            self.process_internal(text, tts_config, voicepeak_path),
        )
        .await;

        match result {
            Ok(Ok(tts_result)) => Ok(tts_result),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(AppError::Tts("TTS generation timed out".to_string())),
        }
    }

    /// 内部処理（タイムアウトなし）
    async fn process_internal(
        &self,
        text: &str,
        tts_config: &TTSConfig,
        voicepeak_path: Option<&str>,
    ) -> Result<TTSResult, AppError> {
        // Step 1: プロバイダーに応じた前処理（感情/キャプション生成）
        let effective_config = self.prepare_config(text, tts_config).await;

        // Step 2: テキスト分割
        let app_config = self.config_manager.get_config();
        let split_config = SplitConfig {
            max_chunk_size: app_config.tts.max_chunk_size,
        };
        let chunks = split_text(text, &split_config);

        // 空テキストの場合
        if chunks.is_empty() {
            return Err(AppError::Tts("No text to synthesize".to_string()));
        }

        // Step 3: 各チャンクの音声合成（逐次実行 — VoicePeakは同時1インスタンス制限）
        let mut audio_chunks: Vec<Vec<u8>> = Vec::new();
        for chunk in &chunks {
            let audio = self
                .tts_connector
                .synthesize(chunk, &effective_config, voicepeak_path)
                .await?;
            audio_chunks.push(audio);
        }

        // Step 4: WAV結合
        let audio_data = concatenate_wav(&audio_chunks)?;

        Ok(TTSResult {
            audio_data,
            text: text.to_string(),
        })
    }

    /// プロバイダーに応じたTTS設定の前処理
    ///
    /// - VoicePeak: EmotionGeneratorで感情パラメータ生成、失敗時はデフォルト設定使用
    /// - Irodori-TTS (Caption mode): CaptionGeneratorでキャプション生成、失敗時はベースキャプションのみ
    /// - Irodori-TTS (ReferenceAudio mode): 設定そのまま使用
    async fn prepare_config(&self, text: &str, tts_config: &TTSConfig) -> TTSConfig {
        match tts_config.provider {
            TTSProvider::Voicepeak => self.prepare_voicepeak_config(text, tts_config).await,
            TTSProvider::IrodoriTts => self.prepare_irodori_config(text, tts_config).await,
        }
    }

    /// VoicePeak用: 感情パラメータをLLMで生成し、設定に反映
    async fn prepare_voicepeak_config(&self, text: &str, tts_config: &TTSConfig) -> TTSConfig {
        let llm_config = self.get_llm_config();

        let generator = EmotionGenerator;
        let result = generator
            .generate(text, tts_config, self.llm_client.as_ref(), &llm_config)
            .await;

        match result {
            Ok(params) => {
                // 生成されたパラメータで設定を上書き
                let mut config = tts_config.clone();
                config.emotion = Some(params.emotion);
                if let Some(speed) = params.speed {
                    config.speed = Some(speed);
                }
                if let Some(pitch) = params.pitch {
                    config.pitch = Some(pitch);
                }
                config
            }
            Err(_) => {
                // LLM失敗時: デフォルト設定をそのまま使用
                tts_config.clone()
            }
        }
    }

    /// Irodori-TTS用: モードに応じてキャプション生成
    async fn prepare_irodori_config(&self, text: &str, tts_config: &TTSConfig) -> TTSConfig {
        let mode = tts_config
            .irodori_mode
            .as_ref()
            .unwrap_or(&IrodoriMode::Caption);

        match mode {
            IrodoriMode::Caption => self.prepare_irodori_caption_config(text, tts_config).await,
            IrodoriMode::ReferenceAudio => {
                // reference_audioモード: キャプション生成なし、設定そのまま
                tts_config.clone()
            }
        }
    }

    /// Irodori-TTS Caption mode: LLMでキャプション生成し、ベースキャプションと結合
    async fn prepare_irodori_caption_config(
        &self,
        text: &str,
        tts_config: &TTSConfig,
    ) -> TTSConfig {
        let base_caption = tts_config.caption.as_deref().unwrap_or("");
        let llm_config = self.get_llm_config();

        let generator = CaptionGenerator;
        let result = generator
            .generate(text, base_caption, self.llm_client.as_ref(), &llm_config)
            .await;

        match result {
            Ok(dynamic_caption) => {
                let combined = CaptionGenerator::combine_captions(base_caption, &dynamic_caption);
                let mut config = tts_config.clone();
                config.caption = Some(combined);
                config
            }
            Err(_) => {
                // LLM失敗時: ベースキャプションのみ使用
                tts_config.clone()
            }
        }
    }

    /// Chat用LLM設定を取得
    fn get_llm_config(&self) -> LLMClientConfig {
        self.config_manager
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
            })
    }
}
