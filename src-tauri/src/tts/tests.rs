// TTS Connector tests

#[cfg(test)]
mod emotion_generator_tests {
    use async_trait::async_trait;

    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse};
    use crate::models::tts::{EmotionParams, TTSConfig, TTSProvider};
    use crate::models::ToolDefinition;
    use crate::tts::emotion_generator::EmotionGenerator;

    /// テスト用モックLLMクライアント
    struct MockLLMClient {
        response: Result<LLMResponse, AppError>,
    }

    impl MockLLMClient {
        fn with_text(text: &str) -> Self {
            Self {
                response: Ok(LLMResponse::Text(text.to_string())),
            }
        }

        fn with_error(err: AppError) -> Self {
            Self { response: Err(err) }
        }
    }

    #[async_trait]
    impl LLMClient for MockLLMClient {
        async fn chat(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            match &self.response {
                Ok(resp) => Ok(resp.clone()),
                Err(e) => Err(AppError::LlmApi(e.to_string())),
            }
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
            unimplemented!("not used in emotion generator tests")
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn make_base_config() -> TTSConfig {
        let mut emotion = EmotionParams::new();
        emotion.insert("happy".to_string(), 50);
        emotion.insert("fun".to_string(), 30);
        emotion.insert("angry".to_string(), 0);
        emotion.insert("sad".to_string(), 0);

        TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: None,
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: Some("Japanese Female 1".to_string()),
            emotion: Some(emotion),
            speed: Some(100.0),
            pitch: Some(0.0),
            irodori_mode: None,
        }
    }

    fn make_llm_config() -> LLMClientConfig {
        LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: None,
        }
    }

    // --- parse_and_validate: 正常系テスト ---

    #[test]
    fn test_parse_valid_json_produces_correct_params() {
        let json =
            r#"{"happy": 80, "fun": 60, "angry": 10, "sad": 5, "speed": 120.0, "pitch": 50.0}"#;
        let config = make_base_config();

        let result = EmotionGenerator::parse_and_validate(json, &config).unwrap();

        assert_eq!(result.emotion.get("happy"), Some(&80));
        assert_eq!(result.emotion.get("fun"), Some(&60));
        assert_eq!(result.emotion.get("angry"), Some(&10));
        assert_eq!(result.emotion.get("sad"), Some(&5));
        assert_eq!(result.speed, Some(120.0));
        assert_eq!(result.pitch, Some(50.0));
    }

    #[test]
    fn test_parse_partial_json_missing_fields_are_none() {
        let json = r#"{"happy": 70, "speed": 110.0}"#;
        let config = make_base_config();

        let result = EmotionGenerator::parse_and_validate(json, &config).unwrap();

        assert_eq!(result.emotion.get("happy"), Some(&70));
        assert_eq!(result.emotion.get("fun"), None);
        assert_eq!(result.emotion.get("angry"), None);
        assert_eq!(result.emotion.get("sad"), None);
        assert_eq!(result.speed, Some(110.0));
        assert_eq!(result.pitch, None);
    }

    // --- parse_and_validate: 範囲外値のクランプテスト ---

    #[test]
    fn test_out_of_range_emotion_values_are_clamped() {
        let json = r#"{"happy": 150, "fun": -20, "angry": 200, "sad": -50}"#;
        let config = make_base_config();

        let result = EmotionGenerator::parse_and_validate(json, &config).unwrap();

        // 0-100にクランプ
        assert_eq!(result.emotion.get("happy"), Some(&100));
        assert_eq!(result.emotion.get("fun"), Some(&0));
        assert_eq!(result.emotion.get("angry"), Some(&100));
        assert_eq!(result.emotion.get("sad"), Some(&0));
    }

    #[test]
    fn test_out_of_range_speed_is_clamped() {
        // speed: 50-200
        let json = r#"{"speed": 300.0}"#;
        let config = make_base_config();
        let result = EmotionGenerator::parse_and_validate(json, &config).unwrap();
        assert_eq!(result.speed, Some(200.0));

        let json = r#"{"speed": 10.0}"#;
        let result = EmotionGenerator::parse_and_validate(json, &config).unwrap();
        assert_eq!(result.speed, Some(50.0));
    }

    #[test]
    fn test_out_of_range_pitch_is_clamped() {
        // pitch: -300 to 300
        let json = r#"{"pitch": 500.0}"#;
        let config = make_base_config();
        let result = EmotionGenerator::parse_and_validate(json, &config).unwrap();
        assert_eq!(result.pitch, Some(300.0));

        let json = r#"{"pitch": -500.0}"#;
        let result = EmotionGenerator::parse_and_validate(json, &config).unwrap();
        assert_eq!(result.pitch, Some(-300.0));
    }

    // --- parse_and_validate: 不正JSON ---

    #[test]
    fn test_invalid_json_returns_error() {
        let json = "this is not json at all";
        let config = make_base_config();

        let result = EmotionGenerator::parse_and_validate(json, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_string_returns_error() {
        let json = "";
        let config = make_base_config();

        let result = EmotionGenerator::parse_and_validate(json, &config);
        assert!(result.is_err());
    }

    // --- parse_and_validate: マークダウンコードブロック対応 ---

    #[test]
    fn test_json_wrapped_in_markdown_code_block() {
        let json = r#"```json
{"happy": 90, "fun": 70, "angry": 0, "sad": 0, "speed": 130.0, "pitch": 20.0}
```"#;
        let config = make_base_config();

        let result = EmotionGenerator::parse_and_validate(json, &config).unwrap();

        assert_eq!(result.emotion.get("happy"), Some(&90));
        assert_eq!(result.emotion.get("fun"), Some(&70));
        assert_eq!(result.speed, Some(130.0));
        assert_eq!(result.pitch, Some(20.0));
    }

    #[test]
    fn test_json_wrapped_in_plain_code_block() {
        let json = r#"```
{"happy": 40, "sad": 60, "speed": 80.0}
```"#;
        let config = make_base_config();

        let result = EmotionGenerator::parse_and_validate(json, &config).unwrap();

        assert_eq!(result.emotion.get("happy"), Some(&40));
        assert_eq!(result.emotion.get("sad"), Some(&60));
        assert_eq!(result.speed, Some(80.0));
    }

    // --- generate(): LLMエラー時のテスト ---

    #[tokio::test]
    async fn test_generate_llm_failure_propagates_error() {
        let generator = EmotionGenerator;
        let base_config = make_base_config();
        let llm_config = make_llm_config();
        let mock_client =
            MockLLMClient::with_error(AppError::LlmApi("Connection refused".to_string()));

        let result = generator
            .generate("こんにちは", &base_config, &mock_client, &llm_config)
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("LLM API error"));
    }

    // --- generate(): 正常系テスト ---

    #[tokio::test]
    async fn test_generate_with_valid_llm_response() {
        let generator = EmotionGenerator;
        let base_config = make_base_config();
        let llm_config = make_llm_config();
        let mock_client = MockLLMClient::with_text(
            r#"{"happy": 85, "fun": 70, "angry": 0, "sad": 0, "speed": 115.0, "pitch": 30.0}"#,
        );

        let result = generator
            .generate(
                "今日はとても楽しい一日だった！",
                &base_config,
                &mock_client,
                &llm_config,
            )
            .await
            .unwrap();

        assert_eq!(result.emotion.get("happy"), Some(&85));
        assert_eq!(result.emotion.get("fun"), Some(&70));
        assert_eq!(result.emotion.get("angry"), Some(&0));
        assert_eq!(result.emotion.get("sad"), Some(&0));
        assert_eq!(result.speed, Some(115.0));
        assert_eq!(result.pitch, Some(30.0));
    }

    #[tokio::test]
    async fn test_generate_with_markdown_wrapped_response() {
        let generator = EmotionGenerator;
        let base_config = make_base_config();
        let llm_config = make_llm_config();
        let mock_client = MockLLMClient::with_text(
            "```json\n{\"happy\": 20, \"sad\": 80, \"speed\": 85.0, \"pitch\": -50.0}\n```",
        );

        let result = generator
            .generate(
                "悲しいお知らせがある",
                &base_config,
                &mock_client,
                &llm_config,
            )
            .await
            .unwrap();

        assert_eq!(result.emotion.get("happy"), Some(&20));
        assert_eq!(result.emotion.get("sad"), Some(&80));
        assert_eq!(result.speed, Some(85.0));
        assert_eq!(result.pitch, Some(-50.0));
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::models::tts::{EmotionParams, TTSConfig, TTSProvider};
    use crate::tts::irodori::IrodoriTTSHandler;
    use crate::tts::voicepeak::VoicePeakHandler;

    fn make_irodori_config() -> TTSConfig {
        TTSConfig {
            provider: TTSProvider::IrodoriTts,
            base_url: Some("http://localhost:5000".to_string()),
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: Some("/path/to/reference.wav".to_string()),
            caption: Some("明るい声で話す".to_string()),
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
            irodori_mode: None,
        }
    }

    fn make_voicepeak_config() -> TTSConfig {
        let mut emotion = EmotionParams::new();
        emotion.insert("happy".to_string(), 50);
        emotion.insert("fun".to_string(), 30);

        TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: None,
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: Some("Japanese Female 1".to_string()),
            emotion: Some(emotion),
            speed: Some(120.0),
            pitch: Some(-50.0),
            irodori_mode: None,
        }
    }

    // --- Irodori-TTS リクエストボディ構築テスト ---

    #[test]
    fn test_irodori_build_request_body_full() {
        let config = make_irodori_config();
        let body = IrodoriTTSHandler::build_request_body("こんにちは", &config);

        assert_eq!(body.text, "こんにちは");
        assert_eq!(
            body.reference_audio_path,
            Some("/path/to/reference.wav".to_string())
        );
        assert_eq!(body.caption, Some("明るい声で話す".to_string()));
    }

    #[test]
    fn test_irodori_build_request_body_minimal() {
        let config = TTSConfig {
            provider: TTSProvider::IrodoriTts,
            base_url: Some("http://localhost:5000".to_string()),
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
            irodori_mode: None,
        };
        let body = IrodoriTTSHandler::build_request_body("テスト", &config);

        assert_eq!(body.text, "テスト");
        assert!(body.reference_audio_path.is_none());
        assert!(body.caption.is_none());
    }

    #[test]
    fn test_irodori_request_body_serialization() {
        let config = make_irodori_config();
        let body = IrodoriTTSHandler::build_request_body("音声合成テスト", &config);
        let json = serde_json::to_value(&body).unwrap();

        assert_eq!(json["text"], "音声合成テスト");
        assert_eq!(json["reference_audio_path"], "/path/to/reference.wav");
        assert_eq!(json["caption"], "明るい声で話す");
    }

    #[test]
    fn test_irodori_request_body_skips_none_fields() {
        let config = TTSConfig {
            provider: TTSProvider::IrodoriTts,
            base_url: Some("http://localhost:5000".to_string()),
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
            irodori_mode: None,
        };
        let body = IrodoriTTSHandler::build_request_body("テスト", &config);
        let json = serde_json::to_string(&body).unwrap();

        // None フィールドはシリアライズされない
        assert!(!json.contains("reference_audio_path"));
        assert!(!json.contains("caption"));
    }

    // --- VoicePeak CLI引数構築テスト ---

    #[test]
    fn test_voicepeak_build_cli_args_full() {
        let config = make_voicepeak_config();
        let output_path = Path::new("/tmp/output.wav");
        let args = VoicePeakHandler::build_cli_args("こんにちは", output_path, &config);

        assert!(args.contains(&"--say".to_string()));
        assert!(args.contains(&"こんにちは".to_string()));
        assert!(args.contains(&"--out".to_string()));
        assert!(args.contains(&"/tmp/output.wav".to_string()));
        assert!(args.contains(&"--narrator".to_string()));
        assert!(args.contains(&"Japanese Female 1".to_string()));
        assert!(args.contains(&"--emotion".to_string()));
        // HashMapの順序は不定なので、emotion文字列の内容を検証
        let emotion_idx = args.iter().position(|a| a == "--emotion").unwrap();
        let emotion_str = &args[emotion_idx + 1];
        assert!(emotion_str.contains("happy=50"));
        assert!(emotion_str.contains("fun=30"));
        assert!(args.contains(&"--speed".to_string()));
        assert!(args.contains(&"120".to_string()));
        assert!(args.contains(&"--pitch".to_string()));
        assert!(args.contains(&"-50".to_string()));
    }

    #[test]
    fn test_voicepeak_build_cli_args_minimal() {
        let config = TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: None,
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
            irodori_mode: None,
        };
        let output_path = Path::new("/tmp/output.wav");
        let args = VoicePeakHandler::build_cli_args("テスト", output_path, &config);

        // --say と --out のみ
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "--say");
        assert_eq!(args[1], "テスト");
        assert_eq!(args[2], "--out");
        assert_eq!(args[3], "/tmp/output.wav");
    }

    #[test]
    fn test_voicepeak_format_emotion_full() {
        let mut emotion = EmotionParams::new();
        emotion.insert("happy".to_string(), 50);
        emotion.insert("fun".to_string(), 30);
        emotion.insert("angry".to_string(), 20);
        emotion.insert("sad".to_string(), 10);

        let result = VoicePeakHandler::format_emotion(&emotion);
        // HashMapの順序は不定なので、パースして検証
        let result_str = result.unwrap();
        let pairs: Vec<&str> = result_str.split(',').collect();
        assert_eq!(pairs.len(), 4);
        assert!(result_str.contains("happy=50"));
        assert!(result_str.contains("fun=30"));
        assert!(result_str.contains("angry=20"));
        assert!(result_str.contains("sad=10"));
    }

    #[test]
    fn test_voicepeak_format_emotion_partial() {
        let mut emotion = EmotionParams::new();
        emotion.insert("angry".to_string(), 80);
        emotion.insert("sad".to_string(), 20);

        let result = VoicePeakHandler::format_emotion(&emotion);
        let result_str = result.unwrap();
        let pairs: Vec<&str> = result_str.split(',').collect();
        assert_eq!(pairs.len(), 2);
        assert!(result_str.contains("angry=80"));
        assert!(result_str.contains("sad=20"));
    }

    #[test]
    fn test_voicepeak_format_emotion_all_none() {
        let emotion = EmotionParams::new();
        let result = VoicePeakHandler::format_emotion(&emotion);
        assert_eq!(result, None);
    }

    #[test]
    fn test_voicepeak_cli_args_no_emotion_when_all_none() {
        let config = TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: None,
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: Some(EmotionParams::new()),
            speed: None,
            pitch: None,
            irodori_mode: None,
        };
        let output_path = Path::new("/tmp/output.wav");
        let args = VoicePeakHandler::build_cli_args("テスト", output_path, &config);

        // --emotion フラグが含まれないこと
        assert!(!args.contains(&"--emotion".to_string()));
    }

    // --- DefaultTTSConnector ディスパッチテスト ---

    #[test]
    fn test_default_tts_connector_creation() {
        use crate::tts::connector::DefaultTTSConnector;
        let _connector = DefaultTTSConnector::new();
    }

    #[test]
    fn test_default_tts_connector_default_trait() {
        use crate::tts::connector::DefaultTTSConnector;
        let _connector = DefaultTTSConnector::default();
    }

    // --- voicepeak_path デフォルト値テスト ---

    #[test]
    fn test_voicepeak_path_defaults_to_voicepeak_when_not_specified() {
        // voicepeak_path 未指定時に "voicepeak" が使用されること
        let path: Option<&str> = None;
        let default_path = path.unwrap_or("voicepeak");
        assert_eq!(default_path, "voicepeak");
    }

    #[test]
    fn test_voicepeak_path_uses_custom_value_when_specified() {
        let path: Option<&str> = Some("C:\\Program Files\\VoicePeak\\voicepeak.exe");
        let resolved = path.unwrap_or("voicepeak");
        assert_eq!(resolved, "C:\\Program Files\\VoicePeak\\voicepeak.exe");
    }

    // --- オプショナルパラメータ省略テスト ---

    #[test]
    fn test_optional_params_omitted_narrator_only() {
        // Requirements 2.7: 未指定パラメータのフラグが含まれないこと
        let config = TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: None,
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: Some("Japanese Male 1".to_string()),
            emotion: None,
            speed: None,
            pitch: None,
            irodori_mode: None,
        };
        let output_path = Path::new("/tmp/output.wav");
        let args = VoicePeakHandler::build_cli_args("テスト", output_path, &config);

        // --narrator は含まれる
        assert!(args.contains(&"--narrator".to_string()));
        assert!(args.contains(&"Japanese Male 1".to_string()));
        // --emotion, --speed, --pitch は含まれない
        assert!(!args.contains(&"--emotion".to_string()));
        assert!(!args.contains(&"--speed".to_string()));
        assert!(!args.contains(&"--pitch".to_string()));
    }

    #[test]
    fn test_optional_params_omitted_speed_only() {
        let config = TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: None,
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: None,
            speed: Some(150.0),
            pitch: None,
            irodori_mode: None,
        };
        let output_path = Path::new("/tmp/output.wav");
        let args = VoicePeakHandler::build_cli_args("テスト", output_path, &config);

        // --speed は含まれる
        assert!(args.contains(&"--speed".to_string()));
        assert!(args.contains(&"150".to_string()));
        // --narrator, --emotion, --pitch は含まれない
        assert!(!args.contains(&"--narrator".to_string()));
        assert!(!args.contains(&"--emotion".to_string()));
        assert!(!args.contains(&"--pitch".to_string()));
    }

    #[test]
    fn test_optional_params_omitted_pitch_only() {
        let config = TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: None,
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: None,
            speed: None,
            pitch: Some(-30.0),
            irodori_mode: None,
        };
        let output_path = Path::new("/tmp/output.wav");
        let args = VoicePeakHandler::build_cli_args("テスト", output_path, &config);

        // --pitch は含まれる
        assert!(args.contains(&"--pitch".to_string()));
        assert!(args.contains(&"-30".to_string()));
        // --narrator, --emotion, --speed は含まれない
        assert!(!args.contains(&"--narrator".to_string()));
        assert!(!args.contains(&"--emotion".to_string()));
        assert!(!args.contains(&"--speed".to_string()));
    }

    // --- TTSConfig シリアライズ/デシリアライズ互換性テスト ---

    #[test]
    fn test_tts_config_serialize_deserialize_roundtrip_voicepeak() {
        let config = make_voicepeak_config();
        let json_str = serde_json::to_string(&config).unwrap();
        let deserialized: TTSConfig = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.provider, config.provider);
        assert_eq!(deserialized.base_url, config.base_url);
        assert_eq!(deserialized.narrator, config.narrator);
        assert_eq!(deserialized.speed, config.speed);
        assert_eq!(deserialized.pitch, config.pitch);
        // EmotionParams比較
        let orig_emotion = config.emotion.unwrap();
        let deser_emotion = deserialized.emotion.unwrap();
        assert_eq!(deser_emotion, orig_emotion);
    }

    #[test]
    fn test_tts_config_serialize_deserialize_roundtrip_irodori() {
        let config = make_irodori_config();
        let json_str = serde_json::to_string(&config).unwrap();
        let deserialized: TTSConfig = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.provider, config.provider);
        assert_eq!(deserialized.base_url, config.base_url);
        assert_eq!(
            deserialized.reference_audio_path,
            config.reference_audio_path
        );
        assert_eq!(deserialized.caption, config.caption);
        assert_eq!(deserialized.narrator, config.narrator);
        assert_eq!(deserialized.speed, config.speed);
        assert_eq!(deserialized.pitch, config.pitch);
    }

    #[test]
    fn test_tts_config_deserialize_with_missing_optional_fields() {
        // base_url が省略されたJSONからデシリアライズ可能なこと
        let json = serde_json::json!({
            "provider": "voicepeak",
            "narrator": "Japanese Female 1"
        });

        let config: TTSConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.provider, TTSProvider::Voicepeak);
        assert_eq!(config.base_url, None);
        assert_eq!(config.narrator, Some("Japanese Female 1".to_string()));
        assert!(config.emotion.is_none());
        assert!(config.speed.is_none());
        assert!(config.pitch.is_none());
    }

    // --- TTSProvider判定テスト ---

    #[test]
    fn test_tts_provider_irodori_dispatch() {
        let config = make_irodori_config();
        assert_eq!(config.provider, TTSProvider::IrodoriTts);
    }

    #[test]
    fn test_tts_provider_voicepeak_dispatch() {
        let config = make_voicepeak_config();
        assert_eq!(config.provider, TTSProvider::Voicepeak);
    }

    // --- TTSConfig シリアライズテスト ---

    #[test]
    fn test_tts_config_serialization_irodori() {
        let config = make_irodori_config();
        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["provider"], "irodori-tts");
        assert_eq!(json["base_url"], "http://localhost:5000");
        assert_eq!(json["reference_audio_path"], "/path/to/reference.wav");
        assert_eq!(json["caption"], "明るい声で話す");
    }

    #[test]
    fn test_tts_config_serialization_voicepeak() {
        let config = make_voicepeak_config();
        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["provider"], "voicepeak");
        assert_eq!(json["narrator"], "Japanese Female 1");
        assert_eq!(json["emotion"]["happy"], 50);
    }

    #[test]
    fn test_tts_config_deserialization() {
        let json = serde_json::json!({
            "provider": "irodori-tts",
            "base_url": "http://localhost:5000",
            "reference_audio_path": "/audio/ref.wav",
            "caption": "テストキャプション"
        });

        let config: TTSConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.provider, TTSProvider::IrodoriTts);
        assert_eq!(config.base_url, Some("http://localhost:5000".to_string()));
        assert_eq!(
            config.reference_audio_path,
            Some("/audio/ref.wav".to_string())
        );
        assert_eq!(config.caption, Some("テストキャプション".to_string()));
        assert!(config.narrator.is_none());
        assert!(config.emotion.is_none());
    }
}

