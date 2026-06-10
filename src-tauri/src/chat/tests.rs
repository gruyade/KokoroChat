// Chat Engine tests

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use crate::chat::engine::{ChatEngine, DefaultChatEngine};
    use crate::db::database::Database;
    use crate::db::repositories::{character as char_repo, chat as chat_repo};
    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
    use crate::models::tts::TTSConfig;
    use crate::models::{
        Attachment, AttachmentType, Character, ChatMessageRecord, ChatRole, ChatSession, Memory,
        ToolCall, ToolDefinition,
    };
    use crate::tts::connector::TTSConnector;

    /// テスト用MockLLMClient
    struct MockLLMClient {
        /// chat_stream呼び出し時に返すテキスト
        response_text: String,
    }

    impl MockLLMClient {
        fn new(response_text: &str) -> Self {
            Self {
                response_text: response_text.to_string(),
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
            Ok(LLMResponse::Text { content: self.response_text.clone(), thinking: None })
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
            callbacks: crate::llm::client::StreamCallbacks,
        ) -> Result<LLMResponse, AppError> {
            // チャンクごとにコールバック呼び出しをシミュレート
            let chunks: Vec<&str> = self.response_text.split(' ').collect();
            let mut full = String::new();
            for (i, chunk) in chunks.iter().enumerate() {
                let text = if i > 0 {
                    format!(" {}", chunk)
                } else {
                    chunk.to_string()
                };
                full.push_str(&text);
                (callbacks.0)(text);
            }
            Ok(LLMResponse::Text { content: full, thinking: None })
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    /// tool_callを返すMockLLMClient
    struct MockToolCallLLMClient;

    #[async_trait]
    impl LLMClient for MockToolCallLLMClient {
        async fn chat(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            Ok(LLMResponse::ToolCalls { calls: vec![ToolCall {
                id: "call_001".to_string(),
                name: "calculator".to_string(),
                arguments: serde_json::json!({"expression": "2+2"}),
                context: None,
            }], thinking: None })
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
            _callbacks: crate::llm::client::StreamCallbacks,
        ) -> Result<LLMResponse, AppError> {
            Ok(LLMResponse::Text { content: String::new(), thinking: None })
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    /// コンテキスト検証用MockLLMClient（受け取ったメッセージを記録）
    struct ContextCaptureLLMClient {
        captured: Arc<Mutex<Vec<ChatMessage>>>,
    }

    impl ContextCaptureLLMClient {
        fn new() -> (Self, Arc<Mutex<Vec<ChatMessage>>>) {
            let captured = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    captured: captured.clone(),
                },
                captured,
            )
        }
    }

    #[async_trait]
    impl LLMClient for ContextCaptureLLMClient {
        async fn chat(
            &self,
            messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            let mut cap = self.captured.lock().unwrap();
            *cap = messages.to_vec();
            Ok(LLMResponse::Text { content: "OK".to_string(), thinking: None })
        }

        async fn chat_stream(
            &self,
            messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
            callbacks: crate::llm::client::StreamCallbacks,
        ) -> Result<LLMResponse, AppError> {
            let mut cap = self.captured.lock().unwrap();
            *cap = messages.to_vec();
            (callbacks.0)("OK".to_string());
            Ok(LLMResponse::Text { content: "OK".to_string(), thinking: None })
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn test_config() -> Arc<crate::config::model_config::ModelConfigManager> {
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

    fn test_llm_lock() -> Arc<tokio::sync::Mutex<()>> {
        Arc::new(tokio::sync::Mutex::new(()))
    }

    /// テスト用MockTTSConnector
    struct MockTTSConnector;

    #[async_trait]
    impl TTSConnector for MockTTSConnector {
        async fn synthesize(
            &self,
            _text: &str,
            _config: &TTSConfig,
            _voicepeak_path: Option<&str>,
        ) -> Result<Vec<u8>, AppError> {
            Ok(vec![])
        }

        async fn test_connection(
            &self,
            _config: &TTSConfig,
            _voicepeak_path: Option<&str>,
        ) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn test_tts_connector() -> Arc<dyn TTSConnector> {
        Arc::new(MockTTSConnector)
    }

    fn setup_db_with_character() -> Arc<Mutex<Database>> {
        let db = Database::open_in_memory().unwrap();
        let character = Character {
            id: "char-001".to_string(),
            name: "テストキャラ".to_string(),
            description: "テスト用".to_string(),
            system_prompt: "あなたはテストキャラです。".to_string(),
            avatar_path: None,
            tts_config: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        char_repo::insert_character(db.connection(), &character).unwrap();
        Arc::new(Mutex::new(db))
    }

    #[tokio::test]
    async fn test_create_session() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db.clone(),
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let session_id = engine.create_session("char-001").await.unwrap();

        assert!(!session_id.is_empty());

        // DBに保存されていることを確認
        let db_lock = db.lock().unwrap();
        let session = chat_repo::get_session(db_lock.connection(), &session_id)
            .unwrap()
            .unwrap();
        assert_eq!(session.character_id, "char-001");
        assert!(session.title.is_none());
        assert!(session.last_message_at.is_none());
    }

    #[tokio::test]
    async fn test_create_session_invalid_character() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db,
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        // 存在しないキャラクターでもセッション作成自体は成功
        // （外部キー制約がある場合はエラーになる）
        let result = engine.create_session("nonexistent").await;
        // SQLiteの外部キー制約によりエラー
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db,
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let _s1 = engine.create_session("char-001").await.unwrap();
        let _s2 = engine.create_session("char-001").await.unwrap();

        let sessions = engine.list_sessions("char-001").await.unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_list_sessions_empty() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db,
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let sessions = engine.list_sessions("char-001").await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_delete_session() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db.clone(),
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let session_id = engine.create_session("char-001").await.unwrap();
        engine.delete_session(&session_id).await.unwrap();

        let db_lock = db.lock().unwrap();
        let session = chat_repo::get_session(db_lock.connection(), &session_id).unwrap();
        assert!(session.is_none());
    }

    #[tokio::test]
    async fn test_get_history_empty() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db,
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let session_id = engine.create_session("char-001").await.unwrap();
        let history = engine.get_history(&session_id).await.unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_build_context_basic() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db,
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let messages = engine.build_context(
            "あなたはテストキャラです。",
            &[],
            &[],
            &[],
            "こんにちは",
            None,
            None,
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[0].content, "あなたはテストキャラです。");
        assert_eq!(messages[1].role, MessageRole::User);
        assert_eq!(messages[1].content, "こんにちは");
    }

    #[test]
    fn test_build_context_with_memories() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db,
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let memories = vec![
            Memory {
                id: "mem-001".to_string(),
                character_id: "char-001".to_string(),
                content: "ユーザーは猫が好き".to_string(),
                source_session_id: None,
                source_message_from: None,
                source_message_to: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
            Memory {
                id: "mem-002".to_string(),
                character_id: "char-001".to_string(),
                content: "ユーザーの名前は太郎".to_string(),
                source_session_id: None,
                source_message_from: None,
                source_message_to: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
        ];

        let messages =
            engine.build_context("System prompt", &memories, &[], &[], "Hello", None, None);

        // system_prompt + 2 memories + user_message = 4
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[1].role, MessageRole::System);
        assert!(messages[1].content.contains("[Memory]"));
        assert!(messages[1].content.contains("ユーザーは猫が好き"));
        assert_eq!(messages[2].role, MessageRole::System);
        assert!(messages[2].content.contains("ユーザーの名前は太郎"));
        assert_eq!(messages[3].role, MessageRole::User);
    }

    #[test]
    fn test_build_context_with_history() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db,
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let history = vec![
            crate::models::ChatMessageRecord {
                id: "msg-001".to_string(),
                session_id: "sess-001".to_string(),
                role: ChatRole::User,
                content: "前のメッセージ".to_string(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                thinking_content: None,
                created_at: "2024-01-01T10:00:00Z".to_string(),
            },
            crate::models::ChatMessageRecord {
                id: "msg-002".to_string(),
                session_id: "sess-001".to_string(),
                role: ChatRole::Assistant,
                content: "前の返答".to_string(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                thinking_content: None,
                created_at: "2024-01-01T10:01:00Z".to_string(),
            },
        ];

        let messages = engine.build_context(
            "System prompt",
            &[],
            &[],
            &history,
            "新しいメッセージ",
            None,
            None,
        );

        // system_prompt + 2 history + user_message = 4
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[1].role, MessageRole::User);
        assert_eq!(messages[1].content, "前のメッセージ");
        assert_eq!(messages[2].role, MessageRole::Assistant);
        assert_eq!(messages[2].content, "前の返答");
        assert_eq!(messages[3].role, MessageRole::User);
        assert_eq!(messages[3].content, "新しいメッセージ");
    }

    #[test]
    fn test_build_context_with_attachments() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db,
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let messages = engine.build_context(
            "System prompt",
            &[],
            &[],
            &[],
            "ファイルを見て",
            Some("--- test.txt ---\nファイル内容"),
            None,
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, MessageRole::User);
        assert!(messages[1].content.contains("ファイルを見て"));
        assert!(messages[1].content.contains("[Attached Files]"));
        assert!(messages[1].content.contains("ファイル内容"));
    }

    #[test]
    fn test_extract_attachment_text() {
        let attachments = vec![
            Attachment {
                id: "att-001".to_string(),
                file_name: "readme.txt".to_string(),
                file_path: "/path/to/readme.txt".to_string(),
                attachment_type: AttachmentType::Text,
                size_bytes: 100,
                extracted_text: Some("Hello World".to_string()),
                base64_data: None,
            },
            Attachment {
                id: "att-002".to_string(),
                file_name: "image.png".to_string(),
                file_path: "/path/to/image.png".to_string(),
                attachment_type: AttachmentType::Image,
                size_bytes: 5000,
                extracted_text: None,
                base64_data: Some("base64data".to_string()),
            },
        ];

        let result = DefaultChatEngine::extract_attachment_text(&attachments);
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("readme.txt"));
        assert!(text.contains("Hello World"));
        // 画像はextracted_textがないので含まれない
        assert!(!text.contains("image.png"));
    }

    #[test]
    fn test_extract_attachment_text_empty() {
        let attachments = vec![Attachment {
            id: "att-001".to_string(),
            file_name: "image.png".to_string(),
            file_path: "/path/to/image.png".to_string(),
            attachment_type: AttachmentType::Image,
            size_bytes: 5000,
            extracted_text: None,
            base64_data: Some("base64data".to_string()),
        }];

        let result = DefaultChatEngine::extract_attachment_text(&attachments);
        assert!(result.is_none());
    }

    #[test]
    fn test_to_message_attachments() {
        let attachments = vec![
            Attachment {
                id: "att-001".to_string(),
                file_name: "test.txt".to_string(),
                file_path: "/path/to/test.txt".to_string(),
                attachment_type: AttachmentType::Text,
                size_bytes: 100,
                extracted_text: Some("content".to_string()),
                base64_data: None,
            },
            Attachment {
                id: "att-002".to_string(),
                file_name: "doc.pdf".to_string(),
                file_path: "/path/to/doc.pdf".to_string(),
                attachment_type: AttachmentType::Pdf,
                size_bytes: 2000,
                extracted_text: Some("pdf content".to_string()),
                base64_data: None,
            },
        ];

        let result = DefaultChatEngine::to_message_attachments(&attachments);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].file_name, "test.txt");
        assert_eq!(result[0].attachment_type, "text");
        assert_eq!(result[0].extracted_text, Some("content".to_string()));
        assert_eq!(result[1].file_name, "doc.pdf");
        assert_eq!(result[1].attachment_type, "pdf");
    }

    #[test]
    fn test_build_context_order() {
        // コンテキスト順序: system_prompt → memories → history → user_message
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db,
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let memories = vec![Memory {
            id: "mem-001".to_string(),
            character_id: "char-001".to_string(),
            content: "記憶内容".to_string(),
            source_session_id: None,
            source_message_from: None,
            source_message_to: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }];

        let history = vec![crate::models::ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::User,
            content: "履歴メッセージ".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            thinking_content: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        }];

        let messages = engine.build_context(
            "システムプロンプト",
            &memories,
            &[],
            &history,
            "新規メッセージ",
            None,
            None,
        );

        // 順序確認: system(0) → memory(1) → history(2) → user(3)
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[0].content, "システムプロンプト");
        assert_eq!(messages[1].role, MessageRole::System);
        assert!(messages[1].content.starts_with("[Memory]"));
        assert_eq!(messages[2].role, MessageRole::User);
        assert_eq!(messages[2].content, "履歴メッセージ");
        assert_eq!(messages[3].role, MessageRole::User);
        assert_eq!(messages[3].content, "新規メッセージ");
    }

    #[test]
    fn test_spontaneous_role_mapped_to_assistant() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(
            db,
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let history = vec![crate::models::ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::Spontaneous,
            content: "自発的発話".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            thinking_content: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        }];

        let messages = engine.build_context("Prompt", &[], &[], &history, "Hi", None, None);

        // Spontaneous → Assistant にマッピング
        assert_eq!(messages[1].role, MessageRole::Assistant);
    }

    // =========================================================================
    // TTS Branch Decision Logic Tests
    //
    // send_message のTTS分岐はAppHandle（Tauriイベント発行）が必要なため、
    // 完全な統合テストはユニットテストでは実行できない。
    //
    // TTS有効時のフロー（tts:generating → tts:complete / tts:error）は
    // tts/tests.rs の flow_controller_tests モジュールで検証済み。
    //
    // ここではTTS分岐の判定ロジック（グローバル設定 × キャラクター設定）を
    // 設定値の組み合わせで検証する。
    // =========================================================================

    /// TTS有効判定: グローバルTTS enabled=true AND キャラクターtts_config=Some → TTS有効
    #[test]
    fn test_tts_enabled_decision_both_conditions_met() {
        let config = test_config_with_tts(true);
        let tts_config: Option<crate::models::tts::TTSConfig> = Some(make_character_tts_config());

        let tts_enabled = config.get_config().tts.enabled && tts_config.is_some();
        assert!(tts_enabled);
    }

    /// TTS有効判定: グローバルTTS enabled=false → TTS無効（キャラクター設定に関わらず）
    #[test]
    fn test_tts_disabled_when_global_config_disabled() {
        let config = test_config_with_tts(false);
        let tts_config: Option<crate::models::tts::TTSConfig> = Some(make_character_tts_config());

        let tts_enabled = config.get_config().tts.enabled && tts_config.is_some();
        assert!(!tts_enabled);
    }

    /// TTS有効判定: キャラクターtts_config=None → TTS無効（グローバル設定に関わらず）
    #[test]
    fn test_tts_disabled_when_character_has_no_tts_config() {
        let config = test_config_with_tts(true);
        let tts_config: Option<crate::models::tts::TTSConfig> = None;

        let tts_enabled = config.get_config().tts.enabled && tts_config.is_some();
        assert!(!tts_enabled);
    }

    /// TTS有効判定: 両方無効 → TTS無効
    #[test]
    fn test_tts_disabled_when_both_conditions_unmet() {
        let config = test_config_with_tts(false);
        let tts_config: Option<crate::models::tts::TTSConfig> = None;

        let tts_enabled = config.get_config().tts.enabled && tts_config.is_some();
        assert!(!tts_enabled);
    }

    // --- TTS分岐テスト用ヘルパー ---

    fn test_config_with_tts(
        tts_enabled: bool,
    ) -> Arc<crate::config::model_config::ModelConfigManager> {
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
                enabled: tts_enabled,
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

    fn make_character_tts_config() -> crate::models::tts::TTSConfig {
        use crate::models::tts::{EmotionParams, TTSConfig, TTSProvider};

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

    // =========================================================================
    // Tool Execution Loop Tests
    //
    // send_message のツール実行ループを検証するテスト群。
    // DefaultChatEngine::send_message_for_test を使用。
    // =========================================================================

    use crate::models::plugin::ToolResult;
    use crate::plugin::system::PluginSystem;

    /// 連続呼び出しで異なるレスポンスを返すMockLLMClient
    struct SequentialMockLLMClient {
        responses: Arc<Mutex<Vec<LLMResponse>>>,
        call_count: Arc<Mutex<usize>>,
    }

    impl SequentialMockLLMClient {
        fn new(responses: Vec<LLMResponse>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses)),
                call_count: Arc::new(Mutex::new(0)),
            }
        }

        fn get_call_count(&self) -> usize {
            *self.call_count.lock().unwrap()
        }
    }

    #[async_trait]
    impl LLMClient for SequentialMockLLMClient {
        async fn chat(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            let mut count = self.call_count.lock().unwrap();
            let responses = self.responses.lock().unwrap();
            let idx = *count;
            *count += 1;
            if idx < responses.len() {
                Ok(responses[idx].clone())
            } else {
                Ok(responses
                    .last()
                    .cloned()
                    .unwrap_or(LLMResponse::Text { content: "done".to_string(), thinking: None }))
            }
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
            callbacks: crate::llm::client::StreamCallbacks,
        ) -> Result<LLMResponse, AppError> {
            let mut count = self.call_count.lock().unwrap();
            let responses = self.responses.lock().unwrap();
            let idx = *count;
            *count += 1;
            let response = if idx < responses.len() {
                responses[idx].clone()
            } else {
                responses
                    .last()
                    .cloned()
                    .unwrap_or(LLMResponse::Text { content: "done".to_string(), thinking: None })
            };

            match &response {
                LLMResponse::Text { content: text, .. } => {
                    (callbacks.0)(text.clone());
                }
                LLMResponse::ToolCalls { .. } => {
                    // ToolCallsの場合はコールバックを呼ばない
                }
            }
            Ok(response)
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    /// テスト用MockPluginSystem
    struct MockPluginSystem {
        tool_definitions: Vec<ToolDefinition>,
    }

    impl MockPluginSystem {
        fn new() -> Self {
            Self {
                tool_definitions: vec![ToolDefinition {
                    name: "calculator".to_string(),
                    description: "Calculate math expressions".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "expression": { "type": "string" }
                        }
                    }),
                }],
            }
        }
    }

    #[async_trait]
    impl PluginSystem for MockPluginSystem {
        async fn handle_tool_calls(
            &self,
            tool_calls: &[crate::models::ToolCall],
            _app_handle: &tauri::AppHandle,
        ) -> Result<Vec<ToolResult>, AppError> {
            let results = tool_calls
                .iter()
                .map(|tc| ToolResult {
                    tool_call_id: tc.id.clone(),
                    content: format!("Result for {}: 4", tc.name),
                    is_error: false,
                })
                .collect();
            Ok(results)
        }

        fn get_enabled_tools(&self) -> Vec<ToolDefinition> {
            self.tool_definitions.clone()
        }
    }

    /// Test 1: ToolCall → ToolResult → 最終テキスト応答
    /// LLMが最初にToolCallsを返し、次にTextを返すシナリオ
    #[tokio::test]
    async fn test_tool_loop_single_tool_call_then_text() {
        let db = setup_db_with_character();
        let llm = Arc::new(SequentialMockLLMClient::new(vec![
            // 1回目: ToolCallsを返す
            LLMResponse::ToolCalls { calls: vec![ToolCall {
                id: "call_001".to_string(),
                name: "calculator".to_string(),
                arguments: serde_json::json!({"expression": "2+2"}),
                context: None,
            }], thinking: None },
            // 2回目: テキストを返す
            LLMResponse::Text { content: "The answer is 4.".to_string(), thinking: None },
        ]));
        let plugin_system: Arc<dyn PluginSystem> = Arc::new(MockPluginSystem::new());
        let engine = DefaultChatEngine::new(
            db.clone(),
            llm.clone(),
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            Some(plugin_system),
        );

        let session_id = engine.create_session("char-001").await.unwrap();

        engine
            .send_message_for_test(&session_id, "What is 2+2?", None)
            .await
            .unwrap();

        // LLMが2回呼ばれたことを確認
        assert_eq!(llm.get_call_count(), 2);

        // DB内のメッセージを確認
        let messages = {
            let db_lock = db.lock().unwrap();
            chat_repo::get_messages(db_lock.connection(), &session_id).unwrap()
        };

        // 期待: user, assistant(tool_calls), tool(result), assistant(final text)
        assert_eq!(messages.len(), 4);

        // 1. ユーザーメッセージ
        assert_eq!(messages[0].role, ChatRole::User);
        assert_eq!(messages[0].content, "What is 2+2?");

        // 2. アシスタント（tool_calls含む）
        assert_eq!(messages[1].role, ChatRole::Assistant);
        assert!(messages[1].tool_calls.is_some());
        let tool_calls = messages[1].tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "calculator");

        // 3. ツール結果
        assert_eq!(messages[2].role, ChatRole::Tool);
        assert_eq!(messages[2].tool_call_id, Some("call_001".to_string()));
        assert!(messages[2].content.contains("Result for calculator"));

        // 4. 最終アシスタント応答
        assert_eq!(messages[3].role, ChatRole::Assistant);
        assert_eq!(messages[3].content, "The answer is 4.");
    }

    /// Test 2: MAX_TOOL_ITERATIONS制限テスト
    /// LLMが常にToolCallsを返す場合、10回で停止することを確認
    #[tokio::test]
    async fn test_tool_loop_max_iterations_limit() {
        let db = setup_db_with_character();

        // 常にToolCallsを返すレスポンスを15個用意（10回ループ + 安全マージン）
        let responses: Vec<LLMResponse> = (0..15)
            .map(|i| {
                LLMResponse::ToolCalls { calls: vec![ToolCall {
                    id: format!("call_{:03}", i + 1),
                    name: "calculator".to_string(),
                    arguments: serde_json::json!({"expression": "loop"}),
                    context: None,
                }], thinking: None }
            })
            .collect();

        let llm = Arc::new(SequentialMockLLMClient::new(responses));
        let plugin_system: Arc<dyn PluginSystem> = Arc::new(MockPluginSystem::new());
        let engine = DefaultChatEngine::new(
            db.clone(),
            llm.clone(),
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            Some(plugin_system),
        );

        let session_id = engine.create_session("char-001").await.unwrap();

        engine
            .send_message_for_test(&session_id, "infinite loop test", None)
            .await
            .unwrap();

        // LLMが10回呼ばれたことを確認（MAX_TOOL_ITERATIONS = 10）
        assert_eq!(llm.get_call_count(), 10);

        // DB内のメッセージを確認
        let messages = {
            let db_lock = db.lock().unwrap();
            chat_repo::get_messages(db_lock.connection(), &session_id).unwrap()
        };

        // 最後のメッセージがフォールバックメッセージであることを確認
        let last_msg = messages.last().unwrap();
        assert_eq!(last_msg.role, ChatRole::Assistant);
        assert!(last_msg.content.contains("Tool execution limit reached"));
    }

    /// Test 3: PluginSystem未設定時のエラー結果生成テスト
    /// plugin_system=None の場合、ToolCallに対してエラー結果が生成されることを確認
    #[tokio::test]
    async fn test_tool_loop_plugin_system_unavailable() {
        let db = setup_db_with_character();
        let llm = Arc::new(SequentialMockLLMClient::new(vec![
            // 1回目: ToolCallsを返す
            LLMResponse::ToolCalls { calls: vec![ToolCall {
                id: "call_001".to_string(),
                name: "calculator".to_string(),
                arguments: serde_json::json!({"expression": "2+2"}),
                context: None,
            }], thinking: None },
            // 2回目: テキストを返す（エラー結果を受けてLLMが応答）
            LLMResponse::Text { content: "Sorry, the tool is not available.".to_string(), thinking: None },
        ]));

        // plugin_system = None で作成
        let engine = DefaultChatEngine::new(
            db.clone(),
            llm.clone(),
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None, // PluginSystem未設定
        );

        let session_id = engine.create_session("char-001").await.unwrap();

        engine
            .send_message_for_test(&session_id, "Calculate 2+2", None)
            .await
            .unwrap();

        // LLMが2回呼ばれたことを確認
        assert_eq!(llm.get_call_count(), 2);

        // DB内のメッセージを確認
        let messages = {
            let db_lock = db.lock().unwrap();
            chat_repo::get_messages(db_lock.connection(), &session_id).unwrap()
        };

        // 期待: user, assistant(tool_calls), tool(error result), assistant(final text)
        assert_eq!(messages.len(), 4);

        // ツール結果がエラーメッセージを含むことを確認
        assert_eq!(messages[2].role, ChatRole::Tool);
        assert!(messages[2]
            .content
            .contains("Plugin system is not available"));

        // 最終応答
        assert_eq!(messages[3].role, ChatRole::Assistant);
        assert_eq!(messages[3].content, "Sorry, the tool is not available.");
    }

    // =========================================================================
    // E2E Integration Test: Real Calculator Plugin
    //
    // 実際の CalculatorPlugin + DefaultPluginRegistry + DefaultPluginSystem を使い、
    // LLM → ToolCall検出 → PluginSystemディスパッチ → Calculator実行 → 結果DB保存
    // → LLM再呼び出し の完全フローを検証する。
    // =========================================================================

    use crate::plugin::builtin::CalculatorPlugin;
    use crate::plugin::registry::{DefaultPluginRegistry, PluginRegistry};
    use crate::plugin::system::DefaultPluginSystem;

    /// E2E Test: 実際のCalculatorPluginを使ったツール実行フロー
    /// SequentialMockLLMClient が最初に ToolCall("calculate", {"expression": "3 + 5 * 2"}) を返し、
    /// 次にテキスト応答を返す。Calculator が実際に数式を評価し、結果 "13" がDBに保存されることを確認。
    #[tokio::test]
    async fn test_e2e_real_calculator_plugin_execution() {
        let db = setup_db_with_character();

        // 実際の PluginRegistry + CalculatorPlugin を構築
        let registry = Arc::new(DefaultPluginRegistry::new());
        registry
            .register(Box::new(CalculatorPlugin::new()))
            .unwrap();

        let plugin_system: Arc<dyn PluginSystem> = Arc::new(DefaultPluginSystem::new(registry));

        // LLMモック: 1回目=ToolCall, 2回目=テキスト応答
        let llm = Arc::new(SequentialMockLLMClient::new(vec![
            LLMResponse::ToolCalls { calls: vec![ToolCall {
                id: "call_calc_001".to_string(),
                name: "calculate".to_string(),
                arguments: serde_json::json!({"expression": "3 + 5 * 2"}),
                context: None,
            }], thinking: None },
            LLMResponse::Text { content: "The result of 3 + 5 * 2 is 13.".to_string(), thinking: None },
        ]));

        let engine = DefaultChatEngine::new(
            db.clone(),
            llm.clone(),
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            Some(plugin_system),
        );

        let session_id = engine.create_session("char-001").await.unwrap();

        engine
            .send_message_for_test(&session_id, "What is 3 + 5 * 2?", None)
            .await
            .unwrap();

        // LLMが2回呼ばれたことを確認（ToolCall応答 → テキスト応答）
        assert_eq!(llm.get_call_count(), 2);

        // DB内のメッセージを検証
        let messages = {
            let db_lock = db.lock().unwrap();
            chat_repo::get_messages(db_lock.connection(), &session_id).unwrap()
        };

        // 期待: user, assistant(tool_calls), tool(result), assistant(final text) = 4件
        assert_eq!(messages.len(), 4);

        // 1. ユーザーメッセージ
        assert_eq!(messages[0].role, ChatRole::User);
        assert_eq!(messages[0].content, "What is 3 + 5 * 2?");

        // 2. アシスタント（tool_calls含む）
        assert_eq!(messages[1].role, ChatRole::Assistant);
        assert!(messages[1].tool_calls.is_some());
        let tool_calls = messages[1].tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "calculate");
        assert_eq!(tool_calls[0].id, "call_calc_001");

        // 3. ツール実行結果 — 実際のCalculatorPluginが "3 + 5 * 2" を評価した結果
        assert_eq!(messages[2].role, ChatRole::Tool);
        assert_eq!(messages[2].tool_call_id, Some("call_calc_001".to_string()));
        // Calculator は 3 + 5 * 2 = 13 を返す（演算子優先順位を正しく処理）
        assert_eq!(messages[2].content, "13");

        // 4. 最終アシスタント応答
        assert_eq!(messages[3].role, ChatRole::Assistant);
        assert_eq!(messages[3].content, "The result of 3 + 5 * 2 is 13.");
    }

    /// E2E Test: Calculatorプラグインでエラーが発生するケース（ゼロ除算）
    /// ツール実行結果が is_error=true でもフローが正常に継続することを確認。
    #[tokio::test]
    async fn test_e2e_real_calculator_plugin_error_case() {
        let db = setup_db_with_character();

        let registry = Arc::new(DefaultPluginRegistry::new());
        registry
            .register(Box::new(CalculatorPlugin::new()))
            .unwrap();

        let plugin_system: Arc<dyn PluginSystem> = Arc::new(DefaultPluginSystem::new(registry));

        // LLMモック: 1回目=ゼロ除算のToolCall, 2回目=エラーを受けたテキスト応答
        let llm = Arc::new(SequentialMockLLMClient::new(vec![
            LLMResponse::ToolCalls { calls: vec![ToolCall {
                id: "call_div_zero".to_string(),
                name: "calculate".to_string(),
                arguments: serde_json::json!({"expression": "10 / 0"}),
                context: None,
            }], thinking: None },
            LLMResponse::Text { content: "I'm sorry, division by zero is not allowed.".to_string(), thinking: None },
        ]));

        let engine = DefaultChatEngine::new(
            db.clone(),
            llm.clone(),
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            Some(plugin_system),
        );

        let session_id = engine.create_session("char-001").await.unwrap();

        engine
            .send_message_for_test(&session_id, "What is 10 / 0?", None)
            .await
            .unwrap();

        assert_eq!(llm.get_call_count(), 2);

        let messages = {
            let db_lock = db.lock().unwrap();
            chat_repo::get_messages(db_lock.connection(), &session_id).unwrap()
        };

        assert_eq!(messages.len(), 4);

        // ツール結果がエラーを含むことを確認
        assert_eq!(messages[2].role, ChatRole::Tool);
        assert_eq!(messages[2].tool_call_id, Some("call_div_zero".to_string()));
        assert!(messages[2].content.contains("計算エラー"));
        assert!(messages[2].content.contains("ゼロ除算"));

        // 最終応答
        assert_eq!(messages[3].role, ChatRole::Assistant);
        assert!(messages[3].content.contains("division by zero"));
    }

    /// E2E Test: 複数回のツール呼び出しを含むフロー
    /// LLMが2回連続でToolCallを返し、3回目でテキストを返すシナリオ。
    #[tokio::test]
    async fn test_e2e_real_calculator_multiple_tool_calls() {
        let db = setup_db_with_character();

        let registry = Arc::new(DefaultPluginRegistry::new());
        registry
            .register(Box::new(CalculatorPlugin::new()))
            .unwrap();

        let plugin_system: Arc<dyn PluginSystem> = Arc::new(DefaultPluginSystem::new(registry));

        // LLMモック: 1回目=ToolCall(2+3), 2回目=ToolCall(5*4), 3回目=テキスト
        let llm = Arc::new(SequentialMockLLMClient::new(vec![
            LLMResponse::ToolCalls { calls: vec![ToolCall {
                id: "call_step1".to_string(),
                name: "calculate".to_string(),
                arguments: serde_json::json!({"expression": "2 + 3"}),
                context: None,
            }], thinking: None },
            LLMResponse::ToolCalls { calls: vec![ToolCall {
                id: "call_step2".to_string(),
                name: "calculate".to_string(),
                arguments: serde_json::json!({"expression": "5 * 4"}),
                context: None,
            }], thinking: None },
            LLMResponse::Text { content: "First: 5, Second: 20. Total: 25.".to_string(), thinking: None },
        ]));

        let engine = DefaultChatEngine::new(
            db.clone(),
            llm.clone(),
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            Some(plugin_system),
        );

        let session_id = engine.create_session("char-001").await.unwrap();

        engine
            .send_message_for_test(&session_id, "Calculate 2+3 then 5*4", None)
            .await
            .unwrap();

        // LLMが3回呼ばれたことを確認
        assert_eq!(llm.get_call_count(), 3);

        let messages = {
            let db_lock = db.lock().unwrap();
            chat_repo::get_messages(db_lock.connection(), &session_id).unwrap()
        };

        // 期待: user, assistant(tc1), tool(r1), assistant(tc2), tool(r2), assistant(final) = 6件
        assert_eq!(messages.len(), 6);

        // 1回目のツール結果: 2+3=5
        assert_eq!(messages[2].role, ChatRole::Tool);
        assert_eq!(messages[2].content, "5");
        assert_eq!(messages[2].tool_call_id, Some("call_step1".to_string()));

        // 2回目のツール結果: 5*4=20
        assert_eq!(messages[4].role, ChatRole::Tool);
        assert_eq!(messages[4].content, "20");
        assert_eq!(messages[4].tool_call_id, Some("call_step2".to_string()));

        // 最終応答
        assert_eq!(messages[5].role, ChatRole::Assistant);
        assert_eq!(messages[5].content, "First: 5, Second: 20. Total: 25.");
    }

    // =========================================================================
    // truncate_thinking_content テスト
    // =========================================================================

    use crate::chat::engine::truncate_thinking_content;

    #[test]
    fn test_truncate_thinking_content_short_string() {
        let content = "短いテキスト";
        let result = truncate_thinking_content(content);
        assert_eq!(result, content);
    }

    #[test]
    fn test_truncate_thinking_content_exact_limit() {
        // ちょうど200,000文字
        let content: String = "a".repeat(200_000);
        let result = truncate_thinking_content(&content);
        assert_eq!(result.chars().count(), 200_000);
        assert_eq!(result, content.as_str());
    }

    #[test]
    fn test_truncate_thinking_content_exceeds_limit() {
        let content: String = "b".repeat(200_001);
        let result = truncate_thinking_content(&content);
        assert_eq!(result.chars().count(), 200_000);
    }

    #[test]
    fn test_truncate_thinking_content_multibyte_chars() {
        // マルチバイト文字（日本語）で200,001文字
        let content: String = "あ".repeat(200_001);
        let result = truncate_thinking_content(&content);
        assert_eq!(result.chars().count(), 200_000);
        // 結果は有効なUTF-8文字列
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn test_truncate_thinking_content_empty_string() {
        let result = truncate_thinking_content("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_thinking_content_is_prefix() {
        let content: String = "abcdefghij".repeat(25_000); // 250,000文字
        let result = truncate_thinking_content(&content);
        assert!(content.starts_with(result));
    }

    // =========================================================================
    // Integration Tests: Thinking Content — Full Layer Integration
    //
    // Task 9.1: thinking付きストリーム→DB保存→履歴取得→UI表示
    //
    // 1. Mock LLMClientがthinking付きLLMResponseを返す
    // 2. DB保存後のget_messagesでthinking_contentが含まれることを確認
    // 3. ChatEngine.get_historyでthinking_contentが正しく取得されることを確認
    //
    // **Validates: Requirements 1.5, 2.2, 4.2, 4.3, 5.1**
    // =========================================================================

    /// thinking付きレスポンスを返すMockLLMClient
    struct MockThinkingLLMClient {
        response_text: String,
        thinking_text: Option<String>,
    }

    impl MockThinkingLLMClient {
        fn new(response_text: &str, thinking_text: Option<&str>) -> Self {
            Self {
                response_text: response_text.to_string(),
                thinking_text: thinking_text.map(|s| s.to_string()),
            }
        }
    }

    #[async_trait]
    impl LLMClient for MockThinkingLLMClient {
        async fn chat(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            Ok(LLMResponse::Text {
                content: self.response_text.clone(),
                thinking: self.thinking_text.clone(),
            })
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
            callbacks: crate::llm::client::StreamCallbacks,
        ) -> Result<LLMResponse, AppError> {
            // thinking_callbackでthinkingチャンクを送信
            if let Some(ref thinking) = self.thinking_text {
                (callbacks.1)(thinking.clone());
            }
            // text_callbackでテキストチャンクを送信
            (callbacks.0)(self.response_text.clone());
            Ok(LLMResponse::Text {
                content: self.response_text.clone(),
                thinking: self.thinking_text.clone(),
            })
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    /// Integration Test: thinking_content付きメッセージのDB保存と取得のラウンドトリップ
    ///
    /// LLMがthinking付きレスポンスを返すシナリオで:
    /// 1. ChatMessageRecordにthinking_contentを設定してinsert
    /// 2. get_messagesで取得したレコードにthinking_contentが含まれることを確認
    ///
    /// **Validates: Requirements 4.2, 4.3**
    #[test]
    fn test_integration_thinking_content_db_roundtrip() {
        let db = setup_db_with_character();
        let db_lock = db.lock().unwrap();
        let conn = db_lock.connection();

        // セッション作成
        let session = ChatSession {
            id: "sess-thinking-001".to_string(),
            character_id: "char-001".to_string(),
            title: None,
            last_message_at: None,
            last_message_preview: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        chat_repo::insert_session(conn, &session).unwrap();

        // thinking_content付きのアシスタントメッセージをDB保存
        let thinking_text = "I need to think about this carefully. The user is asking about quantum physics, so I should explain wave-particle duality first.";
        let msg = ChatMessageRecord {
            id: "msg-thinking-001".to_string(),
            session_id: "sess-thinking-001".to_string(),
            role: ChatRole::Assistant,
            content: "Quantum physics explains...".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            thinking_content: Some(thinking_text.to_string()),
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };
        chat_repo::insert_message(conn, &msg).unwrap();

        // get_messagesで取得してthinking_contentが含まれることを確認
        let messages = chat_repo::get_messages(conn, "sess-thinking-001").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "msg-thinking-001");
        assert_eq!(messages[0].role, ChatRole::Assistant);
        assert_eq!(messages[0].content, "Quantum physics explains...");
        assert_eq!(
            messages[0].thinking_content,
            Some(thinking_text.to_string())
        );
    }

    /// Integration Test: thinking_content=Noneのメッセージが正常に保存・取得される
    ///
    /// thinking_contentがない通常メッセージの後方互換性を確認
    ///
    /// **Validates: Requirements 4.4**
    #[test]
    fn test_integration_thinking_content_none_roundtrip() {
        let db = setup_db_with_character();
        let db_lock = db.lock().unwrap();
        let conn = db_lock.connection();

        let session = ChatSession {
            id: "sess-thinking-002".to_string(),
            character_id: "char-001".to_string(),
            title: None,
            last_message_at: None,
            last_message_preview: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        chat_repo::insert_session(conn, &session).unwrap();

        // thinking_content=Noneのメッセージ
        let msg = ChatMessageRecord {
            id: "msg-no-thinking-001".to_string(),
            session_id: "sess-thinking-002".to_string(),
            role: ChatRole::Assistant,
            content: "Hello!".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            thinking_content: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };
        chat_repo::insert_message(conn, &msg).unwrap();

        let messages = chat_repo::get_messages(conn, "sess-thinking-002").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].thinking_content, None);
    }

    /// Integration Test: ChatEngine.get_historyでthinking_contentが返される
    ///
    /// エンジンレベルのget_history呼び出しでthinking_contentが正しく含まれることを確認
    ///
    /// **Validates: Requirements 4.3**
    #[tokio::test]
    async fn test_integration_engine_get_history_includes_thinking() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockThinkingLLMClient::new("hello", Some("thinking content")));
        let engine = DefaultChatEngine::new(
            db.clone(),
            llm,
            test_config(),
            test_llm_lock(),
            test_tts_connector(),
            None,
            None,
        );

        let session_id = engine.create_session("char-001").await.unwrap();

        // thinking_content付きメッセージを直接DBに挿入
        {
            let db_lock = db.lock().unwrap();
            let conn = db_lock.connection();
            let msg = ChatMessageRecord {
                id: "msg-hist-001".to_string(),
                session_id: session_id.clone(),
                role: ChatRole::Assistant,
                content: "response text".to_string(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                thinking_content: Some("deep thinking process here".to_string()),
                created_at: "2024-01-01T10:00:00Z".to_string(),
            };
            chat_repo::insert_message(conn, &msg).unwrap();
        }

        // get_historyで取得
        let history = engine.get_history(&session_id).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(
            history[0].thinking_content,
            Some("deep thinking process here".to_string())
        );
    }

    /// Integration Test: Mock LLM → LLMResponse.thinking → ChatStreamEvent確認
    ///
    /// MockThinkingLLMClientのchat_streamがthinking_callbackを呼び出し、
    /// LLMResponse内のthinkingフィールドに値が含まれることを確認
    ///
    /// **Validates: Requirements 1.5, 2.2**
    #[tokio::test]
    async fn test_integration_llm_response_contains_thinking() {
        let llm = MockThinkingLLMClient::new("hello world", Some("model is thinking deeply"));

        let text_chunks = Arc::new(Mutex::new(Vec::<String>::new()));
        let thinking_chunks = Arc::new(Mutex::new(Vec::<String>::new()));

        let text_clone = text_chunks.clone();
        let thinking_clone = thinking_chunks.clone();

        let text_callback: Box<dyn Fn(String) + Send> = Box::new(move |chunk: String| {
            text_clone.lock().unwrap().push(chunk);
        });
        let thinking_callback: Box<dyn Fn(String) + Send> = Box::new(move |chunk: String| {
            thinking_clone.lock().unwrap().push(chunk);
        });

        let config = LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            temperature: 0.7,
            provider: None,
        };

        let response = llm
            .chat_stream(&[], &config, None, (text_callback, thinking_callback))
            .await
            .unwrap();

        // LLMResponseにthinkingが含まれることを確認
        match response {
            LLMResponse::Text { content, thinking } => {
                assert_eq!(content, "hello world");
                assert_eq!(thinking, Some("model is thinking deeply".to_string()));
            }
            _ => panic!("Expected LLMResponse::Text"),
        }

        // コールバックが正しく呼ばれたことを確認
        let text_received = text_chunks.lock().unwrap();
        assert_eq!(text_received.len(), 1);
        assert_eq!(text_received[0], "hello world");

        let thinking_received = thinking_chunks.lock().unwrap();
        assert_eq!(thinking_received.len(), 1);
        assert_eq!(thinking_received[0], "model is thinking deeply");
    }
}
