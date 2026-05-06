// Model Config tests

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::model_config::ModelConfigManager;
    use crate::models::config::{
        AppConfig, AttachmentConfig, MemoryConfig, ModelPurpose, ModelSettings, PluginsConfig,
        SendKey, SpontaneousConfig, TTSGlobalConfig, Theme, ThoughtConfig, UIConfig,
    };

    #[test]
    fn test_mask_api_key_empty() {
        assert_eq!(ModelConfigManager::mask_api_key(""), "");
    }

    #[test]
    fn test_mask_api_key_short() {
        assert_eq!(ModelConfigManager::mask_api_key("abc"), "***");
        assert_eq!(ModelConfigManager::mask_api_key("abcd"), "***");
    }

    #[test]
    fn test_mask_api_key_with_prefix() {
        let masked = ModelConfigManager::mask_api_key("sk-abcdefghijk");
        assert_eq!(masked, "sk-a***k");
    }

    #[test]
    fn test_mask_api_key_without_prefix() {
        let masked = ModelConfigManager::mask_api_key("abcdefghijk");
        assert_eq!(masked, "a***k");
    }

    #[test]
    fn test_mask_api_key_long_key() {
        let masked =
            ModelConfigManager::mask_api_key("sk-proj-abc123def456ghi789jkl012mno345pqr678");
        assert!(masked.starts_with("sk-"));
        assert!(masked.contains("***"));
        // 先頭1文字と末尾1文字が見える
        assert_eq!(masked, "sk-p***8");
    }

    #[test]
    fn test_default_config_has_all_purposes() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("config.json");

        let manager = ModelConfigManager::new(config_path).unwrap();
        let config = manager.get_config();

        assert!(config.models.contains_key(&ModelPurpose::Chat));
        assert!(config.models.contains_key(&ModelPurpose::Memory));
        assert!(config.models.contains_key(&ModelPurpose::Thought));
        assert!(config.models.contains_key(&ModelPurpose::CharacterGeneration));
    }

    #[test]
    fn test_default_config_values() {
        // 環境変数の影響を排除
        let env_prefixes = ["AI_CHAT_LLM", "AI_CHAT_MEMORY", "AI_CHAT_THOUGHT", "AI_CHAT_CHARGEN"];
        let suffixes = ["_BASE_URL", "_API_KEY", "_MODEL"];
        for prefix in &env_prefixes {
            for suffix in &suffixes {
                std::env::remove_var(format!("{}{}", prefix, suffix));
            }
        }

        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("config.json");

        let manager = ModelConfigManager::new(config_path).unwrap();
        let config = manager.get_config();

        // 各用途のデフォルト値確認
        for (_purpose, settings) in &config.models {
            assert_eq!(settings.base_url, "");
            assert_eq!(settings.model, "");
            assert_eq!(settings.api_key, None);
            assert!((settings.temperature - 0.7).abs() < f32::EPSILON);
        }

        // その他のデフォルト値
        assert!(!config.spontaneous.enabled);
        assert_eq!(config.spontaneous.min_interval_seconds, 60);
        assert!(!config.thought.enabled);
        assert_eq!(config.thought.interval_minutes, 5);
        assert_eq!(config.memory.compression_threshold, 50);
        assert!(!config.tts.enabled);
        assert_eq!(config.ui.theme, Theme::Dark);
        assert_eq!(config.ui.language, "ja");
        assert!(config.plugins.enabled_plugins.is_empty());
        assert_eq!(config.attachment.max_file_size_bytes, 10 * 1024 * 1024);
        assert_eq!(config.attachment.allowed_extensions.len(), 7);
    }

    #[test]
    fn test_save_and_load_config() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("subdir").join("config.json");

        let manager = ModelConfigManager::new(config_path.clone()).unwrap();

        // 設定変更
        let mut config = manager.get_config();
        config
            .models
            .get_mut(&ModelPurpose::Chat)
            .unwrap()
            .base_url = "http://localhost:8080".to_string();
        config.models.get_mut(&ModelPurpose::Chat).unwrap().model =
            "llama3".to_string();
        config.ui.language = "en".to_string();

        manager.set_config(config.clone()).unwrap();

        // 新しいManagerで再ロード
        let manager2 = ModelConfigManager::new(config_path).unwrap();
        let loaded = manager2.get_config();

        assert_eq!(
            loaded.models.get(&ModelPurpose::Chat).unwrap().base_url,
            "http://localhost:8080"
        );
        assert_eq!(
            loaded.models.get(&ModelPurpose::Chat).unwrap().model,
            "llama3"
        );
        assert_eq!(loaded.ui.language, "en");
    }

    #[test]
    fn test_serialization_round_trip() {
        let mut models = HashMap::new();
        models.insert(
            ModelPurpose::Chat,
            ModelSettings {
                base_url: "http://localhost:1234/v1".to_string(),
                model: "gpt-4".to_string(),
                api_key: Some("sk-test123".to_string()),
                temperature: 0.9,
            },
        );
        models.insert(
            ModelPurpose::Memory,
            ModelSettings {
                base_url: "http://localhost:5678/v1".to_string(),
                model: "llama3".to_string(),
                api_key: None,
                temperature: 0.3,
            },
        );
        models.insert(
            ModelPurpose::Thought,
            ModelSettings {
                base_url: String::new(),
                model: String::new(),
                api_key: None,
                temperature: 0.7,
            },
        );
        models.insert(
            ModelPurpose::CharacterGeneration,
            ModelSettings {
                base_url: String::new(),
                model: String::new(),
                api_key: None,
                temperature: 0.7,
            },
        );

        let config = AppConfig {
            models,
            spontaneous: SpontaneousConfig {
                enabled: true,
                min_interval_seconds: 120,
                probability: 0.5,
            },
            thought: ThoughtConfig {
                enabled: true,
                interval_minutes: 10,
                auto_delete_threshold_minutes: 1440,
            },
            memory: MemoryConfig {
                compression_threshold: 100,
            },
            tts: TTSGlobalConfig { enabled: true },
            ui: UIConfig {
                theme: Theme::Light,
                language: "en".to_string(),
                send_key: SendKey::default(),
            },
            plugins: PluginsConfig {
                enabled_plugins: vec!["calculator".to_string()],
                plugin_settings: HashMap::new(),
            },
            attachment: AttachmentConfig {
                max_file_size_bytes: 5 * 1024 * 1024,
                allowed_extensions: vec!["txt".to_string(), "pdf".to_string()],
            },
        };

        // JSON round-trip
        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(
            deserialized.models.get(&ModelPurpose::Chat).unwrap().base_url,
            "http://localhost:1234/v1"
        );
        assert_eq!(
            deserialized
                .models
                .get(&ModelPurpose::Chat)
                .unwrap()
                .api_key,
            Some("sk-test123".to_string())
        );
        assert_eq!(
            deserialized
                .models
                .get(&ModelPurpose::Memory)
                .unwrap()
                .model,
            "llama3"
        );
        assert!(deserialized.spontaneous.enabled);
        assert_eq!(deserialized.spontaneous.min_interval_seconds, 120);
        assert_eq!(deserialized.ui.theme, Theme::Light);
        assert_eq!(deserialized.plugins.enabled_plugins, vec!["calculator"]);
    }

    #[test]
    fn test_get_model_settings() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("config.json");

        let manager = ModelConfigManager::new(config_path).unwrap();

        let chat_settings = manager.get_model_settings(&ModelPurpose::Chat);
        assert!(chat_settings.is_some());

        let settings = chat_settings.unwrap();
        assert_eq!(settings.base_url, "");
        assert!((settings.temperature - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_env_fallback() {
        // 環境変数を設定
        std::env::set_var("AI_CHAT_LLM_BASE_URL", "http://env-llm:8080/v1");
        std::env::set_var("AI_CHAT_LLM_API_KEY", "env-key-123");
        std::env::set_var("AI_CHAT_LLM_MODEL", "env-model");

        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("config.json");

        let manager = ModelConfigManager::new(config_path).unwrap();
        let config = manager.get_config();

        let chat = config.models.get(&ModelPurpose::Chat).unwrap();
        assert_eq!(chat.base_url, "http://env-llm:8080/v1");
        assert_eq!(chat.api_key, Some("env-key-123".to_string()));
        assert_eq!(chat.model, "env-model");

        // クリーンアップ
        std::env::remove_var("AI_CHAT_LLM_BASE_URL");
        std::env::remove_var("AI_CHAT_LLM_API_KEY");
        std::env::remove_var("AI_CHAT_LLM_MODEL");
    }

    #[test]
    fn test_env_does_not_override_existing_config() {
        // 環境変数を設定
        std::env::set_var("AI_CHAT_MEMORY_LLM_BASE_URL", "http://env-memory:8080/v1");

        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("config.json");

        // 既存設定を持つconfigを保存
        let manager = ModelConfigManager::new(config_path.clone()).unwrap();
        let mut config = manager.get_config();
        config
            .models
            .get_mut(&ModelPurpose::Memory)
            .unwrap()
            .base_url = "http://existing:9090/v1".to_string();
        manager.set_config(config).unwrap();

        // 再ロード — 非空のconfig値は環境変数で上書きされない
        let manager2 = ModelConfigManager::new(config_path).unwrap();
        let loaded = manager2.get_config();
        assert_eq!(
            loaded.models.get(&ModelPurpose::Memory).unwrap().base_url,
            "http://existing:9090/v1"
        );

        // クリーンアップ
        std::env::remove_var("AI_CHAT_MEMORY_LLM_BASE_URL");
    }

    #[test]
    fn test_creates_directory_on_save() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("deep").join("nested").join("config.json");

        let manager = ModelConfigManager::new(config_path.clone()).unwrap();
        manager.save().unwrap();

        assert!(config_path.exists());
    }
}
