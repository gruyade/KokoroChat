//! データモデルのプロパティテスト
//! proptest を使用してデータ層の不変条件を検証する。

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rusqlite::params;

    use crate::db::database::Database;
    use crate::db::repositories::{character, chat, memory, thought};
    use crate::models::tts::{EmotionParams, TTSConfig, TTSProvider};
    use crate::models::{Character, ChatMessageRecord, ChatRole, ChatSession, Memory, Thought};

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
}