#[cfg(test)]
mod caption_generator_tests {
    use async_trait::async_trait;

    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse};
    use crate::models::{ToolCall, ToolDefinition};
    use crate::tts::caption_generator::CaptionGenerator;

    /// テスト用モックLLMクライアント
    struct MockLLMClient {
        response: Result<LLMResponse, AppError>,
    }

    impl MockLLMClient {
        fn with_text(text: &str) -> Self {
            Self {
                response: Ok(LLMResponse::Text(text.to_string())),
            }
        }

        fn with_error(err: AppError) -> Self {
            Self { response: Err(err) }
        }

        fn with_tool_calls() -> Self {
            Self {
                response: Ok(LLMResponse::ToolCalls(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "some_tool".to_string(),
                    arguments: serde_json::Value::Object(serde_json::Map::new()),
                }])),
            }
        }
    }

    #[async_trait]
    impl LLMClient for MockLLMClient {
        async fn chat(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            match &self.response {
                Ok(resp) => Ok(resp.clone()),
                Err(e) => Err(AppError::LlmApi(e.to_string())),
            }
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
            unimplemented!("not used in caption generator tests")
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn make_llm_config() -> LLMClientConfig {
        LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: None,
        }
    }

    // --- combine_captions テスト ---

    #[test]
    fn test_combine_captions_normal_inputs() {
        let result =
            CaptionGenerator::combine_captions("落ち着いた女性の声", "優しく語りかけるように");
        assert!(result.contains("落ち着いた女性の声"));
        assert!(result.contains("優しく語りかけるように"));
    }

    #[test]
    fn test_combine_captions_with_empty_dynamic() {
        let result = CaptionGenerator::combine_captions("明るい声で話す", "");
        assert!(result.contains("明るい声で話す"));
    }

    #[test]
    fn test_combine_captions_with_empty_base() {
        let result = CaptionGenerator::combine_captions("", "元気よく明るく");
        assert!(result.contains("元気よく明るく"));
    }

    // --- generate(): 正常系テスト ---

    #[tokio::test]
    async fn test_generate_with_valid_response_returns_trimmed_caption() {
        let generator = CaptionGenerator;
        let llm_config = make_llm_config();
        let mock_client = MockLLMClient::with_text("  優しく語りかけるように  ");

        let result = generator
            .generate(
                "こんにちは、今日はいい天気ですね。",
                "落ち着いた女性の声",
                &mock_client,
                &llm_config,
            )
            .await
            .unwrap();

        assert_eq!(result, "優しく語りかけるように");
    }

    // --- generate(): LLMエラー時のテスト ---

    #[tokio::test]
    async fn test_generate_llm_failure_propagates_error() {
        let generator = CaptionGenerator;
        let llm_config = make_llm_config();
        let mock_client =
            MockLLMClient::with_error(AppError::LlmApi("Connection refused".to_string()));

        let result = generator
            .generate("テスト文", "ベースキャプション", &mock_client, &llm_config)
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("LLM API error"));
    }

    // --- generate(): ToolCalls返却時のテスト ---

    #[tokio::test]
    async fn test_generate_with_tool_calls_returns_error() {
        let generator = CaptionGenerator;
        let llm_config = make_llm_config();
        let mock_client = MockLLMClient::with_tool_calls();

        let result = generator
            .generate("テスト文", "ベースキャプション", &mock_client, &llm_config)
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unexpected tool call"));
    }
}

