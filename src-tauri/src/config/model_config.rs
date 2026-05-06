// Model Config - 用途別モデル設定管理

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::AppError;
use crate::models::config::{
    AppConfig, AttachmentConfig, MemoryConfig, ModelPurpose, ModelSettings, PluginsConfig,
    SendKey, SpontaneousConfig, TTSGlobalConfig, Theme, ThoughtConfig, UIConfig,
};

/// モデル設定管理
pub struct ModelConfigManager {
    config_path: PathBuf,
    config: Mutex<AppConfig>,
}

impl ModelConfigManager {
    /// 新規作成。config_pathからロードし、なければデフォルト値を使用。
    pub fn new(config_path: PathBuf) -> Result<Self, AppError> {
        let config = Self::load_or_default(&config_path)?;
        Ok(Self {
            config_path,
            config: Mutex::new(config),
        })
    }

    /// テスト用: 指定した設定で作成（ファイルI/Oなし）
    pub fn new_with_config(config: AppConfig) -> Self {
        Self {
            config_path: PathBuf::from("/dev/null"),
            config: Mutex::new(config),
        }
    }

    /// 設定ファイルからロード。存在しなければデフォルト設定を返す。
    /// 環境変数によるフォールバックも適用。
    pub fn load_or_default(config_path: &Path) -> Result<AppConfig, AppError> {
        let mut config = if config_path.exists() {
            let content = std::fs::read_to_string(config_path)?;
            serde_json::from_str(&content)?
        } else {
            Self::default_config()
        };

        // 環境変数フォールバック適用
        Self::apply_env_fallback(&mut config);

        Ok(config)
    }

    /// 設定をファイルに保存。ディレクトリが存在しなければ作成。
    pub fn save(&self) -> Result<(), AppError> {
        if let Some(parent) = self.config_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let config = self.config.lock().map_err(|e| {
            AppError::Io(format!("Failed to acquire config lock: {}", e))
        })?;

        let json = serde_json::to_string_pretty(&*config)?;
        std::fs::write(&self.config_path, json)?;
        Ok(())
    }

    /// 現在の設定を取得
    pub fn get_config(&self) -> AppConfig {
        self.config
            .lock()
            .expect("Failed to acquire config lock")
            .clone()
    }

    /// 設定を更新して保存
    pub fn set_config(&self, config: AppConfig) -> Result<(), AppError> {
        {
            let mut current = self.config.lock().map_err(|e| {
                AppError::Io(format!("Failed to acquire config lock: {}", e))
            })?;
            *current = config;
        }
        self.save()
    }

    /// 指定用途のモデル設定を取得
    pub fn get_model_settings(&self, purpose: &ModelPurpose) -> Option<ModelSettings> {
        self.config
            .lock()
            .expect("Failed to acquire config lock")
            .models
            .get(purpose)
            .cloned()
    }

    /// APIキーをマスク表示用に変換。
    /// 例: "sk-abcdefghijk" → "sk-a***k"
    /// 短いキー（4文字以下）は全て"***"に置換。
    pub fn mask_api_key(api_key: &str) -> String {
        if api_key.is_empty() {
            return String::new();
        }

        let len = api_key.len();
        if len <= 4 {
            return "***".to_string();
        }

        // プレフィックス検出（"sk-"等）
        let (prefix, rest) = if let Some(pos) = api_key.find('-') {
            if pos < len / 2 {
                let (p, r) = api_key.split_at(pos + 1);
                (p.to_string(), r)
            } else {
                (String::new(), api_key)
            }
        } else {
            (String::new(), api_key)
        };

        let rest_len = rest.len();
        if rest_len <= 2 {
            return format!("{}***", prefix);
        }

        let first = &rest[..1];
        let last = &rest[rest_len - 1..];
        format!("{}{}***{}", prefix, first, last)
    }

    /// デフォルト設定を生成
    fn default_config() -> AppConfig {
        let mut models = HashMap::new();

        let default_settings = ModelSettings {
            base_url: String::new(),
            model: String::new(),
            api_key: None,
            temperature: 0.7,
        };

        models.insert(ModelPurpose::Chat, default_settings.clone());
        models.insert(ModelPurpose::Memory, default_settings.clone());
        models.insert(ModelPurpose::Thought, default_settings.clone());
        models.insert(ModelPurpose::CharacterGeneration, default_settings);

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
            tts: TTSGlobalConfig { enabled: false },
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
                allowed_extensions: vec![
                    "txt".to_string(),
                    "md".to_string(),
                    "csv".to_string(),
                    "pdf".to_string(),
                    "png".to_string(),
                    "jpg".to_string(),
                    "webp".to_string(),
                ],
            },
        }
    }

    /// 環境変数からの設定フォールバック適用
    fn apply_env_fallback(config: &mut AppConfig) {
        Self::apply_env_for_purpose(config, ModelPurpose::Chat, "AI_CHAT_LLM");
        Self::apply_env_for_purpose(config, ModelPurpose::Memory, "AI_CHAT_MEMORY_LLM");
        Self::apply_env_for_purpose(config, ModelPurpose::Thought, "AI_CHAT_THOUGHT_LLM");
        Self::apply_env_for_purpose(
            config,
            ModelPurpose::CharacterGeneration,
            "AI_CHAT_CHARGEN_LLM",
        );
    }

    /// 特定用途の環境変数フォールバックを適用。
    /// 設定値が空の場合のみ環境変数を適用する（非空値は保持）。
    fn apply_env_for_purpose(config: &mut AppConfig, purpose: ModelPurpose, prefix: &str) {
        let settings = config
            .models
            .entry(purpose)
            .or_insert_with(|| ModelSettings {
                base_url: String::new(),
                model: String::new(),
                api_key: None,
                temperature: 0.7,
            });

        // 環境変数はフォールバックとしてのみ適用（既存の非空値は保持）
        if settings.base_url.is_empty() {
            if let Ok(val) = std::env::var(format!("{}_BASE_URL", prefix)) {
                if !val.is_empty() {
                    println!("[config] env fallback: {}_BASE_URL = {}", prefix, val);
                    settings.base_url = val;
                }
            }
        }

        if settings.api_key.is_none() {
            if let Ok(val) = std::env::var(format!("{}_API_KEY", prefix)) {
                if !val.is_empty() {
                    println!("[config] env fallback: {}_API_KEY = (set)", prefix);
                    settings.api_key = Some(val);
                }
            }
        }

        if settings.model.is_empty() {
            if let Ok(val) = std::env::var(format!("{}_MODEL", prefix)) {
                if !val.is_empty() {
                    println!("[config] env fallback: {}_MODEL = {}", prefix, val);
                    settings.model = val;
                }
            }
        }
    }
}
