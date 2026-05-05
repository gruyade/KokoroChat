//! TTS Connectorのプロパティテスト
//! proptest を使用してTTSリクエストフォーマットの不変条件を検証する。

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::models::tts::{EmotionParams, TTSConfig, TTSProvider};
    use crate::tts::irodori::IrodoriTTSHandler;
    use crate::tts::voicepeak::VoicePeakHandler;

    // ========================================
    // Arbitrary Strategies
    // ========================================

    /// EmotionParams のストラテジー
    fn arb_emotion_params() -> impl Strategy<Value = EmotionParams> {
        (
            proptest::option::of(0i32..100),
            proptest::option::of(0i32..100),
            proptest::option::of(0i32..100),
            proptest::option::of(0i32..100),
        )
            .prop_map(|(happy, fun, angry, sad)| EmotionParams {
                happy,
                fun,
                angry,
                sad,
            })
    }

    /// Irodori-TTS用 TTSConfig のストラテジー
    fn arb_irodori_config() -> impl Strategy<Value = TTSConfig> {
        (
            "http://[a-z]{3,10}:[0-9]{4}",
            proptest::option::of("/path/to/[a-z]{3,10}\\.wav"),
            proptest::option::of("[ぁ-んァ-ヶa-zA-Z ]{1,30}"),
        )
            .prop_map(|(base_url, reference_audio_path, caption)| TTSConfig {
                provider: TTSProvider::IrodoriTts,
                base_url,
                reference_audio_path,
                caption,
                narrator: None,
                emotion: None,
                speed: None,
                pitch: None,
            })
    }

    /// VoicePeak用 TTSConfig のストラテジー
    fn arb_voicepeak_config() -> impl Strategy<Value = TTSConfig> {
        (
            "http://[a-z]{3,10}:[0-9]{4}",
            proptest::option::of("[A-Za-z ]{3,20}"),
            proptest::option::of(arb_emotion_params()),
            proptest::option::of(0.5f32..2.0),
            proptest::option::of(-1.0f32..1.0),
        )
            .prop_map(
                |(base_url, narrator, emotion, speed, pitch)| TTSConfig {
                    provider: TTSProvider::Voicepeak,
                    base_url,
                    reference_audio_path: None,
                    caption: None,
                    narrator,
                    emotion,
                    speed,
                    pitch,
                },
            )
    }

    /// テキスト入力のストラテジー
    fn arb_text() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9ぁ-んァ-ヶ ]{1,100}"
    }

    // ========================================
    // Property 14: TTS request format correctness per provider
    // ========================================
    // **Validates: Requirements 6.2, 6.4**
    //
    // For any valid TTSConfig, the request body built by the handler SHALL:
    // - For IrodoriTts: always contain "text" field, include "reference_audio_path"
    //   and "caption" when present in config
    // - For Voicepeak: always contain "text" field, include "narrator", "emotion",
    //   "speed", "pitch" when present in config
    // - None fields SHALL NOT appear in the serialized JSON

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        // --- Irodori-TTS ---

        /// Irodori: リクエストボディは常に "text" フィールドを含む
        #[test]
        fn prop_irodori_always_contains_text(
            text in arb_text(),
            config in arb_irodori_config(),
        ) {
            let body = IrodoriTTSHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            prop_assert_eq!(json["text"].as_str().unwrap(), text.as_str());
        }

        /// Irodori: reference_audio_path が Some の場合、JSONに含まれる
        #[test]
        fn prop_irodori_includes_reference_audio_when_present(
            text in arb_text(),
            base_url in "http://[a-z]{3,10}:[0-9]{4}",
            ref_path in "/path/to/[a-z]{3,10}\\.wav",
            caption in proptest::option::of("[a-zA-Z]{1,20}"),
        ) {
            let config = TTSConfig {
                provider: TTSProvider::IrodoriTts,
                base_url,
                reference_audio_path: Some(ref_path.clone()),
                caption,
                narrator: None,
                emotion: None,
                speed: None,
                pitch: None,
            };

            let body = IrodoriTTSHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            prop_assert_eq!(
                json["reference_audio_path"].as_str().unwrap(),
                ref_path.as_str(),
                "reference_audio_path must be present when config has it"
            );
        }

        /// Irodori: caption が Some の場合、JSONに含まれる
        #[test]
        fn prop_irodori_includes_caption_when_present(
            text in arb_text(),
            base_url in "http://[a-z]{3,10}:[0-9]{4}",
            ref_path in proptest::option::of("/path/to/[a-z]{3,10}\\.wav"),
            caption in "[a-zA-Z]{1,20}",
        ) {
            let config = TTSConfig {
                provider: TTSProvider::IrodoriTts,
                base_url,
                reference_audio_path: ref_path,
                caption: Some(caption.clone()),
                narrator: None,
                emotion: None,
                speed: None,
                pitch: None,
            };

            let body = IrodoriTTSHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            prop_assert_eq!(
                json["caption"].as_str().unwrap(),
                caption.as_str(),
                "caption must be present when config has it"
            );
        }

        /// Irodori: None フィールドはシリアライズされたJSONに含まれない
        #[test]
        fn prop_irodori_none_fields_absent_in_json(
            text in arb_text(),
            config in arb_irodori_config(),
        ) {
            let body = IrodoriTTSHandler::build_request_body(&text, &config);
            let json_str = serde_json::to_string(&body).unwrap();

            if config.reference_audio_path.is_none() {
                prop_assert!(
                    !json_str.contains("reference_audio_path"),
                    "None reference_audio_path must not appear in JSON"
                );
            }
            if config.caption.is_none() {
                prop_assert!(
                    !json_str.contains("caption"),
                    "None caption must not appear in JSON"
                );
            }
        }

        // --- VoicePeak ---

        /// VoicePeak: リクエストボディは常に "text" フィールドを含む
        #[test]
        fn prop_voicepeak_always_contains_text(
            text in arb_text(),
            config in arb_voicepeak_config(),
        ) {
            let body = VoicePeakHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            prop_assert_eq!(json["text"].as_str().unwrap(), text.as_str());
        }

        /// VoicePeak: narrator が Some の場合、JSONに含まれる
        #[test]
        fn prop_voicepeak_includes_narrator_when_present(
            text in arb_text(),
            base_url in "http://[a-z]{3,10}:[0-9]{4}",
            narrator in "[A-Za-z ]{3,20}",
            emotion in proptest::option::of(arb_emotion_params()),
            speed in proptest::option::of(0.5f32..2.0),
            pitch in proptest::option::of(-1.0f32..1.0),
        ) {
            let config = TTSConfig {
                provider: TTSProvider::Voicepeak,
                base_url,
                reference_audio_path: None,
                caption: None,
                narrator: Some(narrator.clone()),
                emotion,
                speed,
                pitch,
            };

            let body = VoicePeakHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            prop_assert_eq!(
                json["narrator"].as_str().unwrap(),
                narrator.as_str(),
                "narrator must be present when config has it"
            );
        }

        /// VoicePeak: emotion が Some の場合、JSONに含まれる
        #[test]
        fn prop_voicepeak_includes_emotion_when_present(
            text in arb_text(),
            base_url in "http://[a-z]{3,10}:[0-9]{4}",
            narrator in proptest::option::of("[A-Za-z ]{3,20}"),
            emotion in arb_emotion_params(),
            speed in proptest::option::of(0.5f32..2.0),
            pitch in proptest::option::of(-1.0f32..1.0),
        ) {
            let config = TTSConfig {
                provider: TTSProvider::Voicepeak,
                base_url,
                reference_audio_path: None,
                caption: None,
                narrator,
                emotion: Some(emotion.clone()),
                speed,
                pitch,
            };

            let body = VoicePeakHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            prop_assert!(
                json.get("emotion").is_some(),
                "emotion must be present when config has it"
            );

            let emotion_json = &json["emotion"];
            // 各感情パラメータが正しくマッピングされている
            if let Some(happy) = emotion.happy {
                prop_assert_eq!(emotion_json["happy"].as_i64().unwrap(), happy as i64);
            }
            if let Some(fun) = emotion.fun {
                prop_assert_eq!(emotion_json["fun"].as_i64().unwrap(), fun as i64);
            }
            if let Some(angry) = emotion.angry {
                prop_assert_eq!(emotion_json["angry"].as_i64().unwrap(), angry as i64);
            }
            if let Some(sad) = emotion.sad {
                prop_assert_eq!(emotion_json["sad"].as_i64().unwrap(), sad as i64);
            }
        }

        /// VoicePeak: speed が Some の場合、JSONに含まれる
        #[test]
        fn prop_voicepeak_includes_speed_when_present(
            text in arb_text(),
            base_url in "http://[a-z]{3,10}:[0-9]{4}",
            speed in 0.5f32..2.0,
        ) {
            let config = TTSConfig {
                provider: TTSProvider::Voicepeak,
                base_url,
                reference_audio_path: None,
                caption: None,
                narrator: None,
                emotion: None,
                speed: Some(speed),
                pitch: None,
            };

            let body = VoicePeakHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            let speed_val = json["speed"].as_f64().unwrap();
            prop_assert!(
                (speed_val - speed as f64).abs() < 1e-5,
                "speed must match: got {}, expected {}",
                speed_val,
                speed
            );
        }

        /// VoicePeak: pitch が Some の場合、JSONに含まれる
        #[test]
        fn prop_voicepeak_includes_pitch_when_present(
            text in arb_text(),
            base_url in "http://[a-z]{3,10}:[0-9]{4}",
            pitch in -1.0f32..1.0,
        ) {
            let config = TTSConfig {
                provider: TTSProvider::Voicepeak,
                base_url,
                reference_audio_path: None,
                caption: None,
                narrator: None,
                emotion: None,
                speed: None,
                pitch: Some(pitch),
            };

            let body = VoicePeakHandler::build_request_body(&text, &config);
            let json = serde_json::to_value(&body).unwrap();

            let pitch_val = json["pitch"].as_f64().unwrap();
            prop_assert!(
                (pitch_val - pitch as f64).abs() < 1e-5,
                "pitch must match: got {}, expected {}",
                pitch_val,
                pitch
            );
        }

        /// VoicePeak: None フィールドはシリアライズされたJSONに含まれない
        #[test]
        fn prop_voicepeak_none_fields_absent_in_json(
            text in arb_text(),
            config in arb_voicepeak_config(),
        ) {
            let body = VoicePeakHandler::build_request_body(&text, &config);
            let json_str = serde_json::to_string(&body).unwrap();

            if config.narrator.is_none() {
                prop_assert!(
                    !json_str.contains("narrator"),
                    "None narrator must not appear in JSON"
                );
            }
            if config.emotion.is_none() {
                prop_assert!(
                    !json_str.contains("emotion"),
                    "None emotion must not appear in JSON"
                );
            }
            if config.speed.is_none() {
                prop_assert!(
                    !json_str.contains("speed"),
                    "None speed must not appear in JSON"
                );
            }
            if config.pitch.is_none() {
                prop_assert!(
                    !json_str.contains("pitch"),
                    "None pitch must not appear in JSON"
                );
            }
        }
    }
}
