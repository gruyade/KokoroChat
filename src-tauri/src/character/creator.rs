// Character Creator - キャラクター作成・管理

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::db::database::Database;
use crate::db::repositories::character as char_repo;
use crate::error::AppError;
use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
use crate::models::{Character, CharacterUpdate};

/// キャラクター作成・管理trait
#[async_trait]
pub trait CharacterCreator: Send + Sync {
    /// LLMを使用してキャラクターのシステムプロンプトを自動生成
    async fn generate_system_prompt(
        &self,
        name: &str,
        description: &str,
    ) -> Result<String, AppError>;

    /// キャラクターを保存し、character_idを返す
    async fn save_character(&self, character: &Character) -> Result<String, AppError>;

    /// キャラクターを部分更新
    async fn update_character(&self, id: &str, updates: CharacterUpdate) -> Result<(), AppError>;

    /// キャラクターを削除（CASCADE DELETEにより関連データも全削除）
    async fn delete_character(&self, id: &str) -> Result<(), AppError>;

    /// 全キャラクター一覧取得
    async fn list_characters(&self) -> Result<Vec<Character>, AppError>;

    /// IDでキャラクター取得
    async fn get_character(&self, id: &str) -> Result<Option<Character>, AppError>;
}

/// デフォルトのCharacterCreator実装
pub struct DefaultCharacterCreator {
    db: Arc<Mutex<Database>>,
    llm_client: Arc<dyn LLMClient>,
    config_manager: Arc<crate::config::model_config::ModelConfigManager>,
}

impl DefaultCharacterCreator {
    pub fn new(
        db: Arc<Mutex<Database>>,
        llm_client: Arc<dyn LLMClient>,
        config_manager: Arc<crate::config::model_config::ModelConfigManager>,
    ) -> Self {
        Self {
            db,
            llm_client,
            config_manager,
        }
    }

    /// 現在のCharacterGeneration用LLM設定を取得
    fn current_llm_config(&self) -> LLMClientConfig {
        self.config_manager
            .get_model_settings(&crate::models::config::ModelPurpose::CharacterGeneration)
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

    /// キャラクター生成用のメタプロンプトを構築
    fn build_meta_prompt(name: &str, description: &str) -> String {
        format!(
            "あなたはAIキャラクター設計の専門家です。\n\
             以下の情報をもとに、キャラクターのシステムプロンプトを作成してください。\n\
             \n\
             【キャラクター名】{}\n\
             【概要説明】{}\n\
             \n\
             以下の要素を含むシステムプロンプトを生成してください：\n\
             1. キャラクターの性格・人格（personality）\n\
             2. 背景設定（background）\n\
             3. 話し方・口調のパターン（speech patterns）\n\
             4. 行動原理・価値観\n\
             5. 会話における振る舞いのガイドライン\n\
             \n\
             システムプロンプトのみを出力してください。説明や前置きは不要です。",
            name, description
        )
    }
}

#[async_trait]
impl CharacterCreator for DefaultCharacterCreator {
    async fn generate_system_prompt(
        &self,
        name: &str,
        description: &str,
    ) -> Result<String, AppError> {
        let meta_prompt = Self::build_meta_prompt(name, description);

        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: meta_prompt,
            tool_call_id: None,
            images: None,
        }];

        let response = self
            .llm_client
            .chat(&messages, &self.current_llm_config(), None)
            .await?;

