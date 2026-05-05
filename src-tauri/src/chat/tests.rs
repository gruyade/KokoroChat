// Chat Engine tests

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use crate::chat::engine::{ChatEngine, DefaultChatEngine};
    use crate::db::database::Database;
    use crate::db::repositories::{character as char_repo, chat as chat_repo, memory as mem_repo};
    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
    use crate::models::{
        Attachment, AttachmentType, Character, ChatRole, Memory, ToolCall, ToolDefinition,
    };

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
            Ok(LLMResponse::Text(self.response_text.clone()))
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
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
                callback(text);
            }
            Ok(full)
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
            Ok(LLMResponse::ToolCalls(vec![ToolCall {
                id: "call_001".to_string(),
                name: "calculator".to_string(),
                arguments: serde_json::json!({"expression": "2+2"}),
            }]))
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
            Ok(String::new())
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
            Ok(LLMResponse::Text("OK".to_string()))
        }

        async fn chat_stream(
            &self,
            messages: &[ChatMessage],
            _config: &LLMClientConfig,
            callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
            let mut cap = self.captured.lock().unwrap();
            *cap = messages.to_vec();
            callback("OK".to_string());
            Ok("OK".to_string())
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn test_config() -> LLMClientConfig {
        LLMClientConfig {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            temperature: 0.7,
        }
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
        let engine = DefaultChatEngine::new(db.clone(), llm, test_config());

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
        let engine = DefaultChatEngine::new(db, llm, test_config());

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
        let engine = DefaultChatEngine::new(db, llm, test_config());

        let _s1 = engine.create_session("char-001").await.unwrap();
        let _s2 = engine.create_session("char-001").await.unwrap();

        let sessions = engine.list_sessions("char-001").await.unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_list_sessions_empty() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(db, llm, test_config());

        let sessions = engine.list_sessions("char-001").await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_delete_session() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(db.clone(), llm, test_config());

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
        let engine = DefaultChatEngine::new(db, llm, test_config());

        let session_id = engine.create_session("char-001").await.unwrap();
        let history = engine.get_history(&session_id).await.unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_build_context_basic() {
        let db = setup_db_with_character();
        let llm = Arc::new(MockLLMClient::new("hello"));
        let engine = DefaultChatEngine::new(db, llm, test_config());

        let messages = engine.build_context(
            "あなたはテストキャラです。",
            &[],
            &[],
            "こんにちは",
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
        let engine = DefaultChatEngine::new(db, llm, test_config());

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

        let messages = engine.build_context(
            "System prompt",
            &memories,
            &[],
            "Hello",
            None,
        );

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
        let engine = DefaultChatEngine::new(db, llm, test_config());

        let history = vec![
            crate::models::ChatMessageRecord {
                id: "msg-001".to_string(),
                session_id: "sess-001".to_string(),
                role: ChatRole::User,
                content: "前のメッセージ".to_string(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
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
                created_at: "2024-01-01T10:01:00Z".to_string(),
            },
        ];

        let messages = engine.build_context(
            "System prompt",
            &[],
            &history,
            "新しいメッセージ",
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
        let engine = DefaultChatEngine::new(db, llm, test_config());

        let messages = engine.build_context(
            "System prompt",
            &[],
            &[],
            "ファイルを見て",
            Some("--- test.txt ---\nファイル内容"),
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
        let engine = DefaultChatEngine::new(db, llm, test_config());

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
            created_at: "2024-01-01T10:00:00Z".to_string(),
        }];

        let messages = engine.build_context(
            "システムプロンプト",
            &memories,
            &history,
            "新規メッセージ",
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
        let engine = DefaultChatEngine::new(db, llm, test_config());

        let history = vec![crate::models::ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::Spontaneous,
            content: "自発的発話".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        }];

        let messages = engine.build_context("Prompt", &[], &history, "Hi", None);

        // Spontaneous → Assistant にマッピング
        assert_eq!(messages[1].role, MessageRole::Assistant);
    }
}
