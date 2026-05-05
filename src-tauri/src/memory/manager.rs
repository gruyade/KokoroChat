// Memory Manager - 記憶管理

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::Mutex as TokioMutex;

use crate::db::database::Database;
use crate::db::repositories::{chat as chat_repo, memory as memory_repo};
use crate::error::AppError;
use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
use crate::models::Memory;

/// 記憶管理trait
#[async_trait]
pub trait MemoryManager: Send + Sync {
    /// メッセージ数が閾値に達した場合、会話を圧縮して記憶として保存
    async fn check_and_compress(&self, session_id: &str) -> Result<(), AppError>;

    /// キャラクターに関連する記憶を取得（現時点では全件返却）
    async fn get_relevant_memories(
        &self,
        character_id: &str,
        context: &str,
    ) -> Result<Vec<Memory>, AppError>;

    /// キャラクターの記憶一覧取得
    async fn list_memories(&self, character_id: &str) -> Result<Vec<Memory>, AppError>;

    /// 記憶の内容を更新
    async fn update_memory(&self, id: &str, content: &str) -> Result<(), AppError>;

    /// 記憶を削除
    async fn delete_memory(&self, id: &str) -> Result<(), AppError>;
}

/// デフォルトのMemoryManager実装
pub struct DefaultMemoryManager {
    db: Arc<Mutex<Database>>,
    llm_client: Arc<dyn LLMClient>,
    config_manager: Arc<crate::config::model_config::ModelConfigManager>,
    compression_threshold: u32,
    llm_lock: Arc<TokioMutex<()>>,
}

impl DefaultMemoryManager {
    pub fn new(
        db: Arc<Mutex<Database>>,
        llm_client: Arc<dyn LLMClient>,
        config_manager: Arc<crate::config::model_config::ModelConfigManager>,
        compression_threshold: u32,
        llm_lock: Arc<TokioMutex<()>>,
    ) -> Self {
        Self {
            db,
            llm_client,
            config_manager,
            compression_threshold,
            llm_lock,
        }
    }

    /// 現在のMemory用LLM設定を取得
    fn current_llm_config(&self) -> LLMClientConfig {
        self.config_manager
            .get_model_settings(&crate::models::config::ModelPurpose::Memory)
            .map(|s| LLMClientConfig {
                base_url: s.base_url,
                model: s.model,
                api_key: s.api_key,
                temperature: s.temperature,
            })
            .unwrap_or(LLMClientConfig {
                base_url: String::new(),
                model: String::new(),
                api_key: None,
                temperature: 0.7,
            })
    }

    /// 圧縮用のシステムプロンプトを生成
    fn compression_system_prompt() -> String {
        "あなたは会話要約アシスタントです。以下の会話を分析し、重要な情報を簡潔に要約してください。\n\n\
         以下の観点で要約してください：\n\
         - ユーザーに関する重要な事実\n\
         - 議論された主要なトピック\n\
         - 表明された好みや意見\n\
         - 行われた約束やコミットメント\n\n\
         箇条書きで簡潔にまとめてください。"
            .to_string()
    }

