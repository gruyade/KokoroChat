//! Model Config のプロパティテスト
//! proptest を使用してモデル設定の不変条件を検証する。

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use proptest::prelude::*;

    use crate::config::model_config::ModelConfigManager;
    use crate::models::config::{
        AppConfig, AttachmentConfig, MemoryConfig, ModelPurpose, ModelSettings, PluginsConfig,
        SpontaneousConfig, TTSGlobalConfig, Theme, ThoughtConfig, UIConfig,
    };

    // ========================================
    // Arbitrary Strategies
    // ========================================

    /// ModelSettings のストラテジー
    fn arb_model_settings() -> impl Strategy<Value = ModelSettings> {
        (
            "http://[a-z]{3,10}:[0-9]{4}/v[0-9]",
            "[a-z]{3,10}-[0-9]{1,3}",
            proptest::option::of("sk-[a-zA-Z0-9]{10,30}"),
            0.0f32..2.0,
        )
            .prop_map(|(base_url, model, api_key, temperature)| ModelSettings {
                base_url,
                model,
                api_key,
                temperature,
            })
    }

    /// ModelPurpose のストラテジー
    fn arb_model_purpose() -> impl Strategy<Value = ModelPurpose> {
        prop_oneof![
            Just(ModelPurpose::Chat),
            Just(ModelPurpose::Memory),
            Just(ModelPurpose::Thought),
            Just(ModelPurpose::CharacterGeneration),
        ]
    }

    /// Theme のストラテジー
    fn arb_theme() -> impl Strategy<Value = Theme> {
        prop_oneof![Just(Theme::Light), Just(Theme::Dark),]
    }

    /// AppConfig のストラテジー（proptest のタプル上限12に対応するため分割）
    fn arb_app_config() -> impl Strategy<Value = AppConfig> {
        let models_strategy = (
            arb_model_settings(),
            arb_model_settings(),
            arb_model_settings(),
            arb_model_settings(),
        );

        let misc_strategy = (
            any::<bool>(),
            1u64..3600,
            any::<bool>(),
            1u64..60,
            1u32..200,
            any::<bool>(),
            arb_theme(),
            prop_oneof![Just("ja".to_string()), Just("en".to_string())],
            proptest::collection::vec("[a-z]{3,10}", 0..5),
            1u64..20 * 1024 * 1024,
            proptest::collection::vec(
                prop_oneof![
                    Just("txt".to_string()),
                    Just("md".to_string()),
                    Just("csv".to_string()),
                    Just("pdf".to_string()),
                    Just("png".to_string()),
                    Just("jpg".to_string()),
                    Just("webp".to_string()),
                ],
                1..8
            ),
        );

        (models_strategy, misc_strategy).prop_map(
            |(
                (chat_settings, memory_settings, thought_settings, chargen_settings),
                (
                    spont_enabled,
                    spont_interval,
                    thought_enabled,
                    thought_interval,
                    compression_threshold,
                    tts_enabled,
                    theme,
                    language,
                    enabled_plugins,
                    max_file_size,
                    allowed_extensions,
                ),
            )| {
                let mut models = HashMap::new();
                models.insert(ModelPurpose::Chat, chat_settings);
                models.insert(ModelPurpose::Memory, memory_settings);
                models.insert(ModelPurpose::Thought, thought_settings);
                models.insert(ModelPurpose::CharacterGeneration, chargen_settings);

                AppConfig {
                    models,
                    spontaneous: SpontaneousConfig {
                        enabled: spont_enabled,
                        min_interval_seconds: spont_interval,
                    },
                    thought: ThoughtConfig {
                        enabled: thought_enabled,
                        interval_minutes: thought_interval,
                    },
                    memory: MemoryConfig {
                        compression_threshold,
                    },
                    tts: TTSGlobalConfig { enabled: tts_enabled },
                    ui: UIConfig { theme, language },
                    plugins: PluginsConfig {
                        enabled_plugins,
                        plugin_settings: HashMap::new(),
                    },
                    attachment: AttachmentConfig {
                        max_file_size_bytes: max_file_size,
                        allowed_extensions,
                    },
                }
            },
        )
    }

    // ========================================
    // Property 15: Model config per-purpose isolation
    // ========================================
    // **Validates: Requirements 7.1**
    //
    // For any set of ModelSettings configured for different purposes,
    // updating the config for one purpose SHALL NOT affect the configs
    // of other purposes.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// 1つの用途の設定を更新しても、他の用途の設定は変化しない
        #[test]
        fn prop_updating_one_purpose_does_not_affect_others(
            initial_config in arb_app_config(),
            new_settings in arb_model_settings(),
            target_purpose in arb_model_purpose(),
        ) {
            // 環境変数の影響を排除するためtempfileを使用
            let tmp_dir = tempfile::tempdir().unwrap();
            let config_path = tmp_dir.path().join("config.json");

            let manager = ModelConfigManager::new(config_path).unwrap();
            manager.set_config(initial_config.clone()).unwrap();

            // 更新前の他の用途の設定を記録
            let all_purposes = vec![
                ModelPurpose::Chat,
                ModelPurpose::Memory,
                ModelPurpose::Thought,
                ModelPurpose::CharacterGeneration,
            ];

            let other_purposes: Vec<_> = all_purposes
                .iter()
                .filter(|p| **p != target_purpose)
                .collect();

            let before_others: Vec<_> = other_purposes
                .iter()
                .map(|p| {
                    let s = manager.get_model_settings(p).unwrap();
                    ((*p).clone(), s.base_url.clone(), s.model.clone(), s.api_key.clone(), s.temperature)
                })
                .collect();

            // target_purpose の設定を更新
            let mut updated_config = manager.get_config();
            updated_config.models.insert(target_purpose.clone(), new_settings.clone());
            manager.set_config(updated_config).unwrap();

            // 他の用途の設定が変化していないことを検証
            for (purpose, base_url, model, api_key, temperature) in &before_others {
                let after = manager.get_model_settings(purpose).unwrap();
                prop_assert_eq!(&after.base_url, base_url,
                    "base_url changed for {:?} after updating {:?}", purpose, target_purpose);
                prop_assert_eq!(&after.model, model,
                    "model changed for {:?} after updating {:?}", purpose, target_purpose);
                prop_assert_eq!(&after.api_key, api_key,
                    "api_key changed for {:?} after updating {:?}", purpose, target_purpose);
                prop_assert!(
                    (after.temperature - temperature).abs() < f32::EPSILON,
                    "temperature changed for {:?} after updating {:?}", purpose, target_purpose
                );
            }

            // target_purpose の設定が正しく更新されていることも検証
            let target_after = manager.get_model_settings(&target_purpose).unwrap();
            prop_assert_eq!(&target_after.base_url, &new_settings.base_url);
            prop_assert_eq!(&target_after.model, &new_settings.model);
            prop_assert_eq!(&target_after.api_key, &new_settings.api_key);
            prop_assert!(
                (target_after.temperature - new_settings.temperature).abs() < f32::EPSILON,
                "target temperature not updated correctly"
            );
        }
    }

    // ========================================
    // Property 16: Model config round-trip
    // ========================================
    // **Validates: Requirements 7.2**
    //
    // For any valid AppConfig, serializing to JSON and deserializing back
    // SHALL produce an equivalent AppConfig.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// AppConfig の JSON シリアライズ → デシリアライズで等価なオブジェクトが復元される
        #[test]
        fn prop_app_config_json_round_trip(
            config in arb_app_config(),
        ) {
            let json = serde_json::to_string_pretty(&config).unwrap();
            let deserialized: AppConfig = serde_json::from_str(&json).unwrap();

            // models の各用途を検証
            let purposes = vec![
                ModelPurpose::Chat,
                ModelPurpose::Memory,
                ModelPurpose::Thought,
                ModelPurpose::CharacterGeneration,
            ];

            for purpose in &purposes {
                let original = config.models.get(purpose).unwrap();
                let restored = deserialized.models.get(purpose).unwrap();

                prop_assert_eq!(&restored.base_url, &original.base_url,
                    "base_url mismatch for {:?}", purpose);
                prop_assert_eq!(&restored.model, &original.model,
                    "model mismatch for {:?}", purpose);
                prop_assert_eq!(&restored.api_key, &original.api_key,
                    "api_key mismatch for {:?}", purpose);
                prop_assert!(
                    (restored.temperature - original.temperature).abs() < f32::EPSILON,
                    "temperature mismatch for {:?}: {} vs {}",
                    purpose, restored.temperature, original.temperature
                );
            }

            // その他の設定フィールドを検証
            prop_assert_eq!(deserialized.spontaneous.enabled, config.spontaneous.enabled);
            prop_assert_eq!(
                deserialized.spontaneous.min_interval_seconds,
                config.spontaneous.min_interval_seconds
            );
            prop_assert_eq!(deserialized.thought.enabled, config.thought.enabled);
            prop_assert_eq!(deserialized.thought.interval_minutes, config.thought.interval_minutes);
            prop_assert_eq!(
                deserialized.memory.compression_threshold,
                config.memory.compression_threshold
            );
            prop_assert_eq!(deserialized.tts.enabled, config.tts.enabled);
            prop_assert_eq!(deserialized.ui.theme, config.ui.theme);
            prop_assert_eq!(&deserialized.ui.language, &config.ui.language);
            prop_assert_eq!(
                &deserialized.plugins.enabled_plugins,
                &config.plugins.enabled_plugins
            );
            prop_assert_eq!(
                deserialized.attachment.max_file_size_bytes,
                config.attachment.max_file_size_bytes
            );
            prop_assert_eq!(
                &deserialized.attachment.allowed_extensions,
                &config.attachment.allowed_extensions
            );
        }

        /// ModelConfigManager を通した保存→ロードのラウンドトリップ
        #[test]
        fn prop_model_config_file_round_trip(
            config in arb_app_config(),
        ) {
            let tmp_dir = tempfile::tempdir().unwrap();
            let config_path = tmp_dir.path().join("config.json");

            // 保存
            let manager = ModelConfigManager::new(config_path.clone()).unwrap();
            manager.set_config(config.clone()).unwrap();

            // 環境変数の影響を排除
            let env_prefixes = ["AI_CHAT_LLM", "AI_CHAT_MEMORY", "AI_CHAT_THOUGHT", "AI_CHAT_CHARGEN"];
            let suffixes = ["_BASE_URL", "_API_KEY", "_MODEL"];
            for prefix in &env_prefixes {
                for suffix in &suffixes {
                    std::env::remove_var(format!("{}{}", prefix, suffix));
                }
            }

            // 新しいManagerでロード
            let manager2 = ModelConfigManager::new(config_path).unwrap();
            let loaded = manager2.get_config();

            // models の各用途を検証
            let purposes = vec![
                ModelPurpose::Chat,
                ModelPurpose::Memory,
                ModelPurpose::Thought,
                ModelPurpose::CharacterGeneration,
            ];

            for purpose in &purposes {
                let original = config.models.get(purpose).unwrap();
                let restored = loaded.models.get(purpose).unwrap();

                prop_assert_eq!(&restored.base_url, &original.base_url,
                    "file round-trip: base_url mismatch for {:?}", purpose);
                prop_assert_eq!(&restored.model, &original.model,
                    "file round-trip: model mismatch for {:?}", purpose);
                prop_assert_eq!(&restored.api_key, &original.api_key,
                    "file round-trip: api_key mismatch for {:?}", purpose);
                prop_assert!(
                    (restored.temperature - original.temperature).abs() < f32::EPSILON,
                    "file round-trip: temperature mismatch for {:?}", purpose
                );
            }

            // その他のフィールド
            prop_assert_eq!(loaded.spontaneous.enabled, config.spontaneous.enabled);
            prop_assert_eq!(
                loaded.spontaneous.min_interval_seconds,
                config.spontaneous.min_interval_seconds
            );
            prop_assert_eq!(loaded.thought.enabled, config.thought.enabled);
            prop_assert_eq!(loaded.thought.interval_minutes, config.thought.interval_minutes);
            prop_assert_eq!(
                loaded.memory.compression_threshold,
                config.memory.compression_threshold
            );
            prop_assert_eq!(loaded.tts.enabled, config.tts.enabled);
            prop_assert_eq!(loaded.ui.theme, config.ui.theme);
            prop_assert_eq!(&loaded.ui.language, &config.ui.language);
            prop_assert_eq!(
                &loaded.plugins.enabled_plugins,
                &config.plugins.enabled_plugins
            );
            prop_assert_eq!(
                loaded.attachment.max_file_size_bytes,
                config.attachment.max_file_size_bytes
            );
            prop_assert_eq!(
                &loaded.attachment.allowed_extensions,
                &config.attachment.allowed_extensions
            );
        }
    }
}
