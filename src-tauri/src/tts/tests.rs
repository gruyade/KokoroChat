// TTS Connector tests

#[cfg(test)]
mod tests {
    use crate::models::tts::{EmotionParams, TTSConfig, TTSProvider};
    use crate::tts::irodori::IrodoriTTSHandler;
    use crate::tts::voicepeak::VoicePeakHandler;

    fn make_irodori_config() -> TTSConfig {
        TTSConfig {
            provider: TTSProvider::IrodoriTts,
            base_url: "http://localhost:5000".to_string(),
            reference_audio_path: Some("/path/to/reference.wav".to_string()),
            caption: Some("明るい声で話す".to_string()),
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
        }
    }

    fn make_voicepeak_config() -> TTSConfig {
        TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: "http://localhost:5001".to_string(),
            reference_audio_path: None,
            caption: None,
            narrator: Some("Japanese Female 1".to_string()),
            emotion: Some(EmotionParams {
                happy: Some(50),
                fun: Some(30),
                angry: None,
                sad: None,
            }),
            speed: Some(1.2),
            pitch: Some(-0.5),
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
            base_url: "http://localhost:5000".to_string(),
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
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
            base_url: "http://localhost:5000".to_string(),
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
        };
        let body = IrodoriTTSHandler::build_request_body("テスト", &config);
        let json = serde_json::to_string(&body).unwrap();

        // None フィールドはシリアライズされない
        assert!(!json.contains("reference_audio_path"));
        assert!(!json.contains("caption"));
    }

    // --- VoicePeak リクエストボディ構築テスト ---

    #[test]
    fn test_voicepeak_build_request_body_full() {
        let config = make_voicepeak_config();
        let body = VoicePeakHandler::build_request_body("こんにちは", &config);

        assert_eq!(body.text, "こんにちは");
        assert_eq!(body.narrator, Some("Japanese Female 1".to_string()));
        assert!(body.emotion.is_some());
        let emotion = body.emotion.unwrap();
        assert_eq!(emotion.happy, Some(50));
        assert_eq!(emotion.fun, Some(30));
        assert!(emotion.angry.is_none());
        assert!(emotion.sad.is_none());
        assert_eq!(body.speed, Some(1.2));
        assert_eq!(body.pitch, Some(-0.5));
    }

    #[test]
    fn test_voicepeak_build_request_body_minimal() {
        let config = TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: "http://localhost:5001".to_string(),
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
        };
        let body = VoicePeakHandler::build_request_body("テスト", &config);

        assert_eq!(body.text, "テスト");
        assert!(body.narrator.is_none());
        assert!(body.emotion.is_none());
        assert!(body.speed.is_none());
        assert!(body.pitch.is_none());
    }

    #[test]
    fn test_voicepeak_request_body_serialization() {
        let config = make_voicepeak_config();
        let body = VoicePeakHandler::build_request_body("音声合成テスト", &config);
        let json = serde_json::to_value(&body).unwrap();

        assert_eq!(json["text"], "音声合成テスト");
        assert_eq!(json["narrator"], "Japanese Female 1");
        assert_eq!(json["emotion"]["happy"], 50);
        assert_eq!(json["emotion"]["fun"], 30);
        let speed = json["speed"].as_f64().unwrap();
        assert!((speed - 1.2f64).abs() < 1e-5);
        let pitch = json["pitch"].as_f64().unwrap();
        assert!((pitch - (-0.5f64)).abs() < 1e-5);
    }

    #[test]
    fn test_voicepeak_request_body_skips_none_fields() {
        let config = TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: "http://localhost:5001".to_string(),
            reference_audio_path: None,
            caption: None,
            narrator: None,
            emotion: None,
            speed: None,
            pitch: None,
        };
        let body = VoicePeakHandler::build_request_body("テスト", &config);
        let json = serde_json::to_string(&body).unwrap();

        // None フィールドはシリアライズされない
        assert!(!json.contains("narrator"));
        assert!(!json.contains("emotion"));
        assert!(!json.contains("speed"));
        assert!(!json.contains("pitch"));
    }

    #[test]
    fn test_voicepeak_emotion_partial() {
        let config = TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: "http://localhost:5001".to_string(),
            reference_audio_path: None,
            caption: None,
            narrator: Some("Narrator A".to_string()),
            emotion: Some(EmotionParams {
                happy: None,
                fun: None,
                angry: Some(80),
                sad: Some(20),
            }),
            speed: None,
            pitch: None,
        };
        let body = VoicePeakHandler::build_request_body("怒りテスト", &config);
        let json = serde_json::to_value(&body).unwrap();

        assert_eq!(json["emotion"]["angry"], 80);
        assert_eq!(json["emotion"]["sad"], 20);
        // happy, fun はNoneなのでシリアライズされない
        assert!(json["emotion"].get("happy").is_none());
        assert!(json["emotion"].get("fun").is_none());
    }

    // --- DefaultTTSConnector ディスパッチテスト ---

    #[test]
    fn test_default_tts_connector_creation() {
        use crate::tts::connector::DefaultTTSConnector;
        let _connector = DefaultTTSConnector::new();
        // インスタンス生成が成功すればOK
    }

    #[test]
    fn test_default_tts_connector_default_trait() {
        use crate::tts::connector::DefaultTTSConnector;
        let _connector = DefaultTTSConnector::default();
        // Default trait実装が動作すればOK
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
        assert_eq!(json["base_url"], "http://localhost:5001");
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
        assert_eq!(config.base_url, "http://localhost:5000");
        assert_eq!(
            config.reference_audio_path,
            Some("/audio/ref.wav".to_string())
        );
        assert_eq!(config.caption, Some("テストキャプション".to_string()));
        assert!(config.narrator.is_none());
        assert!(config.emotion.is_none());
    }
}