    /// 会話メッセージを圧縮用テキストに変換
    fn format_messages_for_compression(
        messages: &[crate::models::ChatMessageRecord],
    ) -> String {
        messages
            .iter()
            .map(|msg| {
                let role_label = match msg.role {
                    crate::models::ChatRole::User => "ユーザー",
                    crate::models::ChatRole::Assistant => "アシスタント",
                    crate::models::ChatRole::Spontaneous => "アシスタント（自発）",
                    crate::models::ChatRole::Tool => "ツール",
                };
                format!("{}: {}", role_label, msg.content)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[async_trait]
impl MemoryManager for DefaultMemoryManager {
    async fn check_and_compress(&self, session_id: &str) -> Result<(), AppError> {
        // 1. セッションのメッセージを取得
        let (messages, character_id) = {
            let db = self.db.lock().map_err(|e| {
                AppError::Database(format!("Failed to lock database: {}", e))
            })?;
            let conn = db.connection();

            let messages = chat_repo::get_messages(conn, session_id)?;

            // セッション情報からcharacter_idを取得
            let session = chat_repo::get_session(conn, session_id)?
                .ok_or_else(|| AppError::NotFound(format!("Session not found: {}", session_id)))?;

            (messages, session.character_id)
        };

        // 2. メッセージ数が閾値未満なら何もしない
        if (messages.len() as u32) < self.compression_threshold {
            return Ok(());
        }

        // 3. メッセージを圧縮用テキストに変換
        let conversation_text = Self::format_messages_for_compression(&messages);

        // 4. LLMに要約を依頼
        let llm_messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: Self::compression_system_prompt(),
                tool_call_id: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: format!("以下の会話を要約してください：\n\n{}", conversation_text),
                tool_call_id: None,
            },
        ];

        // 4. LLMに要約を依頼（ロック取得で他のLLMリクエストと直列化）
        let _llm_guard = self.llm_lock.lock().await;

        let response = self
            .llm_client
            .chat(&llm_messages, &self.current_llm_config(), None)
            .await?;

        drop(_llm_guard);

        let summary = match response {
            LLMResponse::Text(text) => text,
            LLMResponse::ToolCalls(_) => {
                return Err(AppError::LlmApi(
                    "Unexpected tool call response during compression".to_string(),
                ));
            }
        };

        // 5. 要約をMemoryレコードとして保存
        let now = chrono::Utc::now().to_rfc3339();
        let memory = Memory {
            id: uuid::Uuid::new_v4().to_string(),
            character_id,
            content: summary,
            source_session_id: Some(session_id.to_string()),
            source_message_from: messages.first().map(|m| m.id.clone()),
            source_message_to: messages.last().map(|m| m.id.clone()),
            created_at: now.clone(),
            updated_at: now,
        };

        let db = self.db.lock().map_err(|e| {
            AppError::Database(format!("Failed to lock database: {}", e))
        })?;
        let conn = db.connection();
        memory_repo::insert_memory(conn, &memory)?;

        Ok(())
    }

    async fn get_relevant_memories(
        &self,
        character_id: &str,
        _context: &str,
    ) -> Result<Vec<Memory>, AppError> {
        // 現時点ではシンプルに全件返却
        let db = self.db.lock().map_err(|e| {
            AppError::Database(format!("Failed to lock database: {}", e))
        })?;
        let conn = db.connection();
        memory_repo::list_memories(conn, character_id)
    }

    async fn list_memories(&self, character_id: &str) -> Result<Vec<Memory>, AppError> {
        let db = self.db.lock().map_err(|e| {
            AppError::Database(format!("Failed to lock database: {}", e))
        })?;
        let conn = db.connection();
        memory_repo::list_memories(conn, character_id)
    }

    async fn update_memory(&self, id: &str, content: &str) -> Result<(), AppError> {
        let db = self.db.lock().map_err(|e| {
            AppError::Database(format!("Failed to lock database: {}", e))
        })?;
        let conn = db.connection();
        memory_repo::update_memory(conn, id, content)
    }

    async fn delete_memory(&self, id: &str) -> Result<(), AppError> {
        let db = self.db.lock().map_err(|e| {
            AppError::Database(format!("Failed to lock database: {}", e))
        })?;
        let conn = db.connection();
        memory_repo::delete_memory(conn, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatMessageRecord, ChatRole, ChatSession, ToolDefinition};

    /// テスト用MockLLMClient
    struct MockLLMClient {
        response: String,
    }

    impl MockLLMClient {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
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
            Ok(LLMResponse::Text(self.response.clone()))
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
            Ok(self.response.clone())
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn setup_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();
        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "char-001",
                "TestChar",
                "A test character",
                "You are a test character.",
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z"
            ],
        )
        .unwrap();
        db
    }

    fn create_session(db: &Database, session_id: &str) {
        let conn = db.connection();
        let session = ChatSession {
            id: session_id.to_string(),
            character_id: "char-001".to_string(),
            title: Some("Test Session".to_string()),
            last_message_at: None,
            last_message_preview: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        chat_repo::insert_session(conn, &session).unwrap();
    }

    fn insert_messages(db: &Database, session_id: &str, count: u32) {
        let conn = db.connection();
        for i in 0..count {
            let msg = ChatMessageRecord {
                id: format!("msg-{:03}", i),
                session_id: session_id.to_string(),
                role: if i % 2 == 0 {
                    ChatRole::User
                } else {
                    ChatRole::Assistant
                },
                content: format!("Message {}", i),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: format!("2024-01-01T{:02}:{:02}:00Z", i / 60, i % 60),
            };
            chat_repo::insert_message(conn, &msg).unwrap();
        }
    }

    fn default_config() -> Arc<crate::config::model_config::ModelConfigManager> {
        use std::collections::HashMap;
        use crate::models::config::*;

        let mut models = HashMap::new();
        let settings = ModelSettings {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            temperature: 0.3,
        };
        models.insert(ModelPurpose::Chat, settings.clone());
        models.insert(ModelPurpose::Memory, settings.clone());
        models.insert(ModelPurpose::Thought, settings.clone());
        models.insert(ModelPurpose::CharacterGeneration, settings);

        let config = AppConfig {
            models,
            spontaneous: SpontaneousConfig { enabled: false, min_interval_seconds: 60, probability: 0.3 },
            thought: ThoughtConfig { enabled: false, interval_minutes: 5 },
            memory: MemoryConfig { compression_threshold: 50 },
            tts: TTSGlobalConfig { enabled: false },
            ui: UIConfig { theme: Theme::Dark, language: "ja".to_string() },
            plugins: PluginsConfig { enabled_plugins: vec![], plugin_settings: HashMap::new() },
            attachment: AttachmentConfig { max_file_size_bytes: 10 * 1024 * 1024, allowed_extensions: vec![] },
        };

        Arc::new(crate::config::model_config::ModelConfigManager::new_with_config(config))
    }

    #[tokio::test]
    async fn test_check_and_compress_below_threshold() {
        let db = setup_db();
        create_session(&db, "sess-001");
        insert_messages(&db, "sess-001", 10);

        let db_arc = Arc::new(Mutex::new(db));
        let mock_llm = Arc::new(MockLLMClient::new("Summary"));
        let llm_lock = Arc::new(tokio::sync::Mutex::new(()));

        let manager = DefaultMemoryManager::new(db_arc.clone(), mock_llm, default_config(), 50, llm_lock);

        // 閾値未満なので圧縮されない
        manager.check_and_compress("sess-001").await.unwrap();

        let db_lock = db_arc.lock().unwrap();
        let memories = memory_repo::list_memories(db_lock.connection(), "char-001").unwrap();
        assert!(memories.is_empty());
    }

    #[tokio::test]
    async fn test_check_and_compress_at_threshold() {
        let db = setup_db();
        create_session(&db, "sess-001");
        insert_messages(&db, "sess-001", 50);

        let db_arc = Arc::new(Mutex::new(db));
        let mock_llm = Arc::new(MockLLMClient::new(
            "- ユーザーは猫が好き\n- プログラミングの話題が多い",
        ));
        let llm_lock = Arc::new(tokio::sync::Mutex::new(()));

        let manager = DefaultMemoryManager::new(db_arc.clone(), mock_llm, default_config(), 50, llm_lock);

        manager.check_and_compress("sess-001").await.unwrap();

        let db_lock = db_arc.lock().unwrap();
        let memories = memory_repo::list_memories(db_lock.connection(), "char-001").unwrap();
        assert_eq!(memories.len(), 1);
        assert!(memories[0].content.contains("ユーザーは猫が好き"));
        assert_eq!(
            memories[0].source_session_id,
            Some("sess-001".to_string())
        );
        assert_eq!(
            memories[0].source_message_from,
            Some("msg-000".to_string())
        );
        assert_eq!(
            memories[0].source_message_to,
            Some("msg-049".to_string())
        );
    }

    #[tokio::test]
    async fn test_check_and_compress_session_not_found() {
        let db = setup_db();
        let db_arc = Arc::new(Mutex::new(db));
        let mock_llm = Arc::new(MockLLMClient::new("Summary"));
        let llm_lock = Arc::new(tokio::sync::Mutex::new(()));

        let manager = DefaultMemoryManager::new(db_arc, mock_llm, default_config(), 50, llm_lock);

        let result = manager.check_and_compress("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_memories() {
        let db = setup_db();
        let conn = db.connection();
        let memory = Memory {
            id: "mem-001".to_string(),
            character_id: "char-001".to_string(),
            content: "テスト記憶".to_string(),
            source_session_id: None,
            source_message_from: None,
            source_message_to: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        memory_repo::insert_memory(conn, &memory).unwrap();

        let db_arc = Arc::new(Mutex::new(db));
        let mock_llm = Arc::new(MockLLMClient::new(""));
        let llm_lock = Arc::new(tokio::sync::Mutex::new(()));

        let manager = DefaultMemoryManager::new(db_arc, mock_llm, default_config(), 50, llm_lock);

        let memories = manager.list_memories("char-001").await.unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].content, "テスト記憶");
    }

    #[tokio::test]
    async fn test_get_relevant_memories() {
        let db = setup_db();
        let conn = db.connection();
        let memory = Memory {
            id: "mem-001".to_string(),
            character_id: "char-001".to_string(),
            content: "ユーザーは猫が好き".to_string(),
            source_session_id: None,
            source_message_from: None,
            source_message_to: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        memory_repo::insert_memory(conn, &memory).unwrap();

        let db_arc = Arc::new(Mutex::new(db));
        let mock_llm = Arc::new(MockLLMClient::new(""));
        let llm_lock = Arc::new(tokio::sync::Mutex::new(()));

        let manager = DefaultMemoryManager::new(db_arc, mock_llm, default_config(), 50, llm_lock);

        let memories = manager
            .get_relevant_memories("char-001", "猫について話そう")
            .await
            .unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].content, "ユーザーは猫が好き");
    }

    #[tokio::test]
    async fn test_update_memory() {
        let db = setup_db();
        let conn = db.connection();
        let memory = Memory {
            id: "mem-001".to_string(),
            character_id: "char-001".to_string(),
            content: "元の内容".to_string(),
            source_session_id: None,
            source_message_from: None,
            source_message_to: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        memory_repo::insert_memory(conn, &memory).unwrap();

        let db_arc = Arc::new(Mutex::new(db));
        let mock_llm = Arc::new(MockLLMClient::new(""));
        let llm_lock = Arc::new(tokio::sync::Mutex::new(()));

        let manager = DefaultMemoryManager::new(db_arc.clone(), mock_llm, default_config(), 50, llm_lock);

        manager.update_memory("mem-001", "更新後の内容").await.unwrap();

        let db_lock = db_arc.lock().unwrap();
        let memories = memory_repo::list_memories(db_lock.connection(), "char-001").unwrap();
        assert_eq!(memories[0].content, "更新後の内容");
        assert_ne!(memories[0].updated_at, "2024-01-01T00:00:00Z");
    }

    #[tokio::test]
    async fn test_delete_memory() {
        let db = setup_db();
        let conn = db.connection();
        let memory = Memory {
            id: "mem-001".to_string(),
            character_id: "char-001".to_string(),
            content: "削除対象".to_string(),
            source_session_id: None,
            source_message_from: None,
            source_message_to: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        memory_repo::insert_memory(conn, &memory).unwrap();

        let db_arc = Arc::new(Mutex::new(db));
        let mock_llm = Arc::new(MockLLMClient::new(""));
        let llm_lock = Arc::new(tokio::sync::Mutex::new(()));

        let manager = DefaultMemoryManager::new(db_arc.clone(), mock_llm, default_config(), 50, llm_lock);

        manager.delete_memory("mem-001").await.unwrap();

        let db_lock = db_arc.lock().unwrap();
        let memories = memory_repo::list_memories(db_lock.connection(), "char-001").unwrap();
        assert!(memories.is_empty());
    }

    #[test]
    fn test_compression_system_prompt_contains_key_points() {
        let prompt = DefaultMemoryManager::compression_system_prompt();
        assert!(prompt.contains("重要な事実"));
        assert!(prompt.contains("トピック"));
        assert!(prompt.contains("好み"));
        assert!(prompt.contains("約束"));
    }

    #[test]
    fn test_format_messages_for_compression() {
        let messages = vec![
            ChatMessageRecord {
                id: "msg-001".to_string(),
                session_id: "sess-001".to_string(),
                role: ChatRole::User,
                content: "こんにちは".to_string(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
            },
            ChatMessageRecord {
                id: "msg-002".to_string(),
                session_id: "sess-001".to_string(),
                role: ChatRole::Assistant,
                content: "はい、こんにちは！".to_string(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: "2024-01-01T00:01:00Z".to_string(),
            },
        ];

        let formatted = DefaultMemoryManager::format_messages_for_compression(&messages);
        assert!(formatted.contains("ユーザー: こんにちは"));
        assert!(formatted.contains("アシスタント: はい、こんにちは！"));
    }
}
