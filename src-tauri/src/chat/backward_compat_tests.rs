// 後方互換性テスト
//
// Task 9.2: thinking=null / thinking_content=None の場合に既存動作が維持されることを確認
//
// **Validates: Requirements 2.5, 2.6, 4.4, 5.6**

#[cfg(test)]
mod backward_compat_tests {
    use crate::chat::engine::ChatStreamEvent;
    use crate::llm::client::LLMResponse;
    use crate::models::{ChatMessageRecord, ChatRole};

    /// ChatStreamEvent with `thinking: None` serializes correctly
    /// thinkingフィールドはJSON内でnullとして出力されること（省略されないこと）
    ///
    /// **Validates: Requirements 2.5, 2.6**
    #[test]
    fn test_chat_stream_event_thinking_none_serializes_as_null() {
        let event = ChatStreamEvent {
            session_id: "sess-001".to_string(),
            chunk: "Hello".to_string(),
            done: false,
            tool_break: false,
            thinking: None,
        };

        let json = serde_json::to_value(&event).unwrap();

        // thinkingフィールドが存在し、nullであること
        assert!(json.get("thinking").is_some(), "thinking field must be present in JSON");
        assert!(json["thinking"].is_null(), "thinking field must be null when None");

        // 通常フィールドは正常にシリアライズ
        assert_eq!(json["chunk"], "Hello");
        assert_eq!(json["done"], false);
        assert_eq!(json["tool_break"], false);
        assert_eq!(json["session_id"], "sess-001");
    }

    /// ChatStreamEvent with `done: true` and `thinking: None` — 完了イベントの後方互換性
    ///
    /// **Validates: Requirements 2.6**
    #[test]
    fn test_chat_stream_event_done_with_thinking_none() {
        let event = ChatStreamEvent {
            session_id: "sess-001".to_string(),
            chunk: "final text".to_string(),
            done: true,
            tool_break: false,
            thinking: None,
        };

        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(json["done"], true);
        assert!(json["thinking"].is_null());
        assert_eq!(json["chunk"], "final text");
    }

    /// ChatStreamEvent with `tool_break: true` and `thinking: None`
    ///
    /// **Validates: Requirements 2.6**
    #[test]
    fn test_chat_stream_event_tool_break_with_thinking_none() {
        let event = ChatStreamEvent {
            session_id: "sess-001".to_string(),
            chunk: "pre-tool text".to_string(),
            done: false,
            tool_break: true,
            thinking: None,
        };

        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(json["tool_break"], true);
        assert!(json["thinking"].is_null());
    }

    /// ChatMessageRecord with `thinking_content: None` — 既存メッセージの動作確認
    /// thinking_contentがNoneのメッセージは正常にシリアライズ/デシリアライズ可能
    ///
    /// **Validates: Requirements 4.4**
    #[test]
    fn test_chat_message_record_thinking_content_none_roundtrip() {
        let record = ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::Assistant,
            content: "これはテストメッセージです".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            thinking_content: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        // シリアライズ
        let json_str = serde_json::to_string(&record).unwrap();
        // デシリアライズ
        let deserialized: ChatMessageRecord = serde_json::from_str(&json_str).unwrap();

        assert_eq!(deserialized.id, "msg-001");
        assert_eq!(deserialized.content, "これはテストメッセージです");
        assert!(deserialized.thinking_content.is_none());
    }

    /// ChatMessageRecord without thinking_content field in JSON — デシリアライズ時にNoneになること
    /// 古いフォーマット（thinking_contentフィールドなし）との後方互換性
    ///
    /// **Validates: Requirements 4.4**
    #[test]
    fn test_chat_message_record_missing_thinking_content_field_deserializes_as_none() {
        // thinking_contentフィールドが存在しないJSONからデシリアライズ
        let json_without_thinking = r#"{
            "id": "msg-old",
            "session_id": "sess-001",
            "role": "assistant",
            "content": "古いメッセージ",
            "created_at": "2024-01-01T00:00:00Z"
        }"#;

        let record: ChatMessageRecord = serde_json::from_str(json_without_thinking).unwrap();