#[cfg(test)]
mod flow_controller_tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use tokio::time::sleep;

    use crate::config::model_config::ModelConfigManager;
    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse};
    use crate::models::config::{
        AppConfig, AttachmentConfig, MemoryConfig, ModelPurpose, ModelSettings, PluginsConfig,
        SendKey, SpontaneousConfig, TTSGlobalConfig, Theme, ThoughtConfig, UIConfig,
    };
    use crate::models::tts::{EmotionParams, IrodoriMode, TTSConfig, TTSProvider};
    use crate::models::ToolDefinition;
    use crate::tts::connector::TTSConnector;
    use crate::tts::flow_controller::TTSFlowController;

    // --- ヘルパー ---

    /// テスト用の有効なWAVデータを生成
    fn make_wav(channels: u16, sample_rate: u32, bits_per_sample: u16, pcm_data: &[u8]) -> Vec<u8> {
        let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_size = pcm_data.len() as u32;
        let file_size = 36 + data_size;

        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        wav.extend_from_slice(pcm_data);
        wav
    }

    /// モックTTSコネクタ — 固定WAVデータを返す
    struct MockTTSConnector {
        wav_data: Vec<u8>,
        delay: Option<Duration>,
    }

    impl MockTTSConnector {
        fn new(wav_data: Vec<u8>) -> Self {
            Self {
                wav_data,
                delay: None,
            }
        }

        fn with_delay(wav_data: Vec<u8>, delay: Duration) -> Self {
            Self {
                wav_data,
                delay: Some(delay),
            }
        }
    }

    #[async_trait]
    impl TTSConnector for MockTTSConnector {
        async fn synthesize(
            &self,
            _text: &str,
            _config: &TTSConfig,
            _voicepeak_path: Option<&str>,
        ) -> Result<Vec<u8>, AppError> {
            if let Some(delay) = self.delay {
                sleep(delay).await;
            }
            Ok(self.wav_data.clone())
        }

        async fn test_connection(
            &self,
            _config: &TTSConfig,
            _voicepeak_path: Option<&str>,
        ) -> Result<(), AppError> {
            Ok(())
        }
    }

    /// モックLLMクライアント
    struct MockLLMClient {
        response: Result<LLMResponse, AppError>,
    }

    impl MockLLMClient {
        fn with_text(text: &str) -> Self {
            Self {
                response: Ok(LLMResponse::Text(text.to_string())),
            }
        }

        fn with_error() -> Self {
            Self {
                response: Err(AppError::LlmApi("Connection refused".to_string())),
            }
        }
    }

    #[async_trait]
    impl LLMClient for MockLLMClient {
        async fn chat(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            match &self.response {
                Ok(resp) => Ok(resp.clone()),
                Err(e) => Err(AppError::LlmApi(e.to_string())),
            }
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
            unimplemented!("not used in flow controller tests")
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    /// テスト用AppConfig生成
    fn make_test_app_config() -> AppConfig {
        let mut models = HashMap::new();
        models.insert(
            ModelPurpose::Chat,
            ModelSettings {
                base_url: "http://localhost:8080/v1".to_string(),
                model: "test-model".to_string(),
                api_key: None,
                temperature: 0.7,
                provider: None,
            },
        );

        AppConfig {
            models,
            spontaneous: SpontaneousConfig {
                enabled: false,
                min_interval_seconds: 60,
                probability: 0.3,
            },
            thought: ThoughtConfig {
                enabled: false,
                interval_minutes: 5,
                auto_delete_threshold_minutes: 1440,
            },
            memory: MemoryConfig {
                compression_threshold: 50,
            },
            tts: TTSGlobalConfig {
                enabled: true,
                voicepeak_path: None,
                irodori_base_url: None,
                irodori_caption_base_url: None,
                irodori_reference_audio_base_url: None,
                timeout_seconds: 60,
                max_chunk_size: 140,
            },
            ui: UIConfig {
                theme: Theme::Dark,
                language: "ja".to_string(),
                send_key: SendKey::default(),
            },
            plugins: PluginsConfig {
                enabled_plugins: vec![],
                plugin_settings: HashMap::new(),
            },
            attachment: AttachmentConfig {
                max_file_size_bytes: 10 * 1024 * 1024,
                allowed_extensions: vec!["txt".to_string()],
            },
        }
    }

    fn make_voicepeak_tts_config() -> TTSConfig {
        let mut emotion = EmotionParams::new();
        emotion.insert("happy".to_string(), 50);
        emotion.insert("fun".to_string(), 30);
        emotion.insert("angry".to_string(), 0);
        emotion.insert("sad".to_string(), 0);

        TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: None,
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: None,
            caption: None,
            narrator: Some("Japanese Female 1".to_string()),
            emotion: Some(emotion),
            speed: Some(100.0),
            pitch: Some(0.0),
            irodori_mode: None,
        }
    }

    fn make_irodori_caption_config() -> TTSConfig {
        TTSConfig {
            provider: TTSProvider::IrodoriTts,
            base_url: Some("http://localhost:5000".to_string()),
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: Some("/path/to/ref.wav".to_string()),
            caption: Some("落ち着いた女性の声".to_string()),
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
            irodori_mode: Some(IrodoriMode::Caption),
        }
    }

    fn make_irodori_reference_audio_config() -> TTSConfig {
        TTSConfig {
            provider: TTSProvider::IrodoriTts,
            base_url: Some("http://localhost:5000".to_string()),
            caption_base_url: None,
            reference_audio_base_url: None,
            reference_audio_path: Some("/path/to/ref.wav".to_string()),
            caption: None,
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
            irodori_mode: Some(IrodoriMode::ReferenceAudio),
        }
    }

    // --- テスト: フルフロー（VoicePeak） ---

    #[tokio::test]
    async fn test_full_flow_voicepeak_with_emotion_generation() {
        let pcm_data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let wav_data = make_wav(1, 44100, 16, &pcm_data);

        let tts_connector = Arc::new(MockTTSConnector::new(wav_data));
        let llm_client = Arc::new(MockLLMClient::with_text(
            r#"{"happy": 80, "fun": 60, "angry": 0, "sad": 0, "speed": 120.0, "pitch": 30.0}"#,
        ));
        let config_manager = Arc::new(ModelConfigManager::new_with_config(make_test_app_config()));

        let controller = TTSFlowController::new(tts_connector, llm_client, config_manager);

        let tts_config = make_voicepeak_tts_config();
        let result = controller
            .process("こんにちは", &tts_config, None, 60)
            .await
            .unwrap();

        // 結果にオーディオデータとテキストが含まれる
        assert!(!result.audio_data.is_empty());
        assert_eq!(result.text, "こんにちは");
        // WAVヘッダーが存在する（RIFFマジック）
        assert_eq!(&result.audio_data[0..4], b"RIFF");
    }

    // --- テスト: タイムアウト ---

    #[tokio::test]
    async fn test_timeout_triggers_error() {
        let pcm_data = vec![0u8; 16];
        let wav_data = make_wav(1, 44100, 16, &pcm_data);

        // 2秒遅延するモックTTSコネクタ
        let tts_connector = Arc::new(MockTTSConnector::with_delay(
            wav_data,
            Duration::from_secs(2),
        ));
        let llm_client = Arc::new(MockLLMClient::with_text(r#"{"happy": 50, "speed": 100.0}"#));
        let config_manager = Arc::new(ModelConfigManager::new_with_config(make_test_app_config()));

        let controller = TTSFlowController::new(tts_connector, llm_client, config_manager);

        let tts_config = make_voicepeak_tts_config();
        // タイムアウト0秒 → 即座にタイムアウト
        let result = controller.process("テスト", &tts_config, None, 0).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("timed out"));
    }

    // --- テスト: LLM失敗時のフォールバック ---

    #[tokio::test]
    async fn test_llm_failure_fallback_uses_default_config() {
        let pcm_data = vec![10u8, 20, 30, 40];
        let wav_data = make_wav(1, 44100, 16, &pcm_data);

        let tts_connector = Arc::new(MockTTSConnector::new(wav_data));
        // LLMがエラーを返す
        let llm_client = Arc::new(MockLLMClient::with_error());
        let config_manager = Arc::new(ModelConfigManager::new_with_config(make_test_app_config()));

        let controller = TTSFlowController::new(tts_connector, llm_client, config_manager);

        let tts_config = make_voicepeak_tts_config();
        // LLM失敗してもフォールバックで成功する
        let result = controller
            .process("テスト文章", &tts_config, None, 60)
            .await;

        assert!(result.is_ok());
        let tts_result = result.unwrap();
        assert_eq!(tts_result.text, "テスト文章");
        assert!(!tts_result.audio_data.is_empty());
    }

    // --- テスト: Irodori reference_audioモードではキャプション生成をスキップ ---

    /// LLM呼び出しを追跡するモック
    struct TrackingLLMClient {
        call_count: std::sync::atomic::AtomicU32,
    }

    impl TrackingLLMClient {
        fn new() -> Self {
            Self {
                call_count: std::sync::atomic::AtomicU32::new(0),
            }
        }

        fn get_call_count(&self) -> u32 {
            self.call_count.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LLMClient for TrackingLLMClient {
        async fn chat(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            self.call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(LLMResponse::Text("動的キャプション".to_string()))
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
            unimplemented!("not used in flow controller tests")
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_irodori_reference_audio_mode_skips_caption_generation() {
        let pcm_data = vec![5u8, 6, 7, 8];
        let wav_data = make_wav(1, 44100, 16, &pcm_data);

        let tts_connector = Arc::new(MockTTSConnector::new(wav_data));
        let llm_client = Arc::new(TrackingLLMClient::new());
        let config_manager = Arc::new(ModelConfigManager::new_with_config(make_test_app_config()));

        let controller = TTSFlowController::new(tts_connector, llm_client.clone(), config_manager);

        let tts_config = make_irodori_reference_audio_config();
        let result = controller
            .process("テスト音声", &tts_config, None, 60)
            .await;

        assert!(result.is_ok());
        // reference_audioモードではLLMが呼ばれない
        assert_eq!(llm_client.get_call_count(), 0);
    }

    // --- テスト: Irodori captionモードではLLMが呼ばれる ---

    #[tokio::test]
    async fn test_irodori_caption_mode_calls_llm() {
        let pcm_data = vec![5u8, 6, 7, 8];
        let wav_data = make_wav(1, 44100, 16, &pcm_data);

        let tts_connector = Arc::new(MockTTSConnector::new(wav_data));
        let llm_client = Arc::new(TrackingLLMClient::new());
        let config_manager = Arc::new(ModelConfigManager::new_with_config(make_test_app_config()));

        let controller = TTSFlowController::new(tts_connector, llm_client.clone(), config_manager);

        let tts_config = make_irodori_caption_config();
        let result = controller
            .process("テスト音声", &tts_config, None, 60)
            .await;

        assert!(result.is_ok());
        // captionモードではLLMが呼ばれる
        assert!(llm_client.get_call_count() > 0);
    }
}
