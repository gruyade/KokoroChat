//! 記憶管理のプロパティテスト
//! proptest を使用して MemoryManager の不変条件を検証する。
//!
//! **Validates: Requirements 5.1, 5.5**

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use proptest::prelude::*;
    use rusqlite::params;

    use async_trait::async_trait;

    use crate::db::database::Database;
    use crate::db::repositories::{chat as chat_repo, memory as memory_repo};
    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse};
    use crate::memory::manager::{DefaultMemoryManager, MemoryManager};
    use crate::models::{ChatMessageRecord, ChatRole, ChatSession, ToolDefinition};

    // ========================================
    // テスト用MockLLMClient
    // ========================================

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

    // ========================================
    // ストラテジー
    // ========================================

    /// 圧縮閾値のストラテジー（5〜30の範囲で生成）
    fn arb_threshold() -> impl Strategy<Value = u32> {
        5u32..30
    }

    // ========================================
    // ヘルパー
    // ========================================

    fn default_llm_config() -> Arc<crate::config::model_config::ModelConfigManager> {
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
            thought: ThoughtConfig { enabled: false, interval_minutes: 5, auto_delete_threshold_minutes: 1440 },
            memory: MemoryConfig { compression_threshold: 50 },
            tts: TTSGlobalConfig { enabled: false },
            ui: UIConfig { theme: Theme::Dark, language: "ja".to_string() },
            plugins: PluginsConfig { enabled_plugins: vec![], plugin_settings: HashMap::new() },
            attachment: AttachmentConfig { max_file_size_bytes: 10 * 1024 * 1024, allowed_extensions: vec![] },
        };

        Arc::new(crate::config::model_config::ModelConfigManager::new_with_config(config))
    }

    fn setup_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();
        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
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
                id: format!("msg-{:04}", i),
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
                created_at: format!("2024-01-01T{:02}:{:02}:{:02}Z", i / 3600, (i % 3600) / 60, i % 60),
            };
            chat_repo::insert_message(conn, &msg).unwrap();
        }
    }

    // ========================================
    // Property 12: Memory compression threshold trigger
    // ========================================
    //
    // **Validates: Requirements 5.1**
    //
    // For any session with message count >= compression_threshold,
    // calling check_and_compress SHALL create a new Memory record.
    // For any session with message count < compression_threshold,
    // no Memory record SHALL be created.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_no_compression_below_threshold(
            threshold in arb_threshold(),
            msg_count_offset in 1u32..20,
        ) {
            // メッセージ数が閾値未満の場合、Memoryは作成されない
            let msg_count = if msg_count_offset >= threshold {
                threshold - 1
            } else {
                msg_count_offset
            };

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = setup_db();
                let session_id = "sess-001";
                create_session(&db, session_id);
                insert_messages(&db, session_id, msg_count);

                let db_arc = Arc::new(Mutex::new(db));
                let mock_llm = Arc::new(MockLLMClient::new("要約テスト結果"));

                let manager = DefaultMemoryManager::new(
                    db_arc.clone(),
                    mock_llm,
                    default_llm_config(),
                    threshold,
                    Arc::new(tokio::sync::Mutex::new(())),
                );

                manager.check_and_compress(session_id).await.unwrap();

                let db_lock = db_arc.lock().unwrap();
                let memories = memory_repo::list_memories(db_lock.connection(), "char-001").unwrap();

                prop_assert!(
                    memories.is_empty(),
                    "No Memory should be created when message count ({}) < threshold ({}), but found {} memories",
                    msg_count,
                    threshold,
                    memories.len()
                );

                Ok(())
            })?;
        }

        #[test]
        fn prop_compression_at_or_above_threshold(
            threshold in arb_threshold(),
            extra in 0u32..15,
        ) {
            // メッセージ数が閾値以上の場合、Memoryが作成される
            let msg_count = threshold + extra;

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = setup_db();
                let session_id = "sess-001";
                create_session(&db, session_id);
                insert_messages(&db, session_id, msg_count);

                let db_arc = Arc::new(Mutex::new(db));
                let mock_llm = Arc::new(MockLLMClient::new("要約テスト結果"));

                let manager = DefaultMemoryManager::new(
                    db_arc.clone(),
                    mock_llm,
                    default_llm_config(),
                    threshold,
                    Arc::new(tokio::sync::Mutex::new(())),
                );

                manager.check_and_compress(session_id).await.unwrap();

                let db_lock = db_arc.lock().unwrap();
                let memories = memory_repo::list_memories(db_lock.connection(), "char-001").unwrap();

                prop_assert_eq!(
                    memories.len(),
                    1,
                    "Exactly one Memory should be created when message count ({}) >= threshold ({})",
                    msg_count,
                    threshold
                );

                Ok(())
            })?;
        }
    }

    // ========================================
    // Property 13: Memory metadata correctness
    // ========================================
    //
    // **Validates: Requirements 5.5**
    //
    // For any Memory created by compression, it SHALL have:
    // - source_session_id matching the compressed session
    // - source_message_from matching the first message ID
    // - source_message_to matching the last message ID
    // - Non-empty content
    // - Valid timestamps

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_memory_metadata_correctness(
            threshold in arb_threshold(),
            extra in 0u32..10,
        ) {
            let msg_count = threshold + extra;

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = setup_db();
                let session_id = "sess-001";
                create_session(&db, session_id);
                insert_messages(&db, session_id, msg_count);

                let db_arc = Arc::new(Mutex::new(db));
                let mock_llm = Arc::new(MockLLMClient::new("圧縮された要約内容"));

                let manager = DefaultMemoryManager::new(
                    db_arc.clone(),
                    mock_llm,
                    default_llm_config(),
                    threshold,
                    Arc::new(tokio::sync::Mutex::new(())),
                );

                manager.check_and_compress(session_id).await.unwrap();

                let db_lock = db_arc.lock().unwrap();
                let memories = memory_repo::list_memories(db_lock.connection(), "char-001").unwrap();

                prop_assert_eq!(memories.len(), 1);
                let memory = &memories[0];

                // source_session_id がセッションIDと一致
                prop_assert_eq!(
                    &memory.source_session_id,
                    &Some(session_id.to_string()),
                    "source_session_id should match the compressed session"
                );

                // source_message_from が最初のメッセージID
                let expected_first = "msg-0000".to_string();
                prop_assert_eq!(
                    &memory.source_message_from,
                    &Some(expected_first),
                    "source_message_from should match the first message ID"
                );

                // source_message_to が最後のメッセージID
                let expected_last = format!("msg-{:04}", msg_count - 1);
                prop_assert_eq!(
                    &memory.source_message_to,
                    &Some(expected_last),
                    "source_message_to should match the last message ID"
                );

                // content が非空
                prop_assert!(
                    !memory.content.is_empty(),
                    "Memory content should not be empty"
                );

                // created_at が有効なタイムスタンプ（RFC3339パース可能）
                prop_assert!(
                    chrono::DateTime::parse_from_rfc3339(&memory.created_at).is_ok(),
                    "created_at should be a valid RFC3339 timestamp, got: {}",
                    &memory.created_at
                );

                // updated_at が有効なタイムスタンプ
                prop_assert!(
                    chrono::DateTime::parse_from_rfc3339(&memory.updated_at).is_ok(),
                    "updated_at should be a valid RFC3339 timestamp, got: {}",
                    &memory.updated_at
                );

                Ok(())
            })?;
        }
    }

    // ========================================
    // Property 14: No re-compression after previous compression without new messages
    // ========================================
    //
    // **Validates: Requirements 5.1**
    //
    // After a successful compression, calling check_and_compress again
    // without adding new messages SHALL NOT create additional Memory records.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_no_recompression_without_new_messages(
            threshold in arb_threshold(),
            extra in 0u32..10,
        ) {
            let msg_count = threshold + extra;

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = setup_db();
                let session_id = "sess-001";
                create_session(&db, session_id);
                insert_messages(&db, session_id, msg_count);

                let db_arc = Arc::new(Mutex::new(db));
                let mock_llm = Arc::new(MockLLMClient::new("要約テスト結果"));

                let manager = DefaultMemoryManager::new(
                    db_arc.clone(),
                    mock_llm,
                    default_llm_config(),
                    threshold,
                    Arc::new(tokio::sync::Mutex::new(())),
                );

                // 1回目の圧縮
                manager.check_and_compress(session_id).await.unwrap();

                {
                    let db_lock = db_arc.lock().unwrap();
                    let memories = memory_repo::list_memories(db_lock.connection(), "char-001").unwrap();
                    prop_assert_eq!(memories.len(), 1, "First compression should create one memory");
                }

                // 2回目: 新規メッセージなし → 再圧縮されない
                manager.check_and_compress(session_id).await.unwrap();

                {
                    let db_lock = db_arc.lock().unwrap();
                    let memories = memory_repo::list_memories(db_lock.connection(), "char-001").unwrap();
                    prop_assert_eq!(
                        memories.len(),
                        1,
                        "No additional memory should be created without new messages (msg_count={}, threshold={})",
                        msg_count,
                        threshold
                    );
                }

                Ok(())
            })?;
        }
    }
}