        assert_eq!(record.id, "msg-old");
        assert_eq!(record.content, "古いメッセージ");
        assert!(record.thinking_content.is_none());
        assert!(record.attachments.is_none());
        assert!(record.tool_calls.is_none());
    }

    /// LLMResponse::Text { content, thinking: None } — 旧動作と同一であること
    ///
    /// **Validates: Requirements 2.5**
    #[test]
    fn test_llm_response_text_thinking_none_behavior() {
        let response = LLMResponse::Text {
            content: "Hello, world!".to_string(),
            thinking: None,
        };

        // text() はcontent値を返す
        assert_eq!(response.text(), "Hello, world!");

        // is_tool_calls() はfalse
        assert!(!response.is_tool_calls());

        // into_text() はcontent値を消費して返す
        let text = response.into_text();
        assert_eq!(text, "Hello, world!");
    }

    /// LLMResponse::ToolCalls { calls, thinking: None } — thinking無しでのtool calls動作
    ///
    /// **Validates: Requirements 2.5**
    #[test]
    fn test_llm_response_tool_calls_thinking_none_behavior() {
        use crate::models::plugin::ToolCall;

        let response = LLMResponse::ToolCalls {
            calls: vec![ToolCall {
                id: "call-001".to_string(),
                name: "test_tool".to_string(),
                arguments: serde_json::json!({"key": "value"}),
                context: None,
            }],
            thinking: None,
        };

        // text() は空文字列を返す
        assert_eq!(response.text(), "");

        // is_tool_calls() はtrue
        assert!(response.is_tool_calls());
    }

    /// ChatMessageRecord with `thinking_content: None` — DB insert/get roundtrip
    /// マイグレーション後も既存メッセージ（thinking_content=None）が正常に保存・取得できること
    ///
    /// **Validates: Requirements 4.4, 5.6**
    #[test]
    fn test_chat_message_record_thinking_content_none_db_roundtrip() {
        use crate::db::database::Database;
        use crate::db::repositories::chat as chat_repo;
        use rusqlite::params;

        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        // テスト用キャラクター・セッション作成
        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "char-compat",
                "CompatTest",
                "Desc",
                "Prompt",
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z"
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            params!["sess-compat", "char-compat", "2024-01-01T00:00:00Z"],
        )
        .unwrap();

        // thinking_content: None のメッセージを挿入
        let record = ChatMessageRecord {
            id: "msg-compat-001".to_string(),
            session_id: "sess-compat".to_string(),
            role: ChatRole::Assistant,
            content: "通常の応答メッセージ".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            thinking_content: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        chat_repo::insert_message(conn, &record).unwrap();

        // 取得して検証
        let messages = chat_repo::get_messages(conn, "sess-compat").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "msg-compat-001");
        assert_eq!(messages[0].content, "通常の応答メッセージ");
        assert!(messages[0].thinking_content.is_none(), "thinking_content should be None");
        assert_eq!(messages[0].role, ChatRole::Assistant);
    }

    /// 既存メッセージ（thinking_contentなし）がマイグレーション後も正常動作する
    /// DBに直接thinking_content=NULLで挿入されたレコードが取得可能であること
    ///
    /// **Validates: Requirements 4.4**
    #[test]
    fn test_existing_messages_without_thinking_content_work_after_migration() {
        use crate::db::database::Database;
        use crate::db::repositories::chat as chat_repo;
        use rusqlite::params;

        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        // テスト用キャラクター・セッション作成
        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "char-legacy",
                "LegacyTest",
                "Desc",
                "Prompt",
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z"
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            params!["sess-legacy", "char-legacy", "2024-01-01T00:00:00Z"],
        )
        .unwrap();

        // 直接SQLでthinking_content=NULLのメッセージを複数挿入（マイグレーション前の既存データを模擬）
        conn.execute(
            "INSERT INTO chat_messages (id, session_id, role, content, thinking_content, created_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
            params![
                "msg-legacy-001",
                "sess-legacy",
                "user",
                "こんにちは",
                "2024-01-01T10:00:00Z"
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO chat_messages (id, session_id, role, content, thinking_content, created_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
            params![
                "msg-legacy-002",
                "sess-legacy",
                "assistant",
                "こんにちは！元気ですか？",
                "2024-01-01T10:01:00Z"
            ],
        )
        .unwrap();

        // get_messagesで取得
        let messages = chat_repo::get_messages(conn, "sess-legacy").unwrap();
        assert_eq!(messages.len(), 2);

        // userメッセージ
        assert_eq!(messages[0].role, ChatRole::User);
        assert_eq!(messages[0].content, "こんにちは");
        assert!(messages[0].thinking_content.is_none());

        // assistantメッセージ
        assert_eq!(messages[1].role, ChatRole::Assistant);
        assert_eq!(messages[1].content, "こんにちは！元気ですか？");
        assert!(messages[1].thinking_content.is_none());
    }

    /// ChatStreamEvent with chunk only (no thinking) — 通常テキストストリーミングの既存動作
    ///
    /// **Validates: Requirements 2.5**
    #[test]
    fn test_chat_stream_event_text_only_stream() {
        // 通常のテキストストリーミング: thinkingはnull, chunkにテキスト
        let events = vec![
            ChatStreamEvent {
                session_id: "sess-001".to_string(),
                chunk: "Hello".to_string(),
                done: false,
                tool_break: false,
                thinking: None,
            },
            ChatStreamEvent {
                session_id: "sess-001".to_string(),
                chunk: " World".to_string(),
                done: false,
                tool_break: false,
                thinking: None,
            },
            ChatStreamEvent {
                session_id: "sess-001".to_string(),
                chunk: "Hello World".to_string(),
                done: true,
                tool_break: false,
                thinking: None,
            },
        ];

        // 全イベントのthinkingがnull
        for event in &events {
            let json = serde_json::to_value(event).unwrap();
            assert!(json["thinking"].is_null());
        }

        // 最終イベントのdoneがtrue
        let last = serde_json::to_value(&events[2]).unwrap();
        assert_eq!(last["done"], true);
    }
}