        match response {
            LLMResponse::Text { content: text, .. } => Ok(text),
            LLMResponse::ToolCalls { calls: _, .. } => Err(AppError::LlmApi(
                "Unexpected tool_call response during system prompt generation".to_string(),
            )),
        }
    }

    async fn save_character(&self, character: &Character) -> Result<String, AppError> {
        let db = self.db.lock().await;
        char_repo::insert_character(db.connection(), character)?;
        Ok(character.id.clone())
    }

    async fn update_character(&self, id: &str, updates: CharacterUpdate) -> Result<(), AppError> {
        let db = self.db.lock().await;
        char_repo::update_character(db.connection(), id, &updates)?;
        Ok(())
    }

    async fn delete_character(&self, id: &str) -> Result<(), AppError> {
        let db = self.db.lock().await;
        char_repo::delete_character(db.connection(), id)?;
        Ok(())
    }

    async fn list_characters(&self) -> Result<Vec<Character>, AppError> {
        let db = self.db.lock().await;
        char_repo::list_characters(db.connection())
    }

    async fn get_character(&self, id: &str) -> Result<Option<Character>, AppError> {
        let db = self.db.lock().await;
        char_repo::get_character(db.connection(), id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ToolDefinition;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// テスト用のモックLLMクライアント
    struct MockLLMClient {
        response: String,
        call_count: AtomicUsize,
    }

    impl MockLLMClient {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
                call_count: AtomicUsize::new(0),
            }
        }

        fn get_call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
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
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(LLMResponse::Text { content: self.response.clone(), thinking: None })
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
            _callbacks: crate::llm::client::StreamCallbacks,
        ) -> Result<LLMResponse, AppError> {
            Ok(LLMResponse::Text { content: self.response.clone(), thinking: None })
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn test_config_manager() -> Arc<crate::config::model_config::ModelConfigManager> {
        use crate::models::config::*;
        use std::collections::HashMap;

        let mut models = HashMap::new();
        let settings = ModelSettings {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: None,
        };
        models.insert(ModelPurpose::Chat, settings.clone());
        models.insert(ModelPurpose::Memory, settings.clone());
        models.insert(ModelPurpose::Thought, settings.clone());
        models.insert(ModelPurpose::CharacterGeneration, settings);

        let config = AppConfig {
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
                enabled: false,
                voicepeak_path: None,
                timeout_seconds: 60,
                max_chunk_size: 140,
                irodori_base_url: None,
                irodori_caption_base_url: None,
                irodori_reference_audio_base_url: None,
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
                allowed_extensions: vec![],
            },
        };

        Arc::new(crate::config::model_config::ModelConfigManager::new_with_config(config))
    }

    fn sample_character() -> Character {
        let now = chrono::Utc::now().to_rfc3339();
        Character {
            id: uuid::Uuid::new_v4().to_string(),
            name: "テストキャラ".to_string(),
            description: "テスト用のキャラクター".to_string(),
            system_prompt: "あなたはテストキャラです。".to_string(),
            avatar_path: None,
            tts_config: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_generate_system_prompt() {
        let mock_response = "あなたは元気な猫のキャラクターです。語尾に「にゃ」をつけて話します。";
        let llm_client = Arc::new(MockLLMClient::new(mock_response));
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let config = test_config_manager();

        let creator = DefaultCharacterCreator::new(db, llm_client.clone(), config);

        let result = creator
            .generate_system_prompt("ネコ助", "元気な猫のキャラクター")
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mock_response);
        assert_eq!(llm_client.get_call_count(), 1);
    }

    #[tokio::test]
    async fn test_save_character() {
        let llm_client = Arc::new(MockLLMClient::new(""));
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let config = test_config_manager();

        let creator = DefaultCharacterCreator::new(db.clone(), llm_client, config);
        let character = sample_character();
        let expected_id = character.id.clone();

        let result = creator.save_character(&character).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_id);

        // DB内に保存されていることを確認
        let db_lock = db.lock().await;
        let saved = char_repo::get_character(db_lock.connection(), &expected_id).unwrap();
        assert!(saved.is_some());
        assert_eq!(saved.unwrap().name, "テストキャラ");
    }

    #[tokio::test]
    async fn test_list_characters() {
        let llm_client = Arc::new(MockLLMClient::new(""));
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let config = test_config_manager();

        let creator = DefaultCharacterCreator::new(db.clone(), llm_client, config);

        let c1 = sample_character();
        let mut c2 = sample_character();
        c2.name = "キャラ2".to_string();

        creator.save_character(&c1).await.unwrap();
        creator.save_character(&c2).await.unwrap();

        let list = creator.list_characters().await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_get_character() {
        let llm_client = Arc::new(MockLLMClient::new(""));
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let config = test_config_manager();

        let creator = DefaultCharacterCreator::new(db, llm_client, config);
        let character = sample_character();
        let id = character.id.clone();

        creator.save_character(&character).await.unwrap();

        let result = creator.get_character(&id).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, id);

        // 存在しないID
        let not_found = creator.get_character("nonexistent").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_update_character() {
        let llm_client = Arc::new(MockLLMClient::new(""));
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let config = test_config_manager();

        let creator = DefaultCharacterCreator::new(db, llm_client, config);
        let character = sample_character();
        let id = character.id.clone();

        creator.save_character(&character).await.unwrap();

        let updates = CharacterUpdate {
            name: Some("更新後の名前".to_string()),
            description: None,
            system_prompt: Some("新しいプロンプト".to_string()),
            avatar_path: None,
            tts_config: None,
            clear_avatar: None,
            clear_tts: None,
        };

        creator.update_character(&id, updates).await.unwrap();

        let updated = creator.get_character(&id).await.unwrap().unwrap();
        assert_eq!(updated.name, "更新後の名前");
        assert_eq!(updated.system_prompt, "新しいプロンプト");
        // 変更していないフィールドは元のまま
        assert_eq!(updated.description, "テスト用のキャラクター");
    }

    #[tokio::test]
    async fn test_delete_character() {
        let llm_client = Arc::new(MockLLMClient::new(""));
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let config = test_config_manager();

        let creator = DefaultCharacterCreator::new(db, llm_client, config);
        let character = sample_character();
        let id = character.id.clone();

        creator.save_character(&character).await.unwrap();
        creator.delete_character(&id).await.unwrap();

        let result = creator.get_character(&id).await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_build_meta_prompt_contains_name_and_description() {
        let prompt = DefaultCharacterCreator::build_meta_prompt("テスト太郎", "明るい性格の少年");
        assert!(prompt.contains("テスト太郎"));
        assert!(prompt.contains("明るい性格の少年"));
        assert!(prompt.contains("性格"));
        assert!(prompt.contains("背景設定"));
        assert!(prompt.contains("話し方"));
    }
}
