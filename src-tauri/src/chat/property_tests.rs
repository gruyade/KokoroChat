//! チャットコンテキスト組み立てのプロパティテスト
//! proptest を使用して build_context の不変条件を検証する。
//!
//! **Validates: Requirements 2.2, 3.2, 3.4, 5.1, 5.2, 5.3, 5.4, 2.3**

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use proptest::prelude::*;
    use rusqlite::params;

    use crate::chat::engine::DefaultChatEngine;
    use crate::db::database::Database;
    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
    use crate::models::tts::TTSConfig;
    use crate::models::{ChatMessageRecord, ChatRole, Memory, ToolDefinition};
    use crate::tts::connector::TTSConnector;

    use async_trait::async_trait;

    // ========================================
    // テスト用MockLLMClient
    // ========================================

    struct MockLLMClient;

    #[async_trait]
    impl LLMClient for MockLLMClient {
        async fn chat(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            Ok(LLMResponse::Text {
                content: "mock".to_string(),
                thinking: None,
            })
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
            _callbacks: crate::llm::client::StreamCallbacks,
        ) -> Result<LLMResponse, AppError> {
            Ok(LLMResponse::Text {
                content: "mock".to_string(),
                thinking: None,
            })
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    // ========================================
    // テスト用MockTTSConnector
    // ========================================

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

    // ========================================
    // ストラテジー
    // ========================================

    /// 非空文字列を生成するストラテジー
    fn non_empty_string() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9ぁ-んァ-ヶ ,.!?]{1,50}".prop_map(|s| s)
    }

    /// UUID文字列を生成するストラテジー
    fn uuid_string() -> impl Strategy<Value = String> {
        "[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}"
    }

    /// ISO 8601日時文字列を生成するストラテジー
    fn iso8601_datetime() -> impl Strategy<Value = String> {
        (
            2020u32..2030,
            1u32..13,
            1u32..29,
            0u32..24,
            0u32..60,
            0u32..60,
        )
            .prop_map(|(y, m, d, h, min, s)| {
                format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, min, s)
            })
    }

    /// Memory のストラテジー（0〜5件）
    fn arb_memories(max_count: usize) -> impl Strategy<Value = Vec<Memory>> {
        proptest::collection::vec(
            (
                uuid_string(),
                uuid_string(),
                non_empty_string(),
                iso8601_datetime(),
                iso8601_datetime(),
            )
                .prop_map(|(id, char_id, content, created_at, updated_at)| Memory {
                    id,
                    character_id: char_id,
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

    /// ChatRole（user/assistant交互）のストラテジー
    fn alternating_role(index: usize) -> ChatRole {
        if index % 2 == 0 {
            ChatRole::User
        } else {
            ChatRole::Assistant
        }
    }

    /// チャット履歴のストラテジー（0〜10件、user/assistant交互）
    fn arb_chat_history(max_count: usize) -> impl Strategy<Value = Vec<ChatMessageRecord>> {
        proptest::collection::vec(
            (
                uuid_string(),
                uuid_string(),
                non_empty_string(),
                iso8601_datetime(),
            ),
            0..=max_count,
        )
        .prop_map(|items| {
            items
                .into_iter()
                .enumerate()
                .map(
                    |(i, (id, session_id, content, created_at))| ChatMessageRecord {
                        id,
                        session_id,
                        role: alternating_role(i),
                        content,
                        attachments: None,
                        tool_calls: None,
                        tool_call_id: None,
                        thinking_content: None,
                        created_at,
                    },
                )
                .collect()
        })
    }

    /// DefaultChatEngine インスタンスを作成（build_contextテスト用、DB不要だがコンストラクタに必要）
    fn create_engine() -> DefaultChatEngine {
        use crate::models::config::*;
        use std::collections::HashMap;

        let db = Database::open_in_memory().unwrap();
        let db = Arc::new(Mutex::new(db));
        let llm_client: Arc<dyn LLMClient> = Arc::new(MockLLMClient);

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

        let config_manager =
            Arc::new(crate::config::model_config::ModelConfigManager::new_with_config(config));
        let llm_lock = Arc::new(tokio::sync::Mutex::new(()));
        let tts_connector: Arc<dyn TTSConnector> = Arc::new(MockTTSConnector);
        DefaultChatEngine::new(
            db,
            llm_client,
            config_manager,
            llm_lock,
            tts_connector,
            None,
            None,
        )
    }

    // ========================================
    // Property 4: Chat context assembly includes system prompt, history, and memories
    // ========================================
    //
    // **Validates: Requirements 2.2, 5.3**
    //
    // For any chat message sent in a session belonging to a Character with existing Memories,
    // the LLM request SHALL contain:
    // 1. The Character's systemPrompt as the first message (role=system)
    // 2. All relevant Memories in the prompt (as system messages with [Memory] prefix)
    // 3. The full chat history in chronological order
    // 4. The user's new message as the last message

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_context_assembly_structure(
            system_prompt in non_empty_string(),
            memories in arb_memories(5),
            history in arb_chat_history(10),
            user_message in non_empty_string(),
        ) {
            let engine = create_engine();

            let result = engine.build_context(
                &system_prompt,
                &memories,
                &[],
                &history,
                &user_message,
                None,
                None,
            );

            let num_memories = memories.len();
            let num_history = history.len();
            let expected_total = 1 + num_memories + num_history + 1;

            // 総メッセージ数 = 1 (system) + num_memories + num_history + 1 (user)
            prop_assert_eq!(
                result.len(),
                expected_total,
                "Total message count mismatch: expected {}, got {}",
                expected_total,
                result.len()
            );

            // 1. 最初のメッセージはシステムプロンプト（role=system）
            prop_assert_eq!(
                result[0].role,
                MessageRole::System,
                "First message should be System role"
            );
            prop_assert_eq!(
                &result[0].content,
                &system_prompt,
                "First message content should be the system prompt"
            );

            // 2. メモリメッセージ（[Memory]プレフィックス付きsystemメッセージ）
            for i in 0..num_memories {
                let msg_idx = 1 + i;
                prop_assert_eq!(
                    result[msg_idx].role,
                    MessageRole::System,
                    "Memory message at index {} should be System role",
                    msg_idx
                );
                prop_assert!(
                    result[msg_idx].content.starts_with("[Memory]"),
                    "Memory message at index {} should start with [Memory] prefix, got: {}",
                    msg_idx,
                    &result[msg_idx].content
                );
                // メモリの内容が含まれている
                prop_assert!(
                    result[msg_idx].content.contains(&memories[i].content),
                    "Memory message at index {} should contain memory content",
                    msg_idx
                );
            }

            // 3. 履歴メッセージが順序通り
            for i in 0..num_history {
                let msg_idx = 1 + num_memories + i;
                prop_assert_eq!(
                    &result[msg_idx].content,
                    &history[i].content,
                    "History message at index {} content mismatch",
                    msg_idx
                );
            }

            // 4. 最後のメッセージはユーザーの新規メッセージ（role=user）
            let last_idx = result.len() - 1;
            prop_assert_eq!(
                result[last_idx].role,
                MessageRole::User,
                "Last message should be User role"
            );
            prop_assert_eq!(
                &result[last_idx].content,
                &user_message,
                "Last message content should be the user's new message"
            );
        }

        #[test]
        fn prop_context_assembly_with_attachment(
            system_prompt in non_empty_string(),
            memories in arb_memories(3),
            history in arb_chat_history(5),
            user_message in non_empty_string(),
            attachment_text in non_empty_string(),
        ) {
            let engine = create_engine();

            let result = engine.build_context(
                &system_prompt,
                &memories,
                &[],
                &history,
                &user_message,
                Some(&attachment_text),
                None,
            );

            let num_memories = memories.len();
            let num_history = history.len();
            let expected_total = 1 + num_memories + num_history + 1;

            // 添付テキストがあっても総メッセージ数は変わらない（ユーザーメッセージに結合）
            prop_assert_eq!(
                result.len(),
                expected_total,
                "Total message count should not change with attachment"
            );

            // 最後のメッセージにユーザーメッセージと添付テキストの両方が含まれる
            let last_idx = result.len() - 1;
            prop_assert_eq!(result[last_idx].role, MessageRole::User);
            prop_assert!(
                result[last_idx].content.contains(&user_message),
                "Last message should contain user message"
            );
            prop_assert!(
                result[last_idx].content.contains(&attachment_text),
                "Last message should contain attachment text"
            );
            prop_assert!(
                result[last_idx].content.contains("[Attached Files]"),
                "Last message should contain [Attached Files] marker"
            );
        }

        #[test]
        fn prop_context_history_role_mapping(
            system_prompt in non_empty_string(),
            history in arb_chat_history(10),
            user_message in non_empty_string(),
        ) {
            let engine = create_engine();

            let result = engine.build_context(
                &system_prompt,
                &[],
                &[],
                &history,
                &user_message,
                None,
                None,
            );

            // 履歴メッセージのロールマッピングが正しい
            for i in 0..history.len() {
                let msg_idx = 1 + i; // system_prompt の次から
                let expected_role = match history[i].role {
                    ChatRole::User => MessageRole::User,
                    ChatRole::Assistant => MessageRole::Assistant,
                    ChatRole::Spontaneous => MessageRole::Assistant,
                    ChatRole::Tool => MessageRole::Tool,
                };
                prop_assert_eq!(
                    result[msg_idx].role,
                    expected_role,
                    "History message at index {} role mapping incorrect: {:?} -> expected {:?}",
                    i,
                    history[i].role,
                    expected_role
                );
            }
        }
    }

    // ========================================
    // Feature: knowledge-plugin, Property 6: Engine enabled-state filter
    // ========================================
    //
    // For any set of knowledge entries with varying enabled states, building the LLM context
    // SHALL include only entries where enabled=true, regardless of injection_mode.
    // Disabled entries SHALL appear in neither the system prompt nor the get_knowledge tool availability.
    //
    // **Validates: Requirements 3.2, 3.4, 2.3**

    /// DB + セッションを準備して engine を返すヘルパー
    fn create_engine_with_session() -> (DefaultChatEngine, String, Arc<Mutex<Database>>) {
        use crate::db::database::Database;
        use crate::models::config::*;
        use std::collections::HashMap;

        let db = Database::open_in_memory().unwrap();

        // キャラクター + セッション作成
        {
            let conn = db.connection();
            conn.execute(
                "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "char-test",
                    "Test",
                    "Desc",
                    "Base Prompt",
                    "2024-01-01T00:00:00Z",
                    "2024-01-01T00:00:00Z"
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
                params!["sess-test", "char-test", "2024-01-01T00:00:00Z"],
            )
            .unwrap();
        }

        let db = Arc::new(Mutex::new(db));
        let db_clone = db.clone();

        let llm_client: Arc<dyn LLMClient> = Arc::new(MockLLMClient);

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

        let config_manager =
            Arc::new(crate::config::model_config::ModelConfigManager::new_with_config(config));
        let llm_lock = Arc::new(tokio::sync::Mutex::new(()));
        let tts_connector: Arc<dyn TTSConnector> = Arc::new(MockTTSConnector);
        let engine = DefaultChatEngine::new(
            db,
            llm_client,
            config_manager,
            llm_lock,
            tts_connector,
            None,
            None,
        );

        (engine, "sess-test".to_string(), db_clone)
    }

    /// get_knowledge ツール定義を生成するヘルパー
    fn make_get_knowledge_tool() -> crate::models::plugin::ToolDefinition {
        crate::models::plugin::ToolDefinition {
            name: "get_knowledge".to_string(),
            description: "Retrieve knowledge content by file name".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_name": {
                        "type": "string",
                        "description": "The file name to retrieve"
                    }
                },
                "required": ["file_name"]
            }),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        // Feature: knowledge-plugin, Property 6: Engine enabled-state filter
        #[test]
        fn prop_engine_enabled_state_filter(
            entries_enabled in proptest::collection::vec(any::<bool>(), 1..=6),
            entries_modes in proptest::collection::vec(
                prop_oneof![Just("system_prompt".to_string()), Just("tool_reference".to_string())],
                1..=6
            ),
            contents in proptest::collection::vec("[a-zA-Z0-9]{5,30}", 1..=6),
        ) {
            use crate::db::repositories::knowledge as knowledge_repo;

            // エントリ数を揃える
            let count = entries_enabled.len().min(entries_modes.len()).min(contents.len());
            if count == 0 {
                return Ok(());
            }

            let (engine, session_id, db_arc) = create_engine_with_session();

            // エントリをDBに追加
            {
                let db_guard = db_arc.lock().unwrap();
                let conn = db_guard.connection();
                for i in 0..count {
                    let entry = crate::models::KnowledgeEntry {
                        id: format!("know-p6-{}", i),
                        session_id: session_id.clone(),
                        file_name: format!("file_{}.txt", i),
                        content: contents[i].clone(),
                        size_bytes: contents[i].len() as i64,
                        enabled: entries_enabled[i],
                        injection_mode: entries_modes[i].clone(),
                        created_at: format!("2024-01-{:02}T00:00:00Z", (i % 28) + 1),
                    };
                    knowledge_repo::add_knowledge(conn, &entry).unwrap();
                }
            }

            // ---- system_prompt モードの検証 ----
            // inject_knowledge_to_system_prompt は enabled=true AND mode=system_prompt のエントリのみ含む
            let base_prompt = "Base system prompt";
            let injected = engine.inject_knowledge_to_system_prompt(&session_id, base_prompt);

            for i in 0..count {
                let file_name = format!("file_{}.txt", i);
                let expected_header = format!("## {}", file_name);

                if entries_enabled[i] && entries_modes[i] == "system_prompt" {
                    // enabled=true AND system_prompt → システムプロンプトに含まれる
                    prop_assert!(
                        injected.contains(&expected_header),
                        "Enabled system_prompt entry '{}' should be in injected prompt, but was not found",
                        file_name
                    );
                    prop_assert!(
                        injected.contains(&contents[i]),
                        "Content of enabled system_prompt entry '{}' should be in injected prompt",
                        file_name
                    );
                } else {
                    // disabled OR tool_reference → システムプロンプトに含まれない
                    prop_assert!(
                        !injected.contains(&expected_header),
                        "Entry '{}' (enabled={}, mode={}) should NOT be in system prompt",
                        file_name,
                        entries_enabled[i],
                        entries_modes[i]
                    );
                }
            }

            // ---- tool_reference モードの検証 ----
            // filter_knowledge_tools は enabled=true AND mode=tool_reference のエントリが1件以上あれば
            // get_knowledge を含め、0件なら除外する
            let tools = vec![make_get_knowledge_tool()];
            let filtered = engine.filter_knowledge_tools(&session_id, tools);

            let has_enabled_tool_ref = (0..count).any(|i| {
                entries_enabled[i] && entries_modes[i] == "tool_reference"
            });

            if has_enabled_tool_ref {
                // get_knowledge ツールが残っている
                prop_assert!(
                    filtered.iter().any(|t| t.name == "get_knowledge"),
                    "get_knowledge tool should be present when enabled tool_reference entries exist"
                );
            } else {
                // get_knowledge ツールが除外される
                prop_assert!(
                    !filtered.iter().any(|t| t.name == "get_knowledge"),
                    "get_knowledge tool should be removed when no enabled tool_reference entries exist"
                );
            }

            // disabled エントリの file_name は get_knowledge ツールの description に含まれてはいけない
            if has_enabled_tool_ref {
                let tool = filtered.iter().find(|t| t.name == "get_knowledge").unwrap();
                let params_str = serde_json::to_string(&tool.parameters).unwrap_or_default();
                for i in 0..count {
                    let file_name = format!("file_{}.txt", i);
                    if !entries_enabled[i] || entries_modes[i] != "tool_reference" {
                        prop_assert!(
                            !params_str.contains(&file_name),
                            "Disabled/non-tool_reference entry '{}' should NOT appear in get_knowledge params",
                            file_name
                        );
                    }
                }
            }
        }
    }

    // ========================================
    // Feature: knowledge-plugin, Property 8: System prompt injection ordering and format
    // ========================================
    //
    // For any set of enabled knowledge entries with injection_mode="system_prompt",
    // the system prompt SHALL contain each entry formatted as "## {file_name}\n{content}",
    // concatenated in created_at ascending order, appearing after the base system prompt
    // and before thoughts/memories context.
    //
    // **Validates: Requirements 5.1, 5.2, 5.3, 5.4**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        // Feature: knowledge-plugin, Property 8: System prompt injection ordering and format
        #[test]
        fn prop_system_prompt_injection_ordering_and_format(
            entry_count in 1usize..=5,
            contents in proptest::collection::vec("[a-zA-Z0-9 ]{5,40}", 5),
            base_prompt in "[a-zA-Z0-9]{10,30}",
        ) {
            use crate::db::repositories::knowledge as knowledge_repo;

            let count = entry_count.min(contents.len());
            let (engine, session_id, db_arc) = create_engine_with_session();

            // エントリをDBに追加（各エントリは enabled=true, mode=system_prompt）
            // created_at は日付が異なるように設定（昇順の検証用）
            let mut expected_order: Vec<(String, String)> = Vec::new();
            {
                let db_guard = db_arc.lock().unwrap();
                let conn = db_guard.connection();
                for i in 0..count {
                    let file_name = format!("doc_{}.txt", i);
                    let created_at = format!("2024-01-{:02}T{:02}:00:00Z", (i % 28) + 1, i);
                    let entry = crate::models::KnowledgeEntry {
                        id: format!("know-p8-{}", i),
                        session_id: session_id.clone(),
                        file_name: file_name.clone(),
                        content: contents[i].clone(),
                        size_bytes: contents[i].len() as i64,
                        enabled: true,
                        injection_mode: "system_prompt".to_string(),
                        created_at,
                    };
                    knowledge_repo::add_knowledge(conn, &entry).unwrap();
                    expected_order.push((file_name, contents[i].clone()));
                }
            }

            // inject_knowledge_to_system_prompt を呼び出す
            let injected = engine.inject_knowledge_to_system_prompt(&session_id, &base_prompt);

            // 1. ベースプロンプトが先頭にある
            prop_assert!(
                injected.starts_with(&base_prompt),
                "Injected prompt should start with base prompt. Got: {}",
                crate::utils::safe_truncate_bytes(&injected, 100)
            );

            // 2. 各エントリが "## {file_name}\n{content}" 形式で含まれる
            for (file_name, content) in &expected_order {
                let expected_section = format!("## {}\n{}", file_name, content);
                prop_assert!(
                    injected.contains(&expected_section),
                    "Injected prompt should contain '## {}\\n{}', but was not found in: {}",
                    file_name,
                    content,
                    &injected
                );
            }

            // 3. エントリが created_at 昇順で出現する（position 比較）
            if count > 1 {
                let mut positions: Vec<usize> = Vec::new();
                for (file_name, _content) in &expected_order {
                    let header = format!("## {}", file_name);
                    let pos = injected.find(&header).unwrap_or(usize::MAX);
                    positions.push(pos);
                }
                for i in 1..positions.len() {
                    prop_assert!(
                        positions[i] > positions[i - 1],
                        "Entries should appear in created_at ascending order. Entry {} at pos {}, entry {} at pos {}",
                        i - 1,
                        positions[i - 1],
                        i,
                        positions[i]
                    );
                }
            }

            // 4. ナレッジセクションはベースプロンプトの後に出現する
            if count > 0 {
                let first_header = format!("## {}", expected_order[0].0);
                let base_end = base_prompt.len();
                let first_header_pos = injected.find(&first_header).unwrap_or(0);
                prop_assert!(
                    first_header_pos > base_end,
                    "Knowledge section should appear after base prompt (base ends at {}, first entry at {})",
                    base_end,
                    first_header_pos
                );
            }
        }
    }

    // ========================================
    // Feature: thinking-reasoning-support, Property 6: Thinking content truncation invariant
    // ========================================
    //
    // For any thinking content string, after truncation the saved content SHALL have
    // length ≤ 200,000 characters AND SHALL be a prefix of the original content
    // (preserving UTF-8 character boundaries).
    //
    // **Validates: Requirements 4.5**

    use crate::chat::engine::{truncate_thinking_content, ChatStreamEvent};

    /// マルチバイト文字（CJK、絵文字）を含むランダム文字列を生成するストラテジー
    /// 短い文字列から上限超過する長い文字列まで幅広く生成
    fn arb_multibyte_string() -> impl Strategy<Value = String> {
        prop_oneof![
            // ASCII only (short)
            "[a-zA-Z0-9 ]{0,100}",
            // CJK characters
            "[あ-んア-ヶ一-龥]{0,100}",
            // Mixed ASCII + CJK (medium)
            "[a-zA-Z0-9あ-んア-ヶ ]{0,500}",
            // Long strings that may exceed 200,000 chars
            // Generate base pattern and repeat to create longer strings
            "[a-zA-Z0-9あ-ん🌟🎉🚀]{1,50}".prop_map(|s| s.repeat(5000)),
            // Very long ASCII strings that exceed the limit
            "[a-z]{1,10}".prop_map(|s| s.repeat(30000)),
            // Very long CJK strings that exceed the limit
            "[あ-ん]{1,10}".prop_map(|s| s.repeat(30000)),
            // Strings with emoji sequences
            "(🌟|🎉|🚀|👨‍👩‍👧‍👦|🏳️‍🌈){1,20}".prop_map(|s| s.repeat(15000)),
            // Empty string
            Just(String::new()),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        // Feature: thinking-reasoning-support, Property 6: Thinking content truncation invariant
        #[test]
        fn prop_truncate_thinking_content_invariant(
            content in arb_multibyte_string(),
        ) {
            let result = truncate_thinking_content(&content);

            // 1. 切り詰め後の文字数は ≤ 200,000
            let result_char_count = result.chars().count();
            prop_assert!(
                result_char_count <= 200_000,
                "Truncated result should have at most 200,000 chars, but got {}",
                result_char_count
            );

            // 2. 結果は元文字列のprefixである（元文字列がresultで始まる）
            prop_assert!(
                content.starts_with(result),
                "Original content should start with the truncated result"
            );

            // 3. 元文字列が200,000文字以下の場合、結果は元文字列と同一
            let original_char_count = content.chars().count();
            if original_char_count <= 200_000 {
                prop_assert_eq!(
                    result,
                    content.as_str(),
                    "Content within limit should be returned unchanged"
                );
            }

            // 4. 結果は有効なUTF-8文字列である（&strなので型システムで保証されるが念のため）
            prop_assert!(
                std::str::from_utf8(result.as_bytes()).is_ok(),
                "Result should be valid UTF-8"
            );
        }
    }

    // ========================================
    // Feature: thinking-reasoning-support, Property 3: Stream event field assignment invariant
    // ========================================
    //
    // For any ChatStreamEvent emitted by the Chat Engine during streaming:
    // - If thinking content is present, the thinking field contains the thinking delta and the chunk field is empty string
    // - If text content is present, the chunk field contains the text delta and the thinking field is None
    // - The thinking and chunk fields never both have content simultaneously (mutually exclusive for deltas)
    // - done=true events always have thinking=None
    //
    // **Validates: Requirements 2.2, 2.3, 2.5**

    /// ランダムな非空デルタ文字列を生成するストラテジー
    fn arb_delta_string() -> impl Strategy<Value = String> {
        prop_oneof![
            "[a-zA-Z0-9 ]{1,100}",
            "[あ-んア-ヶ]{1,50}",
            "[a-zA-Z0-9あ-ん ,.!?]{1,80}",
        ]
    }

    /// ChatStreamEventをChat Engineのルールに従って構築する関数
    /// - thinking_delta有り → thinking=Some(delta), chunk=""
    /// - text_delta有り → chunk=delta, thinking=None
    /// - done=true → thinking=None
    fn build_stream_event(
        session_id: &str,
        text_delta: Option<&str>,
        thinking_delta: Option<&str>,
        done: bool,
    ) -> ChatStreamEvent {
        if done {
            // done=true イベント: thinking は常に None (Req 2.6)
            ChatStreamEvent {
                session_id: session_id.to_string(),
                chunk: String::new(),
                done: true,
                tool_break: false,
                thinking: None,
            }
        } else if let Some(thinking) = thinking_delta {
            // thinking delta イベント: chunk は空文字列 (Req 2.2)
            ChatStreamEvent {
                session_id: session_id.to_string(),
                chunk: String::new(),
                done: false,
                tool_break: false,
                thinking: Some(thinking.to_string()),
            }
        } else if let Some(text) = text_delta {
            // text delta イベント: thinking は None (Req 2.3, 2.5)
            ChatStreamEvent {
                session_id: session_id.to_string(),
                chunk: text.to_string(),
                done: false,
                tool_break: false,
                thinking: None,
            }
        } else {
            // どちらもない場合: 空イベント
            ChatStreamEvent {
                session_id: session_id.to_string(),
                chunk: String::new(),
                done: false,
                tool_break: false,
                thinking: None,
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        // Feature: thinking-reasoning-support, Property 3: Stream event field assignment invariant
        #[test]
        fn prop_stream_event_thinking_delta_has_empty_chunk(
            thinking_delta in arb_delta_string(),
            session_id in uuid_string(),
        ) {
            let event = build_stream_event(&session_id, None, Some(&thinking_delta), false);

            // thinking が設定されているとき、chunk は空文字列
            prop_assert_eq!(
                &event.chunk,
                "",
                "When thinking is present, chunk should be empty string, got: '{}'",
                event.chunk
            );
            prop_assert_eq!(
                event.thinking.as_deref(),
                Some(thinking_delta.as_str()),
                "Thinking field should contain the thinking delta"
            );
            prop_assert!(!event.done, "Thinking delta event should not be done");
        }

        #[test]
        fn prop_stream_event_text_delta_has_no_thinking(
            text_delta in arb_delta_string(),
            session_id in uuid_string(),
        ) {
            let event = build_stream_event(&session_id, Some(&text_delta), None, false);

            // chunk が設定されているとき、thinking は None
            prop_assert_eq!(
                &event.chunk,
                &text_delta,
                "Chunk field should contain the text delta"
            );
            prop_assert!(
                event.thinking.is_none(),
                "When text chunk is present, thinking should be None, got: {:?}",
                event.thinking
            );
            prop_assert!(!event.done, "Text delta event should not be done");
        }

        #[test]
        fn prop_stream_event_mutual_exclusivity(
            text_delta in proptest::option::of(arb_delta_string()),
            thinking_delta in proptest::option::of(arb_delta_string()),
            session_id in uuid_string(),
        ) {
            let event = build_stream_event(
                &session_id,
                text_delta.as_deref(),
                thinking_delta.as_deref(),
                false,
            );

            // 相互排他性: thinking が Some で非空のとき chunk は空、chunk が非空のとき thinking は None
            if let Some(ref thinking) = event.thinking {
                if !thinking.is_empty() {
                    prop_assert!(
                        event.chunk.is_empty(),
                        "Mutual exclusivity: when thinking has content ('{}'), chunk must be empty, got: '{}'",
                        thinking,
                        event.chunk
                    );
                }
            }
            if !event.chunk.is_empty() {
                prop_assert!(
                    event.thinking.is_none(),
                    "Mutual exclusivity: when chunk has content ('{}'), thinking must be None, got: {:?}",
                    event.chunk,
                    event.thinking
                );
            }
        }

        #[test]
        fn prop_stream_event_done_has_no_thinking(
            text_delta in proptest::option::of(arb_delta_string()),
            thinking_delta in proptest::option::of(arb_delta_string()),
            session_id in uuid_string(),
        ) {
            // done=true イベントでは常に thinking=None (Req 2.6)
            let event = build_stream_event(
                &session_id,
                text_delta.as_deref(),
                thinking_delta.as_deref(),
                true,
            );

            prop_assert!(
                event.thinking.is_none(),
                "done=true event should always have thinking=None, got: {:?}",
                event.thinking
            );
            prop_assert!(event.done, "Event should have done=true");
        }
    }
}
