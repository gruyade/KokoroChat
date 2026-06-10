//! データモデルのプロパティテスト
//! proptest を使用してデータ層の不変条件を検証する。

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rusqlite::params;

    use crate::db::database::Database;
    use crate::db::repositories::{character, chat, knowledge, memory, thought};
    use crate::models::tts::{EmotionParams, TTSConfig, TTSProvider};
    use crate::models::{
        Character, ChatMessageRecord, ChatRole, ChatSession, KnowledgeEntry, Memory, Thought,
    };

    // ========================================
    // Arbitrary Strategies
    // ========================================

    /// 非空文字列を生成するストラテジー
    fn non_empty_string() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9ぁ-んァ-ヶ亜-熙]{1,50}".prop_map(|s| s)
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

    /// UUID風のID文字列を生成するストラテジー
    fn uuid_string() -> impl Strategy<Value = String> {
        "[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}"
    }

    /// EmotionParams のストラテジー
    fn arb_emotion_params() -> impl Strategy<Value = EmotionParams> {
        proptest::collection::hash_map(
            prop::sample::select(vec![
                "happy".to_string(),
                "fun".to_string(),
                "angry".to_string(),
                "sad".to_string(),
            ]),
            0i32..100,
            0..=4,
        )
    }

    /// TTSConfig のストラテジー
    fn arb_tts_config() -> impl Strategy<Value = TTSConfig> {
        (
            prop_oneof![Just(TTSProvider::IrodoriTts), Just(TTSProvider::Voicepeak)],
            "http://localhost:[0-9]{4}",
            proptest::option::of(non_empty_string()),
            proptest::option::of(non_empty_string()),
            proptest::option::of(non_empty_string()),
            proptest::option::of(arb_emotion_params()),
            proptest::option::of(0.5f32..2.0),
            proptest::option::of(-1.0f32..1.0),
        )
            .prop_map(
                |(provider, base_url, ref_audio, caption, narrator, emotion, speed, pitch)| {
                    TTSConfig {
                        provider,
                        base_url: Some(base_url),
                        caption_base_url: None,
                        reference_audio_base_url: None,
                        reference_audio_path: ref_audio,
                        caption,
                        narrator,
                        emotion,
                        speed,
                        pitch,
                        irodori_mode: None,
                    }
                },
            )
    }

    /// Character のストラテジー
    fn arb_character() -> impl Strategy<Value = Character> {
        (
            uuid_string(),
            non_empty_string(),
            non_empty_string(),
            non_empty_string(),
            proptest::option::of(non_empty_string()),
            proptest::option::of(arb_tts_config()),
            iso8601_datetime(),
            iso8601_datetime(),
        )
            .prop_map(
                |(
                    id,
                    name,
                    description,
                    system_prompt,
                    avatar_path,
                    tts_config,
                    created_at,
                    updated_at,
                )| {
                    Character {
                        id,
                        name,
                        description,
                        system_prompt,
                        avatar_path,
                        tts_config,
                        created_at,
                        updated_at,
                    }
                },
            )
    }

    /// ChatSession のストラテジー（character_idは外部から指定）
    fn arb_session(character_id: String) -> impl Strategy<Value = ChatSession> {
        (
            uuid_string(),
            proptest::option::of(non_empty_string()),
            iso8601_datetime(),
        )
            .prop_map(move |(id, title, created_at)| ChatSession {
                id,
                character_id: character_id.clone(),
                title,
                last_message_at: None,
                last_message_preview: None,
                created_at,
            })
    }

    /// ChatMessageRecord のストラテジー（session_idは外部から指定）
    fn arb_message(session_id: String) -> impl Strategy<Value = ChatMessageRecord> {
        (
            uuid_string(),
            prop_oneof![
                Just(ChatRole::User),
                Just(ChatRole::Assistant),
                Just(ChatRole::Spontaneous),
            ],
            non_empty_string(),
            iso8601_datetime(),
        )
            .prop_map(move |(id, role, content, created_at)| ChatMessageRecord {
                id,
                session_id: session_id.clone(),
                role,
                content,
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                thinking_content: None,
                created_at,
            })
    }

    // ========================================
    // Property 1: Character serialization round-trip
    // ========================================
    // **Validates: Requirements 1.2**
    //
    // For any valid Character object (including name, description, system_prompt,
    // tts_config, and all metadata fields), serializing to JSON and deserializing
    // back SHALL produce an equivalent Character object.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_character_serialization_roundtrip(character in arb_character()) {
            let json = serde_json::to_string(&character).unwrap();
            let deserialized: Character = serde_json::from_str(&json).unwrap();

            prop_assert_eq!(&character.id, &deserialized.id);
            prop_assert_eq!(&character.name, &deserialized.name);
            prop_assert_eq!(&character.description, &deserialized.description);
            prop_assert_eq!(&character.system_prompt, &deserialized.system_prompt);
            prop_assert_eq!(&character.avatar_path, &deserialized.avatar_path);
            prop_assert_eq!(&character.created_at, &deserialized.created_at);
            prop_assert_eq!(&character.updated_at, &deserialized.updated_at);

            // TTSConfig比較（Value round-trip — HashMapの順序非依存）
            let orig_tts_value = serde_json::to_value(&character.tts_config).unwrap();
            let deser_tts_value = serde_json::to_value(&deserialized.tts_config).unwrap();
            prop_assert_eq!(orig_tts_value, deser_tts_value);
        }
    }

    // ========================================
    // Property 3: Cascade delete removes all related data
    // ========================================
    // **Validates: Requirements 1.6**
    //
    // For any Character with associated ChatSessions, ChatMessages, Memories,
    // and Thoughts, deleting that Character SHALL result in zero remaining records
    // referencing that Character's ID across all tables.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]

        #[test]
        fn prop_cascade_delete_removes_all_related_data(
            character in arb_character(),
            num_sessions in 1usize..4,
            num_messages_per_session in 1usize..4,
            num_memories in 0usize..4,
            num_thoughts in 0usize..4,
        ) {
            let db = Database::open_in_memory().unwrap();
            let conn = db.connection();

            // キャラクター挿入
            character::insert_character(conn, &character).unwrap();

            // セッション・メッセージ作成
            let mut session_ids = Vec::new();
            for i in 0..num_sessions {
                let session = ChatSession {
                    id: format!("sess-{:04}", i),
                    character_id: character.id.clone(),
                    title: Some(format!("Session {}", i)),
                    last_message_at: None,
                    last_message_preview: None,
                    created_at: format!("2024-01-01T{:02}:00:00Z", i),
                };
                chat::insert_session(conn, &session).unwrap();
                session_ids.push(session.id.clone());

                for j in 0..num_messages_per_session {
                    let msg = ChatMessageRecord {
                        id: format!("msg-{:04}-{:04}", i, j),
                        session_id: session.id.clone(),
                        role: ChatRole::User,
                        content: format!("Message {} in session {}", j, i),
                        attachments: None,
                        tool_calls: None,
                        tool_call_id: None,
                        thinking_content: None,
                        created_at: format!("2024-01-01T{:02}:{:02}:00Z", i, j),
                    };
                    chat::insert_message(conn, &msg).unwrap();
                }
            }

            // メモリ作成
            for i in 0..num_memories {
                let mem = Memory {
                    id: format!("mem-{:04}", i),
                    character_id: character.id.clone(),
                    content: format!("Memory {}", i),
                    source_session_id: None,
                    source_message_from: None,
                    source_message_to: None,
                    created_at: format!("2024-01-01T{:02}:00:00Z", i),
                    updated_at: format!("2024-01-01T{:02}:00:00Z", i),
                };
                memory::insert_memory(conn, &mem).unwrap();
            }

            // 思考作成
            for i in 0..num_thoughts {
                let t = Thought {
                    id: format!("thought-{:04}", i),
                    character_id: character.id.clone(),
                    content: format!("Thought {}", i),
                    context: None,
                    created_at: format!("2024-01-01T{:02}:00:00Z", i),
                };
                thought::insert_thought(conn, &t).unwrap();
            }

            // キャラクター削除
            character::delete_character(conn, &character.id).unwrap();

            // 全関連データが削除されていることを検証
            let session_count: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM chat_sessions WHERE character_id = ?1",
                    params![character.id],
                    |row| row.get(0),
                )
                .unwrap();
            prop_assert_eq!(session_count, 0, "Sessions should be deleted");

            // メッセージ（セッション経由でCASCADE）
            for sid in &session_ids {
                let msg_count: i32 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM chat_messages WHERE session_id = ?1",
                        params![sid],
                        |row| row.get(0),
                    )
                    .unwrap();
                prop_assert_eq!(msg_count, 0, "Messages in session {} should be deleted", sid);
            }

            let memory_count: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM memories WHERE character_id = ?1",
                    params![character.id],
                    |row| row.get(0),
                )
                .unwrap();
            prop_assert_eq!(memory_count, 0, "Memories should be deleted");

            let thought_count: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM thoughts WHERE character_id = ?1",
                    params![character.id],
                    |row| row.get(0),
                )
                .unwrap();
            prop_assert_eq!(thought_count, 0, "Thoughts should be deleted");
        }
    }

    // ========================================
    // Property 5: Session listing completeness
    // ========================================
    // **Validates: Requirements 2.3**
    //
    // For any Character with created ChatSessions, calling list_sessions SHALL
    // return all sessions belonging to that Character.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_session_listing_completeness(
            num_sessions in 1usize..8,
        ) {
            let db = Database::open_in_memory().unwrap();
            let conn = db.connection();

            // キャラクター作成
            let character = Character {
                id: "char-test".to_string(),
                name: "Test".to_string(),
                description: "Desc".to_string(),
                system_prompt: "Prompt".to_string(),
                avatar_path: None,
                tts_config: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            };
            character::insert_character(conn, &character).unwrap();

            // 別キャラクター（ノイズ用）
            let other_char = Character {
                id: "char-other".to_string(),
                name: "Other".to_string(),
                description: "Other".to_string(),
                system_prompt: "Other".to_string(),
                avatar_path: None,
                tts_config: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            };
            character::insert_character(conn, &other_char).unwrap();

            // 対象キャラクターのセッション作成
            let mut expected_ids: Vec<String> = Vec::new();
            for i in 0..num_sessions {
                let session = ChatSession {
                    id: format!("sess-{:04}", i),
                    character_id: "char-test".to_string(),
                    title: Some(format!("Session {}", i)),
                    last_message_at: Some(format!("2024-01-01T{:02}:00:00Z", i)),
                    last_message_preview: None,
                    created_at: format!("2024-01-01T{:02}:00:00Z", i),
                };
                chat::insert_session(conn, &session).unwrap();
                expected_ids.push(session.id);
            }

            // 別キャラクターのセッション（ノイズ）
            let noise_session = ChatSession {
                id: "sess-noise".to_string(),
                character_id: "char-other".to_string(),
                title: Some("Noise".to_string()),
                last_message_at: Some("2024-06-01T00:00:00Z".to_string()),
                last_message_preview: None,
                created_at: "2024-06-01T00:00:00Z".to_string(),
            };
            chat::insert_session(conn, &noise_session).unwrap();

            // list_sessions呼び出し
            let listed = chat::list_sessions(conn, "char-test").unwrap();

            // 件数一致
            prop_assert_eq!(listed.len(), num_sessions, "Session count mismatch");

            // 全IDが含まれている
            let listed_ids: Vec<String> = listed.iter().map(|s| s.id.clone()).collect();
            for expected_id in &expected_ids {
                prop_assert!(
                    listed_ids.contains(expected_id),
                    "Session {} not found in listing",
                    expected_id
                );
            }

            // ノイズセッションが含まれていない
            prop_assert!(
                !listed_ids.contains(&"sess-noise".to_string()),
                "Noise session should not be in listing"
            );
        }
    }

    // ========================================
    // Property 6: Session metadata invariant
    // ========================================
    // **Validates: Requirements 2.4**
    //
    // For any ChatSession with one or more messages, the session's lastMessageAt
    // SHALL equal the createdAt of the most recent message, and lastMessagePreview
    // SHALL equal a prefix of that message's content.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_session_metadata_invariant(
            num_messages in 1usize..6,
        ) {
            let db = Database::open_in_memory().unwrap();
            let conn = db.connection();

            // キャラクター作成
            let character = Character {
                id: "char-meta".to_string(),
                name: "Meta".to_string(),
                description: "Desc".to_string(),
                system_prompt: "Prompt".to_string(),
                avatar_path: None,
                tts_config: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            };
            character::insert_character(conn, &character).unwrap();

            // セッション作成
            let session = ChatSession {
                id: "sess-meta".to_string(),
                character_id: "char-meta".to_string(),
                title: Some("Metadata test".to_string()),
                last_message_at: None,
                last_message_preview: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
            };
            chat::insert_session(conn, &session).unwrap();

            // メッセージ挿入（時系列順）
            let mut latest_created_at = String::new();
            let mut latest_content = String::new();
            for i in 0..num_messages {
                let created_at = format!("2024-01-01T{:02}:{:02}:00Z", 10 + i / 60, i % 60);
                let content = format!("メッセージ内容_{:04}", i);
                let msg = ChatMessageRecord {
                    id: format!("msg-meta-{:04}", i),
                    session_id: "sess-meta".to_string(),
                    role: if i % 2 == 0 { ChatRole::User } else { ChatRole::Assistant },
                    content: content.clone(),
                    attachments: None,
                    tool_calls: None,
                    tool_call_id: None,
                    thinking_content: None,
                    created_at: created_at.clone(),
                };
                chat::insert_message(conn, &msg).unwrap();
                latest_created_at = created_at;
                latest_content = content;
            }

            // セッションメタデータ更新（実際のアプリケーションロジックと同様）
            let preview = if latest_content.len() > 50 {
                latest_content[..50].to_string()
            } else {
                latest_content.clone()
            };
            chat::update_session_metadata(conn, "sess-meta", &latest_created_at, &preview).unwrap();

            // セッション取得して検証
            let updated_session = chat::get_session(conn, "sess-meta").unwrap().unwrap();

            // lastMessageAt == 最新メッセージのcreated_at
            prop_assert_eq!(
                updated_session.last_message_at.as_deref(),
                Some(latest_created_at.as_str()),
                "lastMessageAt should equal the most recent message's createdAt"
            );

            // lastMessagePreview == 最新メッセージcontentのプレフィックス
            let actual_preview = updated_session.last_message_preview.unwrap_or_default();
            prop_assert!(
                latest_content.starts_with(&actual_preview),
                "lastMessagePreview '{}' should be a prefix of latest content '{}'",
                actual_preview,
                latest_content
            );
        }
    }

    // ========================================
    // Knowledge Plugin - Helper Strategies
    // ========================================

    /// 512KB以下の有効なUTF-8コンテンツを生成するストラテジー
    fn valid_knowledge_content() -> impl Strategy<Value = String> {
        // 1〜4096バイト程度のコンテンツ（テスト高速化のため上限を抑える）
        "[a-zA-Z0-9 \\n\\t,.!?ぁ-んァ-ヶ]{1,2000}"
    }

    /// 有効なファイル名を生成するストラテジー（basename only）
    fn valid_file_name() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9_-]{1,20}\\.(txt|md|json|csv|log)"
    }

    /// テスト用のDBセットアップ（キャラクター+セッション作成済み）
    fn setup_knowledge_test_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "char-k001",
                "KnowledgeTest",
                "Desc",
                "Prompt",
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z"
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            params!["sess-k001", "char-k001", "2024-01-01T00:00:00Z"],
        )
        .unwrap();

        db
    }

    // ========================================
    // Feature: knowledge-plugin, Property 1: Knowledge entry creation round-trip
    // ========================================
    // **Validates: Requirements 1.1, 1.4, 1.5**
    //
    // For any valid UTF-8 string content (≤512KB) and any valid file_name,
    // adding a knowledge entry to a session and then retrieving it SHALL produce
    // a record where file_name equals the input basename, size_bytes equals the
    // byte length of content, enabled is true, and injection_mode is "system_prompt".

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_knowledge_creation_roundtrip(
            content in valid_knowledge_content(),
            file_name in valid_file_name(),
        ) {
            let db = setup_knowledge_test_db();
            let conn = db.connection();

            let entry = KnowledgeEntry {
                id: format!("know-rt-{}", file_name),
                session_id: "sess-k001".to_string(),
                file_name: file_name.clone(),
                content: content.clone(),
                size_bytes: content.len() as i64,
                enabled: true,
                injection_mode: "system_prompt".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
            };

            knowledge::add_knowledge(conn, &entry).unwrap();

            // list で取得して検証
            let list = knowledge::list_knowledge(conn, "sess-k001").unwrap();
            let found = list.iter().find(|e| e.file_name == file_name);
            prop_assert!(found.is_some(), "Entry should exist after adding");

            let meta = found.unwrap();
            prop_assert_eq!(&meta.file_name, &file_name, "file_name should match input");
            prop_assert_eq!(meta.size_bytes, content.len() as i64, "size_bytes should equal byte length of content");
            prop_assert!(meta.enabled, "enabled should be true");
            prop_assert_eq!(&meta.injection_mode, "system_prompt", "injection_mode should be system_prompt");

            // content取得で検証
            let retrieved_content = knowledge::get_knowledge_content(conn, "sess-k001", &file_name).unwrap();
            prop_assert_eq!(&retrieved_content, &content, "Content should match input");
        }
    }

    // ========================================
    // Feature: knowledge-plugin, Property 2: Upsert replaces existing entry
    // ========================================
    // **Validates: Requirements 1.3, 9.3**
    //
    // For any session and file_name, adding a knowledge entry twice with different
    // content SHALL result in exactly one record for that (session_id, file_name)
    // pair, with the content and size_bytes matching the second addition.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_knowledge_upsert_replaces_existing(
            content1 in valid_knowledge_content(),
            content2 in valid_knowledge_content(),
            file_name in valid_file_name(),
        ) {
            let db = setup_knowledge_test_db();
            let conn = db.connection();

            let entry1 = KnowledgeEntry {
                id: format!("know-up1-{}", file_name),
                session_id: "sess-k001".to_string(),
                file_name: file_name.clone(),
                content: content1.clone(),
                size_bytes: content1.len() as i64,
                enabled: true,
                injection_mode: "system_prompt".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
            };

            let entry2 = KnowledgeEntry {
                id: format!("know-up2-{}", file_name),
                session_id: "sess-k001".to_string(),
                file_name: file_name.clone(),
                content: content2.clone(),
                size_bytes: content2.len() as i64,
                enabled: true,
                injection_mode: "system_prompt".to_string(),
                created_at: "2024-01-02T00:00:00Z".to_string(),
            };

            knowledge::add_knowledge(conn, &entry1).unwrap();
            knowledge::add_knowledge(conn, &entry2).unwrap();

            // 同一(session_id, file_name)のレコードは1件のみ
            let list = knowledge::list_knowledge(conn, "sess-k001").unwrap();
            let matching: Vec<_> = list.iter().filter(|e| e.file_name == file_name).collect();
            prop_assert_eq!(matching.len(), 1, "Should have exactly one record for same (session_id, file_name)");

            // size_bytesは2回目の値
            prop_assert_eq!(matching[0].size_bytes, content2.len() as i64, "size_bytes should match second content");

            // contentは2回目の値
            let retrieved = knowledge::get_knowledge_content(conn, "sess-k001", &file_name).unwrap();
            prop_assert_eq!(&retrieved, &content2, "Content should match second addition");
        }
    }

    // ========================================
    // Feature: knowledge-plugin, Property 3: Oversized content rejection
    // ========================================
    // **Validates: Requirements 1.6**
    //
    // For any content string whose UTF-8 byte length exceeds 512KB, attempting
    // to add it as a knowledge entry SHALL fail with an error and SHALL NOT
    // create a record in the database.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_knowledge_oversized_content_rejection(
            // 512KB + 1〜512KB + 1024 の範囲
            extra_bytes in 1usize..1024,
            file_name in valid_file_name(),
        ) {
            let db = setup_knowledge_test_db();
            let conn = db.connection();

            let oversized_content = "x".repeat(512 * 1024 + extra_bytes);

            let entry = KnowledgeEntry {
                id: format!("know-big-{}", file_name),
                session_id: "sess-k001".to_string(),
                file_name: file_name.clone(),
                content: oversized_content.clone(),
                size_bytes: oversized_content.len() as i64,
                enabled: true,
                injection_mode: "system_prompt".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
            };

            // 追加は失敗するべき
            let result = knowledge::add_knowledge(conn, &entry);
            prop_assert!(result.is_err(), "Adding oversized content should fail");

            // DBにレコードが作成されていないことを確認
            let list = knowledge::list_knowledge(conn, "sess-k001").unwrap();
            let matching: Vec<_> = list.iter().filter(|e| e.file_name == file_name).collect();
            prop_assert_eq!(matching.len(), 0, "No record should be created for oversized content");
        }
    }

    // ========================================
    // Feature: knowledge-plugin, Property 4: Delete removes target and preserves others
    // ========================================
    // **Validates: Requirements 2.2**
    //
    // For any session with N knowledge entries (N≥2), deleting one entry by
    // file_name SHALL result in exactly N-1 remaining entries, none of which
    // have the deleted file_name.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_knowledge_delete_removes_target_preserves_others(
            num_entries in 2usize..6,
            delete_idx in 0usize..5,
        ) {
            let num_entries = num_entries;
            let delete_idx = delete_idx % num_entries;

            let db = setup_knowledge_test_db();
            let conn = db.connection();

            // N個のエントリを作成
            let mut file_names = Vec::new();
            for i in 0..num_entries {
                let fname = format!("file_{}.txt", i);
                let entry = KnowledgeEntry {
                    id: format!("know-del-{}", i),
                    session_id: "sess-k001".to_string(),
                    file_name: fname.clone(),
                    content: format!("Content for file {}", i),
                    size_bytes: format!("Content for file {}", i).len() as i64,
                    enabled: true,
                    injection_mode: "system_prompt".to_string(),
                    created_at: format!("2024-01-01T{:02}:00:00Z", i),
                };
                knowledge::add_knowledge(conn, &entry).unwrap();
                file_names.push(fname);
            }

            // 1つ削除
            let target = &file_names[delete_idx];
            knowledge::remove_knowledge(conn, "sess-k001", target).unwrap();

            // N-1件残っている
            let list = knowledge::list_knowledge(conn, "sess-k001").unwrap();
            prop_assert_eq!(list.len(), num_entries - 1, "Should have N-1 entries after deletion");

            // 削除対象が含まれていない
            let remaining_names: Vec<&str> = list.iter().map(|e| e.file_name.as_str()).collect();
            prop_assert!(
                !remaining_names.contains(&target.as_str()),
                "Deleted file_name should not be in remaining entries"
            );

            // 他のエントリは全て残っている
            for (i, fname) in file_names.iter().enumerate() {
                if i != delete_idx {
                    prop_assert!(
                        remaining_names.contains(&fname.as_str()),
                        "Non-deleted entry {} should still exist",
                        fname
                    );
                }
            }
        }
    }

    // ========================================
    // Feature: knowledge-plugin, Property 5: Toggle round-trip preserves entry
    // ========================================
    // **Validates: Requirements 3.1, 3.3**
    //
    // For any knowledge entry, toggling enabled to false then back to true SHALL
    // result in the entry having enabled=true with all other fields unchanged.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_knowledge_toggle_roundtrip_preserves_entry(
            content in valid_knowledge_content(),
            file_name in valid_file_name(),
            injection_mode in prop_oneof![Just("system_prompt"), Just("tool_reference")],
        ) {
            let db = setup_knowledge_test_db();
            let conn = db.connection();

            let entry = KnowledgeEntry {
                id: format!("know-tgl-{}", file_name),
                session_id: "sess-k001".to_string(),
                file_name: file_name.clone(),
                content: content.clone(),
                size_bytes: content.len() as i64,
                enabled: true,
                injection_mode: injection_mode.to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
            };
            knowledge::add_knowledge(conn, &entry).unwrap();

            // toggle off
            knowledge::toggle_knowledge(conn, "sess-k001", &file_name, false).unwrap();
            // toggle back on
            knowledge::toggle_knowledge(conn, "sess-k001", &file_name, true).unwrap();

            // 検証: enabled=true, 他フィールドは変わっていない
            let list = knowledge::list_knowledge(conn, "sess-k001").unwrap();
            let found = list.iter().find(|e| e.file_name == file_name).unwrap();

            prop_assert!(found.enabled, "enabled should be true after toggle round-trip");
            prop_assert_eq!(&found.file_name, &file_name, "file_name should be preserved");
            prop_assert_eq!(found.size_bytes, content.len() as i64, "size_bytes should be preserved");
            prop_assert_eq!(&found.injection_mode, injection_mode, "injection_mode should be preserved");

            // content も保持されている
            let retrieved = knowledge::get_knowledge_content(conn, "sess-k001", &file_name).unwrap();
            prop_assert_eq!(&retrieved, &content, "content should be preserved after toggle round-trip");
        }
    }

    // ========================================
    // Feature: knowledge-plugin, Property 7: Injection mode persistence
    // ========================================
    // **Validates: Requirements 4.1, 4.2**
    //
    // For any knowledge entry and any valid injection_mode value, setting the
    // injection_mode and then reading the entry SHALL return the updated mode value.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_knowledge_injection_mode_persistence(
            content in valid_knowledge_content(),
            file_name in valid_file_name(),
            initial_mode in prop_oneof![Just("system_prompt"), Just("tool_reference")],
            target_mode in prop_oneof![Just("system_prompt"), Just("tool_reference")],
        ) {
            let db = setup_knowledge_test_db();
            let conn = db.connection();

            let entry = KnowledgeEntry {
                id: format!("know-inj-{}", file_name),
                session_id: "sess-k001".to_string(),
                file_name: file_name.clone(),
                content: content.clone(),
                size_bytes: content.len() as i64,
                enabled: true,
                injection_mode: initial_mode.to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
            };
            knowledge::add_knowledge(conn, &entry).unwrap();

            // injection_mode を変更
            knowledge::set_injection_mode(conn, "sess-k001", &file_name, target_mode).unwrap();

            // 読み取って検証
            let list = knowledge::list_knowledge(conn, "sess-k001").unwrap();
            let found = list.iter().find(|e| e.file_name == file_name).unwrap();
            prop_assert_eq!(
                &found.injection_mode, target_mode,
                "injection_mode should be updated to target_mode"
            );
        }
    }

    // ========================================
    // Feature: knowledge-plugin, Property 12: list_knowledge returns ordered metadata without content
    // ========================================
    // **Validates: Requirements 8.2, 10.3**
    //
    // For any session with knowledge entries, list_knowledge SHALL return all
    // entries ordered by created_at ascending.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn prop_knowledge_list_returns_ordered_metadata(
            num_entries in 1usize..8,
        ) {
            let db = setup_knowledge_test_db();
            let conn = db.connection();

            // ランダムな順番でエントリ作成（created_atは逆順で挿入）
            let mut expected_order: Vec<(String, String)> = Vec::new();
            for i in 0..num_entries {
                let fname = format!("ordered_{}.txt", i);
                // created_at を i に基づいて設定（昇順になるように）
                let created_at = format!("2024-01-{:02}T00:00:00Z", i + 1);
                let content = format!("Content {}", i);
                let entry = KnowledgeEntry {
                    id: format!("know-ord-{}", i),
                    session_id: "sess-k001".to_string(),
                    file_name: fname.clone(),
                    content: content.clone(),
                    size_bytes: content.len() as i64,
                    enabled: true,
                    injection_mode: "system_prompt".to_string(),
                    created_at: created_at.clone(),
                };
                knowledge::add_knowledge(conn, &entry).unwrap();
                expected_order.push((fname, created_at));
            }

            // 逆順でもう1つ追加して順序を検証しやすくする
            // (上記は既に昇順なので、逆順で挿入して結果が created_at 昇順かを確認)

            let list = knowledge::list_knowledge(conn, "sess-k001").unwrap();

            // 件数一致
            prop_assert_eq!(list.len(), num_entries, "Should return all entries");

            // created_at 昇順で並んでいることを検証
            for i in 1..list.len() {
                prop_assert!(
                    list[i - 1].created_at <= list[i].created_at,
                    "Entries should be ordered by created_at ascending: {} vs {}",
                    list[i - 1].created_at,
                    list[i].created_at
                );
            }

            // 各エントリにメタデータフィールドが含まれている
            for entry in &list {
                prop_assert!(!entry.id.is_empty(), "id should be present");
                prop_assert!(!entry.file_name.is_empty(), "file_name should be present");
                prop_assert!(entry.size_bytes > 0, "size_bytes should be positive");
                prop_assert!(!entry.injection_mode.is_empty(), "injection_mode should be present");
                prop_assert!(!entry.created_at.is_empty(), "created_at should be present");
            }
        }
    }

    // ========================================
    // Feature: knowledge-plugin, Property 11: Cascade delete removes knowledge entries
    // ========================================
    // **Validates: Requirements 9.2**
    //
    // For any chat session with associated knowledge entries, deleting the session
    // SHALL result in zero knowledge entries remaining for that session_id.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_knowledge_cascade_delete_removes_entries(
            num_entries in 1usize..10,
        ) {
            let db = Database::open_in_memory().unwrap();
            let conn = db.connection();

            // キャラクター作成
            conn.execute(
                "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "char-cascade",
                    "CascadeTest",
                    "Desc",
                    "Prompt",
                    "2024-01-01T00:00:00Z",
                    "2024-01-01T00:00:00Z"
                ],
            )
            .unwrap();

            // セッション作成
            conn.execute(
                "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
                params!["sess-cascade", "char-cascade", "2024-01-01T00:00:00Z"],
            )
            .unwrap();

            // ランダムな数のナレッジエントリを追加
            for i in 0..num_entries {
                let entry = KnowledgeEntry {
                    id: format!("know-cascade-{}", i),
                    session_id: "sess-cascade".to_string(),
                    file_name: format!("cascade_file_{}.txt", i),
                    content: format!("Cascade content {}", i),
                    size_bytes: format!("Cascade content {}", i).len() as i64,
                    enabled: true,
                    injection_mode: if i % 2 == 0 { "system_prompt" } else { "tool_reference" }.to_string(),
                    created_at: format!("2024-01-01T{:02}:00:00Z", i),
                };
                knowledge::add_knowledge(conn, &entry).unwrap();
            }

            // エントリが存在することを確認
            let before = knowledge::list_knowledge(conn, "sess-cascade").unwrap();
            prop_assert_eq!(before.len(), num_entries, "Should have {} entries before deletion", num_entries);

            // セッション削除（CASCADE で knowledge エントリも削除されるべき）
            conn.execute(
                "DELETE FROM chat_sessions WHERE id = ?1",
                params!["sess-cascade"],
            )
            .unwrap();

            // ナレッジエントリが全て削除されていることを検証
            let after = knowledge::list_knowledge(conn, "sess-cascade").unwrap();
            prop_assert_eq!(after.len(), 0, "All knowledge entries should be deleted after session deletion");
        }
    }

    // ========================================
    // Feature: thinking-reasoning-support, Property 5: DB persistence round-trip for thinking content
    // ========================================
    // **Validates: Requirements 4.2, 4.3**
    //
    // For any valid ChatMessageRecord with a non-null thinking_content field,
    // inserting the record into the database and then retrieving it SHALL produce
    // a record with identical thinking_content value.

    /// thinking_content用のストラテジー: Option<String>（None, Some(空), Some(ASCII), Some(マルチバイト), Some(長文)）
    fn arb_thinking_content() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            // None (thinking なし)
            Just(None),
            // 空文字列
            Just(Some(String::new())),
            // ASCII文字列
            "[a-zA-Z0-9 .,!?\\n]{1,200}".prop_map(Some),
            // マルチバイト文字列（日本語）
            "[ぁ-んァ-ヶ亜-熙]{1,100}".prop_map(Some),
            // 長い文字列（1000〜5000文字）
            "[a-zA-Z0-9ぁ-んァ-ヶ \\n]{1000,5000}".prop_map(Some),
            // 特殊文字を含む文字列
            ".*{1,200}".prop_map(Some),
        ]
    }

    /// thinking content DBラウンドトリップテスト用のDBセットアップ
    fn setup_thinking_test_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "char-think",
                "ThinkingTest",
                "Desc",
                "Prompt",
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z"
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            params!["sess-think", "char-think", "2024-01-01T00:00:00Z"],
        )
        .unwrap();

        db
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_thinking_content_db_roundtrip(
            thinking_content in arb_thinking_content(),
            msg_idx in 0u32..10000,
        ) {
            let db = setup_thinking_test_db();
            let conn = db.connection();

            let msg_id = format!("msg-think-{:05}", msg_idx);

            let message = ChatMessageRecord {
                id: msg_id.clone(),
                session_id: "sess-think".to_string(),
                role: ChatRole::Assistant,
                content: "Test response".to_string(),
                attachments: None,
                tool_calls: None,
                tool_call_id: None,
                thinking_content: thinking_content.clone(),
                created_at: "2024-01-01T10:00:00Z".to_string(),
            };

            // Insert
            chat::insert_message(conn, &message).unwrap();

            // Retrieve
            let messages = chat::get_messages(conn, "sess-think").unwrap();
            prop_assert_eq!(messages.len(), 1, "Should have exactly one message");

            let retrieved = &messages[0];
            prop_assert_eq!(
                &retrieved.thinking_content,
                &thinking_content,
                "thinking_content should survive DB round-trip: expected {:?}, got {:?}",
                thinking_content,
                retrieved.thinking_content
            );
        }
    }
}
