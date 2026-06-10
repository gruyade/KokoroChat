//! キャラクター機能のプロパティテスト
//! proptest を使用してCharacter Creator の不変条件を検証する。

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use async_trait::async_trait;
    use proptest::prelude::*;
    use tokio::sync::Mutex;

    use crate::character::creator::{CharacterCreator, DefaultCharacterCreator};
    use crate::commands::character::{
        CharacterExportData, ExportedCharacter, ExportedChatSession, ExportedMemory,
        ExportedMessage, ExportedThought,
    };
    use crate::db::database::Database;
    use crate::db::repositories::{
        character as char_repo, chat as chat_repo, memory as memory_repo, thought as thought_repo,
    };
    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse};
    use crate::models::{
        Character, ChatMessageRecord, ChatRole, ChatSession, Memory, Thought, ToolDefinition,
    };

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
                content: "mock response".to_string(),
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
                content: "mock stream".to_string(),
                thinking: None,
            })
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    // ========================================
    // ヘルパー
    // ========================================

    fn test_llm_config() -> Arc<crate::config::model_config::ModelConfigManager> {
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

    /// 非空文字列を生成するストラテジー
    fn non_empty_string() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9ぁ-んァ-ヶ]{1,30}".prop_map(|s| s)
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

    /// ユニークなUUIDを生成するストラテジー
    fn uuid_string() -> impl Strategy<Value = String> {
        "[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}"
    }

    /// Character のストラテジー
    fn arb_character() -> impl Strategy<Value = Character> {
        (
            uuid_string(),
            non_empty_string(),
            non_empty_string(),
            non_empty_string(),
            proptest::option::of(non_empty_string()),
            iso8601_datetime(),
            iso8601_datetime(),
        )
            .prop_map(
                |(id, name, description, system_prompt, avatar_path, created_at, updated_at)| {
                    Character {
                        id,
                        name,
                        description,
                        system_prompt,
                        avatar_path,
                        tts_config: None,
                        created_at,
                        updated_at,
                    }
                },
            )
    }

    /// ユニークIDを持つキャラクターセットを生成するストラテジー
    fn arb_character_set(max_size: usize) -> impl Strategy<Value = Vec<Character>> {
        proptest::collection::vec(arb_character(), 1..=max_size).prop_map(|mut chars| {
            // IDの重複を排除（ユニークなIDを保証）
            let mut seen_ids = HashSet::new();
            chars.retain(|c| seen_ids.insert(c.id.clone()));
            chars
        })
    }

    // ========================================
    // Property 2: Character listing completeness
    // ========================================
    // **Validates: Requirements 1.4**
    //
    // For any set of created Characters, calling list_characters SHALL return
    // exactly those characters — no more, no fewer.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_character_listing_completeness(
            characters in arb_character_set(8),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
                let llm_client: Arc<dyn LLMClient> = Arc::new(MockLLMClient);
                let config = test_llm_config();
                let creator = DefaultCharacterCreator::new(db, llm_client, config);

                // 全キャラクターを保存
                let mut saved_ids: HashSet<String> = HashSet::new();
                for character in &characters {
                    creator.save_character(character).await.unwrap();
                    saved_ids.insert(character.id.clone());
                }

                // list_characters呼び出し
                let listed = creator.list_characters().await.unwrap();

                // 件数一致
                prop_assert_eq!(
                    listed.len(),
                    characters.len(),
                    "Character count mismatch: expected {}, got {}",
                    characters.len(),
                    listed.len()
                );

                // 全IDが含まれている（no fewer）
                let listed_ids: HashSet<String> = listed.iter().map(|c| c.id.clone()).collect();
                for expected_id in &saved_ids {
                    prop_assert!(
                        listed_ids.contains(expected_id),
                        "Character {} not found in listing",
                        expected_id
                    );
                }

                // 余分なIDが含まれていない（no more）
                for listed_id in &listed_ids {
                    prop_assert!(
                        saved_ids.contains(listed_id),
                        "Unexpected character {} found in listing",
                        listed_id
                    );
                }

                Ok(())
            })?;
        }

        #[test]
        fn prop_character_listing_after_deletion(
            characters in arb_character_set(6),
            delete_ratio in 0.1f64..0.9,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
                let llm_client: Arc<dyn LLMClient> = Arc::new(MockLLMClient);
                let config = test_llm_config();
                let creator = DefaultCharacterCreator::new(db, llm_client, config);

                // 全キャラクターを保存
                for character in &characters {
                    creator.save_character(character).await.unwrap();
                }

                // 一部を削除
                let delete_count = ((characters.len() as f64) * delete_ratio).ceil() as usize;
                let delete_count = delete_count.min(characters.len() - 1).max(1);
                let to_delete: Vec<String> = characters.iter()
                    .take(delete_count)
                    .map(|c| c.id.clone())
                    .collect();

                for id in &to_delete {
                    creator.delete_character(id).await.unwrap();
                }

                // list_characters呼び出し
                let listed = creator.list_characters().await.unwrap();

                // 残存キャラクターのID集合
                let expected_remaining: HashSet<String> = characters.iter()
                    .filter(|c| !to_delete.contains(&c.id))
                    .map(|c| c.id.clone())
                    .collect();

                // 件数一致
                prop_assert_eq!(
                    listed.len(),
                    expected_remaining.len(),
                    "After deletion: expected {} characters, got {}",
                    expected_remaining.len(),
                    listed.len()
                );

                // 残存IDが全て含まれている
                let listed_ids: HashSet<String> = listed.iter().map(|c| c.id.clone()).collect();
                for expected_id in &expected_remaining {
                    prop_assert!(
                        listed_ids.contains(expected_id),
                        "Remaining character {} not found after deletion",
                        expected_id
                    );
                }

                // 削除済みIDが含まれていない
                for deleted_id in &to_delete {
                    prop_assert!(
                        !listed_ids.contains(deleted_id),
                        "Deleted character {} still found in listing",
                        deleted_id
                    );
                }

                Ok(())
            })?;
        }
    }

    // ========================================
    // Property 1: エクスポート/インポート ラウンドトリップ
    // ========================================
    // **Validates: Requirements 1.2, 1.3, 1.4, 1.5, 2.3, 2.4, 2.5, 2.6, 2.9**
    //
    // For any 有効なキャラクターデータ（name, description, system_prompt, tts_config）
    // および関連データ（チャット履歴、思考、記憶）に対して、全オプションを有効にして
    // エクスポートし、その結果をインポートし、再度全オプションを有効にしてエクスポート
    // した場合、2回のエクスポート結果のキャラクター設定・チャット内容・思考内容・
    // 記憶内容は等価である（IDとタイムスタンプを除く）。

    // ─── ラウンドトリップ用ストラテジー ───

    /// ChatRole のストラテジー（DBに保存可能なロールのみ）
    fn arb_chat_role() -> impl Strategy<Value = ChatRole> {
        prop_oneof![
            Just(ChatRole::User),
            Just(ChatRole::Assistant),
            Just(ChatRole::Spontaneous),
            Just(ChatRole::Tool),
        ]
    }

    /// ChatMessageRecord のストラテジー
    fn arb_message(session_id: String) -> impl Strategy<Value = ChatMessageRecord> {
        (
            uuid_string(),
            arb_chat_role(),
            non_empty_string(),
            proptest::option::of(non_empty_string()), // tool_call_id
            iso8601_datetime(),
        )
            .prop_map(move |(id, role, content, tool_call_id, created_at)| {
                ChatMessageRecord {
                    id,
                    session_id: session_id.clone(),
                    role,
                    content,
                    attachments: None,
                    tool_calls: None,
                    tool_call_id,
                    thinking_content: None,
                    created_at,
                }
            })
    }

    /// ChatSession + messages のストラテジー
    fn arb_session_with_messages(
        character_id: String,
    ) -> impl Strategy<Value = (ChatSession, Vec<ChatMessageRecord>)> {
        (
            uuid_string(),
            proptest::option::of(non_empty_string()),
            iso8601_datetime(),
        )
            .prop_flat_map(move |(session_id, title, created_at)| {
                let session = ChatSession {
                    id: session_id.clone(),
                    character_id: character_id.clone(),
                    title,
                    last_message_at: None,
                    last_message_preview: None,
                    created_at,
                };
                let messages_strategy = proptest::collection::vec(arb_message(session_id), 0..=3);
                (Just(session), messages_strategy)
            })
    }

    /// Thought のストラテジー
    fn arb_thought(character_id: String) -> impl Strategy<Value = Thought> {
        (
            uuid_string(),
            non_empty_string(),
            proptest::option::of(non_empty_string()),
            iso8601_datetime(),
        )
            .prop_map(move |(id, content, context, created_at)| Thought {
                id,
                character_id: character_id.clone(),
                content,
                context,
                created_at,
            })
    }

    /// Memory のストラテジー
    fn arb_memory(character_id: String) -> impl Strategy<Value = Memory> {
        (
            uuid_string(),
            non_empty_string(),
            iso8601_datetime(),
            iso8601_datetime(),
        )
            .prop_map(move |(id, content, created_at, updated_at)| Memory {
                id,
                character_id: character_id.clone(),
                content,
                source_session_id: None,
                source_message_from: None,
                source_message_to: None,
                created_at,
                updated_at,
            })
    }

    /// エクスポート/インポートテスト用の全データセット
    #[derive(Debug, Clone)]
    struct ExportTestData {
        character: Character,
        sessions: Vec<(ChatSession, Vec<ChatMessageRecord>)>,
        thoughts: Vec<Thought>,
        memories: Vec<Memory>,
    }

    fn arb_export_test_data() -> impl Strategy<Value = ExportTestData> {
        // まずキャラクターを生成し、そのIDを使って関連データを生成
        arb_character().prop_flat_map(|character| {
            let char_id = character.id.clone();
            let char_id2 = character.id.clone();
            let char_id3 = character.id.clone();

            (
                Just(character),
                proptest::collection::vec(arb_session_with_messages(char_id), 0..=2),
                proptest::collection::vec(arb_thought(char_id2), 0..=3),
                proptest::collection::vec(arb_memory(char_id3), 0..=3),
            )
                .prop_map(|(character, sessions, thoughts, memories)| ExportTestData {
                    character,
                    sessions,
                    thoughts,
                    memories,
                })
        })
    }

    // ─── エクスポート/インポート ヘルパー関数 ───

    /// DBからキャラクターデータをエクスポート（コマンドと同じロジック）
    fn do_export(
        conn: &rusqlite::Connection,
        character_id: &str,
    ) -> Result<CharacterExportData, AppError> {
        let character = char_repo::get_character(conn, character_id)?
            .ok_or_else(|| AppError::NotFound(format!("Character not found: {}", character_id)))?;

        let tts_config_value = character
            .tts_config
            .as_ref()
            .map(|c| serde_json::to_value(c))
            .transpose()?;

        let exported_character = ExportedCharacter {
            name: character.name,
            description: character.description,
            system_prompt: character.system_prompt,
            tts_config: tts_config_value,
        };

        // チャット履歴
        let sessions = chat_repo::list_sessions(conn, character_id)?;
        let mut exported_sessions = Vec::new();
        for session in sessions {
            let messages = chat_repo::get_messages(conn, &session.id)?;
            let exported_messages: Vec<ExportedMessage> = messages
                .into_iter()
                .map(|msg| {
                    let role_str = match msg.role {
                        ChatRole::User => "user",
                        ChatRole::Assistant => "assistant",
                        ChatRole::Spontaneous => "spontaneous",
                        ChatRole::Tool => "tool",
                    };
                    let attachments_value = msg
                        .attachments
                        .map(|a| serde_json::to_value(a))
                        .transpose()?;
                    let tool_calls_value = msg
                        .tool_calls
                        .map(|t| serde_json::to_value(t))
                        .transpose()?;
                    Ok(ExportedMessage {
                        role: role_str.to_string(),
                        content: msg.content,
                        attachments: attachments_value,
                        tool_calls: tool_calls_value,
                        tool_call_id: msg.tool_call_id,
                        created_at: msg.created_at,
                    })
                })
                .collect::<Result<Vec<_>, AppError>>()?;

            exported_sessions.push(ExportedChatSession {
                id: session.id,
                title: session.title,
                created_at: session.created_at,
                messages: exported_messages,
            });
        }

        // 思考
        let thought_list = thought_repo::get_thoughts(conn, character_id, None)?;
        let exported_thoughts: Vec<ExportedThought> = thought_list
            .into_iter()
            .map(|t| ExportedThought {
                content: t.content,
                context: t.context,
                created_at: t.created_at,
            })
            .collect();

        // 記憶
        let memory_list = memory_repo::list_memories(conn, character_id)?;
        let exported_memories: Vec<ExportedMemory> = memory_list
            .into_iter()
            .map(|m| ExportedMemory {
                content: m.content,
                source_session_id: m.source_session_id,
                created_at: m.created_at,
            })
            .collect();

        let exported_at = chrono::Utc::now().to_rfc3339();

        Ok(CharacterExportData {
            version: 1,
            exported_at,
            character: exported_character,
            chat_sessions: Some(exported_sessions),
            thoughts: Some(exported_thoughts),
            memories: Some(exported_memories),
        })
    }

    /// エクスポートデータをDBにインポート（コマンドと同じロジック）
    /// 新規キャラクターIDを返す
    fn do_import(
        conn: &rusqlite::Connection,
        data: &CharacterExportData,
    ) -> Result<String, AppError> {
        let new_character_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let tts_config = data
            .character
            .tts_config
            .as_ref()
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(|e| AppError::Validation(format!("tts_config形式が不正: {}", e)))?;

        let character = Character {
            id: new_character_id.clone(),
            name: data.character.name.clone(),
            description: data.character.description.clone(),
            system_prompt: data.character.system_prompt.clone(),
            avatar_path: None,
            tts_config,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        conn.execute_batch("BEGIN")
            .map_err(|e| AppError::Database(format!("Transaction begin failed: {}", e)))?;

        let result = (|| -> Result<(), AppError> {
            char_repo::insert_character(conn, &character)?;

            // チャット履歴インポート
            if let Some(ref sessions) = data.chat_sessions {
                for session_data in sessions {
                    let new_session_id = uuid::Uuid::new_v4().to_string();
                    let session = ChatSession {
                        id: new_session_id.clone(),
                        character_id: new_character_id.clone(),
                        title: session_data.title.clone(),
                        last_message_at: None,
                        last_message_preview: None,
                        created_at: session_data.created_at.clone(),
                    };
                    chat_repo::insert_session(conn, &session)?;

                    for msg_data in &session_data.messages {
                        let new_msg_id = uuid::Uuid::new_v4().to_string();
                        let role = match msg_data.role.as_str() {
                            "user" => ChatRole::User,
                            "assistant" => ChatRole::Assistant,
                            "spontaneous" => ChatRole::Spontaneous,
                            "tool" => ChatRole::Tool,
                            _ => ChatRole::User,
                        };
                        let attachments = msg_data
                            .attachments
                            .as_ref()
                            .map(|v| serde_json::from_value(v.clone()))
                            .transpose()
                            .map_err(|e| {
                                AppError::Validation(format!("attachments形式が不正: {}", e))
                            })?;
                        let tool_calls = msg_data
                            .tool_calls
                            .as_ref()
                            .map(|v| serde_json::from_value(v.clone()))
                            .transpose()
                            .map_err(|e| {
                                AppError::Validation(format!("tool_calls形式が不正: {}", e))
                            })?;
                        let message = ChatMessageRecord {
                            id: new_msg_id,
                            session_id: new_session_id.clone(),
                            role,
                            content: msg_data.content.clone(),
                            attachments,
                            tool_calls,
                            tool_call_id: msg_data.tool_call_id.clone(),
                            thinking_content: None,
                            created_at: msg_data.created_at.clone(),
                        };
                        chat_repo::insert_message(conn, &message)?;
                    }
                }
            }

            // 思考インポート
            if let Some(ref thoughts) = data.thoughts {
                for thought_data in thoughts {
                    let new_thought_id = uuid::Uuid::new_v4().to_string();
                    let thought = Thought {
                        id: new_thought_id,
                        character_id: new_character_id.clone(),
                        content: thought_data.content.clone(),
                        context: thought_data.context.clone(),
                        created_at: thought_data.created_at.clone(),
                    };
                    thought_repo::insert_thought(conn, &thought)?;
                }
            }

            // 記憶インポート
            if let Some(ref memories) = data.memories {
                for memory_data in memories {
                    let new_memory_id = uuid::Uuid::new_v4().to_string();
                    let memory = Memory {
                        id: new_memory_id,
                        character_id: new_character_id.clone(),
                        content: memory_data.content.clone(),
                        source_session_id: None,
                        source_message_from: None,
                        source_message_to: None,
                        created_at: memory_data.created_at.clone(),
                        updated_at: now.clone(),
                    };
                    memory_repo::insert_memory(conn, &memory)?;
                }
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT")
                    .map_err(|e| AppError::Database(format!("Commit failed: {}", e)))?;
                Ok(new_character_id)
            }
            Err(e) => {
                conn.execute_batch("ROLLBACK").ok();
                Err(e)
            }
        }
    }

    /// エクスポートデータの内容比較（ID・タイムスタンプ除外）
    /// キャラクター設定、チャット内容、思考内容、記憶内容が等価であることを検証
    fn assert_exports_equivalent(
        first: &CharacterExportData,
        second: &CharacterExportData,
    ) -> Result<(), TestCaseError> {
        // キャラクター設定の比較
        prop_assert_eq!(
            &first.character.name,
            &second.character.name,
            "Character name mismatch"
        );
        prop_assert_eq!(
            &first.character.description,
            &second.character.description,
            "Character description mismatch"
        );
        prop_assert_eq!(
            &first.character.system_prompt,
            &second.character.system_prompt,
            "Character system_prompt mismatch"
        );
        prop_assert_eq!(
            &first.character.tts_config,
            &second.character.tts_config,
            "Character tts_config mismatch"
        );

        // チャットセッション比較（セッション数、各セッションのtitle・メッセージ内容）
        let first_sessions = first.chat_sessions.as_ref().unwrap_or(&Vec::new()).clone();
        let second_sessions = second.chat_sessions.as_ref().unwrap_or(&Vec::new()).clone();
        prop_assert_eq!(
            first_sessions.len(),
            second_sessions.len(),
            "Chat session count mismatch: {} vs {}",
            first_sessions.len(),
            second_sessions.len()
        );

        // セッションをcreated_at + titleでソートして比較（IDは異なるため）
        let mut first_sorted: Vec<_> = first_sessions.iter().collect();
        let mut second_sorted: Vec<_> = second_sessions.iter().collect();
        first_sorted.sort_by(|a, b| (&a.created_at, &a.title).cmp(&(&b.created_at, &b.title)));
        second_sorted.sort_by(|a, b| (&a.created_at, &a.title).cmp(&(&b.created_at, &b.title)));

        for (fs, ss) in first_sorted.iter().zip(second_sorted.iter()) {
            prop_assert_eq!(&fs.title, &ss.title, "Session title mismatch");
            prop_assert_eq!(
                &fs.created_at,
                &ss.created_at,
                "Session created_at mismatch"
            );
            prop_assert_eq!(
                fs.messages.len(),
                ss.messages.len(),
                "Message count mismatch in session"
            );

            for (fm, sm) in fs.messages.iter().zip(ss.messages.iter()) {
                prop_assert_eq!(&fm.role, &sm.role, "Message role mismatch");
                prop_assert_eq!(&fm.content, &sm.content, "Message content mismatch");
                prop_assert_eq!(
                    &fm.attachments,
                    &sm.attachments,
                    "Message attachments mismatch"
                );
                prop_assert_eq!(
                    &fm.tool_calls,
                    &sm.tool_calls,
                    "Message tool_calls mismatch"
                );
                prop_assert_eq!(
                    &fm.tool_call_id,
                    &sm.tool_call_id,
                    "Message tool_call_id mismatch"
                );
                prop_assert_eq!(
                    &fm.created_at,
                    &sm.created_at,
                    "Message created_at mismatch"
                );
            }
        }

        // 思考比較（content + context + created_at）
        let first_thoughts = first.thoughts.as_ref().unwrap_or(&Vec::new()).clone();
        let second_thoughts = second.thoughts.as_ref().unwrap_or(&Vec::new()).clone();
        prop_assert_eq!(
            first_thoughts.len(),
            second_thoughts.len(),
            "Thought count mismatch: {} vs {}",
            first_thoughts.len(),
            second_thoughts.len()
        );

        let mut first_thoughts_sorted: Vec<_> = first_thoughts.iter().collect();
        let mut second_thoughts_sorted: Vec<_> = second_thoughts.iter().collect();
        first_thoughts_sorted
            .sort_by(|a, b| (&a.created_at, &a.content).cmp(&(&b.created_at, &b.content)));
        second_thoughts_sorted
            .sort_by(|a, b| (&a.created_at, &a.content).cmp(&(&b.created_at, &b.content)));

        for (ft, st) in first_thoughts_sorted
            .iter()
            .zip(second_thoughts_sorted.iter())
        {
            prop_assert_eq!(&ft.content, &st.content, "Thought content mismatch");
            prop_assert_eq!(&ft.context, &st.context, "Thought context mismatch");
            prop_assert_eq!(
                &ft.created_at,
                &st.created_at,
                "Thought created_at mismatch"
            );
        }

        // 記憶比較（content + created_at）
        // Note: source_session_idはインポート時にNoneになるため、
        // 2回目のエクスポートでもNoneになる。1回目もインポート後の再エクスポートなので
        // 比較対象は2回目のエクスポート同士。
        let first_memories = first.memories.as_ref().unwrap_or(&Vec::new()).clone();
        let second_memories = second.memories.as_ref().unwrap_or(&Vec::new()).clone();
        prop_assert_eq!(
            first_memories.len(),
            second_memories.len(),
            "Memory count mismatch: {} vs {}",
            first_memories.len(),
            second_memories.len()
        );

        let mut first_memories_sorted: Vec<_> = first_memories.iter().collect();
        let mut second_memories_sorted: Vec<_> = second_memories.iter().collect();
        first_memories_sorted
            .sort_by(|a, b| (&a.created_at, &a.content).cmp(&(&b.created_at, &b.content)));
        second_memories_sorted
            .sort_by(|a, b| (&a.created_at, &a.content).cmp(&(&b.created_at, &b.content)));

        for (fm, sm) in first_memories_sorted
            .iter()
            .zip(second_memories_sorted.iter())
        {
            prop_assert_eq!(&fm.content, &sm.content, "Memory content mismatch");
            prop_assert_eq!(&fm.created_at, &sm.created_at, "Memory created_at mismatch");
        }

        Ok(())
    }

    // ========================================
    // Property 2: 不正フォーマット拒否 バリデーション
    // ========================================

    /// インポートデータのバリデーション（import_characterコマンドと同じロジック）
    /// エラー時はAppErrorを返す
    fn validate_import_data(data: &CharacterExportData) -> Result<(), AppError> {
        if data.version != 1 {
            return Err(AppError::Validation(format!(
                "未対応のエクスポート形式（version: {}）",
                data.version
            )));
        }
        if data.character.name.trim().is_empty() {
            return Err(AppError::Validation(
                "必須データが不足: character.name".to_string(),
            ));
        }
        if data.character.description.trim().is_empty() {
            return Err(AppError::Validation(
                "必須データが不足: character.description".to_string(),
            ));
        }
        if data.character.system_prompt.trim().is_empty() {
            return Err(AppError::Validation(
                "必須データが不足: character.system_prompt".to_string(),
            ));
        }
        Ok(())
    }

    /// バリデーション付きインポート（import_characterコマンドと同等）
    fn do_import_with_validation(
        conn: &rusqlite::Connection,
        data: &CharacterExportData,
    ) -> Result<String, AppError> {
        validate_import_data(data)?;
        do_import(conn, data)
    }

    /// DB内のキャラクター数を取得
    fn count_characters(conn: &rusqlite::Connection) -> usize {
        char_repo::list_characters(conn).unwrap().len()
    }

    // ─── 不正データ生成ストラテジー ───

    /// 不正なバージョン番号（0 or 2以上）を生成
    fn invalid_version() -> impl Strategy<Value = u32> {
        prop_oneof![Just(0u32), 2u32..100,]
    }

    /// 空白のみ or 空文字列を生成（trimすると空になる文字列）
    fn empty_or_whitespace_string() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(String::new()),
            Just(" ".to_string()),
            Just("  ".to_string()),
            Just("\t".to_string()),
            Just(" \t \n ".to_string()),
        ]
    }

    /// 不正なCharacterExportDataを生成するストラテジー
    /// 4つの不正パターンのいずれかを選択:
    /// 1. version != 1
    /// 2. character.name が空
    /// 3. character.description が空
    /// 4. character.system_prompt が空
    fn arb_invalid_export_data() -> impl Strategy<Value = CharacterExportData> {
        // 不正パターンの種類を選択（0..4）
        (
            0u8..4,
            non_empty_string(),
            non_empty_string(),
            non_empty_string(),
            empty_or_whitespace_string(),
            invalid_version(),
            iso8601_datetime(),
        )
            .prop_map(
                |(
                    pattern,
                    valid_name,
                    valid_desc,
                    valid_prompt,
                    empty_str,
                    bad_version,
                    exported_at,
                )| {
                    let (version, name, description, system_prompt) = match pattern {
                        0 => (bad_version, valid_name, valid_desc, valid_prompt),
                        1 => (1u32, empty_str.clone(), valid_desc, valid_prompt),
                        2 => (1u32, valid_name, empty_str.clone(), valid_prompt),
                        3 => (1u32, valid_name, valid_desc, empty_str.clone()),
                        _ => unreachable!(),
                    };

                    CharacterExportData {
                        version,
                        exported_at,
                        character: ExportedCharacter {
                            name,
                            description,
                            system_prompt,
                            tts_config: None,
                        },
                        chat_sessions: None,
                        thoughts: None,
                        memories: None,
                    }
                },
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Feature: app-enhancements-v2, Property 1: エクスポート/インポート ラウンドトリップ
        ///
        /// For any 有効なキャラクターデータに対して、export → import → re-export した場合、
        /// 2回のエクスポート結果は等価（IDとタイムスタンプを除く）。
        #[test]
        fn prop_export_import_roundtrip(
            test_data in arb_export_test_data(),
        ) {
            let db = Database::open_in_memory().unwrap();
            let conn = db.connection();

            // 1. テストデータをDBに挿入
            char_repo::insert_character(conn, &test_data.character).unwrap();

            for (session, messages) in &test_data.sessions {
                chat_repo::insert_session(conn, session).unwrap();
                for msg in messages {
                    chat_repo::insert_message(conn, msg).unwrap();
                }
            }

            for thought in &test_data.thoughts {
                thought_repo::insert_thought(conn, thought).unwrap();
            }

            for memory in &test_data.memories {
                memory_repo::insert_memory(conn, memory).unwrap();
            }

            // 2. 最初のエクスポート
            let first_export = do_export(conn, &test_data.character.id).unwrap();

            // 3. エクスポートデータをインポート（新規キャラクターとして）
            let imported_character_id = do_import(conn, &first_export).unwrap();

            // 4. インポートしたキャラクターを再エクスポート
            let second_export = do_export(conn, &imported_character_id).unwrap();

            // 5. 2回のエクスポート結果を比較（ID・タイムスタンプ除外）
            assert_exports_equivalent(&first_export, &second_export)?;
        }

        /// Feature: app-enhancements-v2, Property 2: 不正フォーマット拒否
        ///
        /// **Validates: Requirements 2.8**
        ///
        /// For any 必須フィールド（version, character.name, character.description,
        /// character.system_prompt）のいずれかが欠落または不正な型を持つJSONデータに対して、
        /// インポート処理はエラーを返し、データベースに変更を加えない。
        #[test]
        fn prop_invalid_format_rejected(
            invalid_data in arb_invalid_export_data(),
        ) {
            let db = Database::open_in_memory().unwrap();
            let conn = db.connection();

            // インポート前のキャラクター数を記録
            let count_before = count_characters(conn);

            // 不正データでインポートを試行
            let result = do_import_with_validation(conn, &invalid_data);

            // エラーが返されることを確認
            prop_assert!(
                result.is_err(),
                "Invalid data should be rejected but was accepted: version={}, name='{}', desc='{}', prompt='{}'",
                invalid_data.version,
                invalid_data.character.name,
                invalid_data.character.description,
                invalid_data.character.system_prompt,
            );

            // DB変更なし確認（キャラクター数が変わっていない）
            let count_after = count_characters(conn);
            prop_assert_eq!(
                count_before,
                count_after,
                "DB should not be modified after rejecting invalid data"
            );
        }
    }
}
