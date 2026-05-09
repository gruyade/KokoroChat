//! 独自思考のプロパティテスト
//! proptest を使用して ThoughtEngine の不変条件を検証する。
//!
//! **Validates: Requirements 4.2, 4.4**

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use proptest::prelude::*;
    use rusqlite::params;

    use async_trait::async_trait;

    use crate::db::database::Database;
    use crate::db::repositories::{chat as chat_repo, thought as thought_repo};
    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
    use crate::models::{ChatMessageRecord, ChatRole, Memory, ToolDefinition};
    use crate::thought::engine::{DefaultThoughtEngine, ThoughtEngine};

    // ========================================
    // テスト用MockLLMClient
    // ========================================

    struct MockLLMClient {
        response: Mutex<String>,
    }

    impl MockLLMClient {
        fn new(response: &str) -> Self {
            Self {
                response: Mutex::new(response.to_string()),
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
            let resp = self.response.lock().unwrap().clone();
            Ok(LLMResponse::Text(resp))
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
            let resp = self.response.lock().unwrap().clone();
            Ok(resp)
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    // ========================================
    // ストラテジー
    // ========================================

    /// 非空文字列を生成するストラテジー（trim後も非空であることを保証）
    fn non_empty_string() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9ぁ-んァ-ヶ]{1,50}".prop_map(|s| s)
    }

    /// UUID文字列を生成するストラテジー
    fn uuid_string() -> impl Strategy<Value = String> {
        "[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}"
    }

    /// ISO 8601日時文字列を生成するストラテジー
    fn iso8601_datetime() -> impl Strategy<Value = String> {
        (2020u32..2030, 1u32..13, 1u32..29, 0u32..24, 0u32..60, 0u32..60).prop_map(
            |(y, m, d, h, min, s)| {
                format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, min, s)
            },
        )
    }

    /// ChatRoleのストラテジー（User, Assistant, Spontaneous）
    fn arb_chat_role() -> impl Strategy<Value = ChatRole> {
        prop_oneof![
            Just(ChatRole::User),
            Just(ChatRole::Assistant),
            Just(ChatRole::Spontaneous),
        ]
    }

    /// ChatMessageRecordのストラテジー
    fn arb_chat_message_record() -> impl Strategy<Value = ChatMessageRecord> {
        (uuid_string(), uuid_string(), arb_chat_role(), non_empty_string(), iso8601_datetime())
            .prop_map(|(id, session_id, role, content, created_at)| ChatMessageRecord {
                id,
                session_id,
                role,
                content,
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                created_at,
            })
    }

    /// メッセージ履歴のストラテジー（0〜10件）
    fn arb_message_history(max_count: usize) -> impl Strategy<Value = Vec<ChatMessageRecord>> {
        proptest::collection::vec(arb_chat_message_record(), 0..=max_count)
    }

    /// Memory のストラテジー（0〜5件）
    fn arb_memories(max_count: usize) -> impl Strategy<Value = Vec<Memory>> {
        proptest::collection::vec(
            (uuid_string(), non_empty_string(), iso8601_datetime(), iso8601_datetime())
                .prop_map(|(id, content, created_at, updated_at)| Memory {
                    id,
                    character_id: "char-001".to_string(),
                    content,
                    source_session_id: None,
                    source_message_from: None,
                    source_message_to: None,
                    created_at,
                    updated_at,
                }),
            0..=max_count,
        )
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
            temperature: 0.7,
            provider: None,
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
            tts: TTSGlobalConfig { enabled: false, voicepeak_path: None, timeout_seconds: 60, max_chunk_size: 140, irodori_base_url: None, irodori_caption_base_url: None, irodori_reference_audio_base_url: None },
            ui: UIConfig { theme: Theme::Dark, language: "ja".to_string(), send_key: SendKey::default() },
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
                "テストキャラ",
                "テスト用",
                "あなたは猫のキャラクターです。",
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z"
            ],
        )
        .unwrap();

        db
    }

    fn setup_session(db: &Database, session_id: &str) {
        let conn = db.connection();
        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, title, last_message_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                "char-001",
                "テストセッション",
                "2024-01-01T12:00:00Z",
                "2024-01-01T00:00:00Z"
            ],
        )
        .unwrap();
    }

    // ========================================
    // Property 10: Thought storage separation
    // ========================================
    //
    // **Validates: Requirements 4.2**
    //
    // For any generated Thought, it SHALL exist only in the thoughts storage
    // and SHALL NOT appear in any chat_messages query result.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_thought_storage_separation(
            thought_content in non_empty_string(),
        ) {
            // tokio runtime for async operations
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = setup_db();
                let session_id = "sess-001";
                setup_session(&db, session_id);

                let db = Arc::new(Mutex::new(db));
                let mock_llm = Arc::new(MockLLMClient::new(&thought_content));
                let config = default_llm_config();

                let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, Arc::new(tokio::sync::Mutex::new(())));

                // 思考を生成
                let thought = engine.generate_thought("char-001").await.unwrap();

                // 1. 思考がthoughtsストレージに存在する
                let thoughts = engine.get_thoughts("char-001", None).await.unwrap();
                prop_assert!(
                    thoughts.iter().any(|t| t.id == thought.id),
                    "Generated thought should exist in thoughts storage"
                );

                // 2. 思考がchat_messagesに存在しない
                let db_guard = db.lock().unwrap();
                let conn = db_guard.connection();
                let messages = chat_repo::get_messages(conn, session_id).unwrap();
                prop_assert!(
                    !messages.iter().any(|m| m.content == thought.content && m.id == thought.id),
                    "Thought should NOT appear in chat_messages"
                );

                Ok(())
            })?;
        }

        #[test]
        fn prop_thought_not_in_any_session_messages(
            thought_content in non_empty_string(),
            num_sessions in 1usize..4,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = setup_db();

                // 複数セッションを作成
                let session_ids: Vec<String> = (0..num_sessions)
                    .map(|i| format!("sess-{:03}", i))
                    .collect();
                for sid in &session_ids {
                    setup_session(&db, sid);
                }

                let db = Arc::new(Mutex::new(db));
                let mock_llm = Arc::new(MockLLMClient::new(&thought_content));
                let config = default_llm_config();

                let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, Arc::new(tokio::sync::Mutex::new(())));

                // 思考を生成
                let thought = engine.generate_thought("char-001").await.unwrap();

                // 全セッションのメッセージを確認
                let db_guard = db.lock().unwrap();
                let conn = db_guard.connection();
                for sid in &session_ids {
                    let messages = chat_repo::get_messages(conn, sid).unwrap();
                    prop_assert!(
                        !messages.iter().any(|m| m.id == thought.id),
                        "Thought ID should NOT appear in session {} messages",
                        sid
                    );
                }

                // thoughtsストレージには存在する
                let thoughts = thought_repo::get_thoughts(conn, "char-001", None).unwrap();
                prop_assert!(
                    thoughts.iter().any(|t| t.id == thought.id),
                    "Thought should exist in thoughts storage"
                );

                Ok(())
            })?;
        }
    }

    // ========================================
    // Property 11: Thought generation context includes chat and memories
    // ========================================
    //
    // **Validates: Requirements 4.4**
    //
    // For any thought generation request for a Character with existing
    // ChatSessions and Memories, the LLM request SHALL include both
    // recent chat messages and relevant Memory content as context.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_thought_prompt_includes_system_prompt(
            system_prompt in non_empty_string(),
            messages in arb_message_history(10),
            memories in arb_memories(5),
        ) {
            let result = DefaultThoughtEngine::build_thought_prompt(
                &system_prompt,
                &messages,
                &memories,
            );

            // 最初のメッセージは必ずシステムプロンプト（role=System）
            prop_assert!(
                !result.is_empty(),
                "Prompt should not be empty"
            );
            prop_assert_eq!(
                result[0].role,
                MessageRole::System,
                "First message must be System role"
            );
            prop_assert_eq!(
                &result[0].content,
                &system_prompt,
                "First message content must be the system prompt"
            );
        }

        #[test]
        fn prop_thought_prompt_includes_memories(
            system_prompt in non_empty_string(),
            messages in arb_message_history(5),
            memories in arb_memories(5),
        ) {
            let result = DefaultThoughtEngine::build_thought_prompt(
                &system_prompt,
                &messages,
                &memories,
            );

            if !memories.is_empty() {
                // 記憶がある場合、システムプロンプトの次に記憶メッセージが存在する
                prop_assert!(
                    result.len() >= 2,
                    "With memories, prompt should have at least 2 messages"
                );
                prop_assert_eq!(
                    result[1].role,
                    MessageRole::System,
                    "Memory message should have System role"
                );
                // 各記憶の内容が含まれている
                for memory in &memories {
                    prop_assert!(
                        result[1].content.contains(&memory.content),
                        "Memory content '{}' should be included in prompt",
                        &memory.content
                    );
                }
            }
        }

        #[test]
        fn prop_thought_prompt_includes_chat_history(
            system_prompt in non_empty_string(),
            messages in arb_message_history(10),
            memories in arb_memories(3),
        ) {
            let result = DefaultThoughtEngine::build_thought_prompt(
                &system_prompt,
                &messages,
                &memories,
            );

            // 会話メッセージのオフセット: 1 (system) + memories分 (0 or 1)
            let memory_offset = if memories.is_empty() { 0 } else { 1 };
            let chat_start = 1 + memory_offset;

            // 各会話メッセージの内容が正しく含まれている
            for (i, msg) in messages.iter().enumerate() {
                let prompt_idx = chat_start + i;
                prop_assert_eq!(
                    &result[prompt_idx].content,
                    &msg.content,
                    "Chat message at index {} content mismatch",
                    i
                );
            }
        }

        #[test]
        fn prop_thought_prompt_ends_with_meta_prompt(
            system_prompt in non_empty_string(),
            messages in arb_message_history(10),
            memories in arb_memories(5),
        ) {
            let result = DefaultThoughtEngine::build_thought_prompt(
                &system_prompt,
                &messages,
                &memories,
            );

            // 最後のメッセージはメタプロンプト（role=User, "internal thought"を含む）
            let last = result.last().unwrap();
            prop_assert_eq!(
                last.role,
                MessageRole::User,
                "Last message (meta-prompt) should be User role"
            );
            prop_assert!(
                last.content.contains("internal thought"),
                "Meta-prompt should contain 'internal thought' instruction, got: {}",
                &last.content
            );
        }

        #[test]
        fn prop_thought_prompt_total_message_count(
            system_prompt in non_empty_string(),
            messages in arb_message_history(10),
            memories in arb_memories(5),
        ) {
            let result = DefaultThoughtEngine::build_thought_prompt(
                &system_prompt,
                &messages,
                &memories,
            );

            // 総数 = 1 (system) + memory分 (0 or 1) + messages.len() + 1 (meta-prompt)
            let memory_count = if memories.is_empty() { 0 } else { 1 };
            let expected_len = 1 + memory_count + messages.len() + 1;
            prop_assert_eq!(
                result.len(),
                expected_len,
                "Prompt length should be 1 (system) + {} (memory) + {} (messages) + 1 (meta) = {}, got {}",
                memory_count,
                messages.len(),
                expected_len,
                result.len()
            );
        }

        #[test]
        fn prop_thought_prompt_role_mapping(
            system_prompt in non_empty_string(),
            messages in arb_message_history(10),
            memories in arb_memories(3),
        ) {
            let result = DefaultThoughtEngine::build_thought_prompt(
                &system_prompt,
                &messages,
                &memories,
            );

            let memory_offset = if memories.is_empty() { 0 } else { 1 };
            let chat_start = 1 + memory_offset;

            // ChatRole → MessageRole のマッピング検証
            for (i, msg) in messages.iter().enumerate() {
                let prompt_idx = chat_start + i;
                let expected_role = match msg.role {
                    ChatRole::User => MessageRole::User,
                    ChatRole::Assistant => MessageRole::Assistant,
                    ChatRole::Spontaneous => MessageRole::Assistant,
                    ChatRole::Tool => MessageRole::Tool,
                };
                prop_assert_eq!(
                    result[prompt_idx].role,
                    expected_role,
                    "Role mapping at index {} incorrect: {:?} should map to {:?}",
                    i,
                    msg.role,
                    expected_role
                );
            }
        }
    }
}
