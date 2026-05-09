// Chat repository - ChatSession, ChatMessage CRUD

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::models::{ChatMessageRecord, ChatRole, ChatSession, MessageAttachment, ToolCall};

/// セッションをDBに挿入
pub fn insert_session(conn: &Connection, session: &ChatSession) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO chat_sessions (id, character_id, title, last_message_at, last_message_preview, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            session.id,
            session.character_id,
            session.title,
            session.last_message_at,
            session.last_message_preview,
            session.created_at,
        ],
    )?;
    Ok(())
}

/// IDでセッションを取得
pub fn get_session(conn: &Connection, id: &str) -> Result<Option<ChatSession>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, character_id, title, last_message_at, last_message_preview, created_at
         FROM chat_sessions WHERE id = ?1",
    )?;

    let mut rows = stmt.query(params![id])?;
    match rows.next()? {
        Some(row) => Ok(Some(ChatSession {
            id: row.get(0)?,
            character_id: row.get(1)?,
            title: row.get(2)?,
            last_message_at: row.get(3)?,
            last_message_preview: row.get(4)?,
            created_at: row.get(5)?,
        })),
        None => Ok(None),
    }
}

/// キャラクターIDでセッション一覧取得（最終メッセージ日時の降順）
pub fn list_sessions(
    conn: &Connection,
    character_id: &str,
) -> Result<Vec<ChatSession>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, character_id, title, last_message_at, last_message_preview, created_at
         FROM chat_sessions WHERE character_id = ?1
         ORDER BY last_message_at DESC",
    )?;

    let rows = stmt.query_map(params![character_id], |row| {
        Ok(ChatSession {
            id: row.get(0)?,
            character_id: row.get(1)?,
            title: row.get(2)?,
            last_message_at: row.get(3)?,
            last_message_preview: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;

    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row?);
    }
    Ok(sessions)
}

/// セッションを削除（CASCADE DELETEによりメッセージも削除）
pub fn delete_session(conn: &Connection, id: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM chat_sessions WHERE id = ?1", params![id])?;
    Ok(())
}

/// セッションのメタデータ（last_message_at, last_message_preview）を更新
pub fn update_session_metadata(
    conn: &Connection,
    session_id: &str,
    last_message_at: &str,
    preview: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE chat_sessions SET last_message_at = ?1, last_message_preview = ?2 WHERE id = ?3",
        params![last_message_at, preview, session_id],
    )?;
    Ok(())
}

/// メッセージをDBに挿入
pub fn insert_message(conn: &Connection, message: &ChatMessageRecord) -> Result<(), AppError> {
    let role_str = match message.role {
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
        ChatRole::Spontaneous => "spontaneous",
        ChatRole::Tool => "tool",
    };

    let attachments_json = message
        .attachments
        .as_ref()
        .map(|a| serde_json::to_string(a))
        .transpose()?;

    let tool_calls_json = message
        .tool_calls
        .as_ref()
        .map(|t| serde_json::to_string(t))
        .transpose()?;

    conn.execute(
        "INSERT INTO chat_messages (id, session_id, role, content, attachments, tool_calls, tool_call_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            message.id,
            message.session_id,
            role_str,
            message.content,
            attachments_json,
            tool_calls_json,
            message.tool_call_id,
            message.created_at,
        ],
    )?;
    Ok(())
}

/// 指定IDのメッセージを削除
pub fn delete_message(conn: &Connection, id: &str) -> Result<bool, AppError> {
    let rows_affected = conn.execute(
        "DELETE FROM chat_messages WHERE id = ?1",
        params![id],
    )?;
    Ok(rows_affected > 0)
}

/// 指定メッセージ以降の全メッセージを削除（指定メッセージ自体は残す）
/// created_at が対象メッセージより後のメッセージを削除する
pub fn delete_messages_after(conn: &Connection, session_id: &str, message_id: &str) -> Result<u32, AppError> {
    // 対象メッセージの created_at を取得
    let created_at: String = conn.query_row(
        "SELECT created_at FROM chat_messages WHERE id = ?1 AND session_id = ?2",
        params![message_id, session_id],
        |row| row.get(0),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("Message not found: {}", message_id))
        }
        other => AppError::Database(other.to_string()),
    })?;

    let rows_affected = conn.execute(
        "DELETE FROM chat_messages WHERE session_id = ?1 AND created_at > ?2",
        params![session_id, created_at],
    )?;
    Ok(rows_affected as u32)
}

/// 指定メッセージの content を更新
pub fn update_message_content(conn: &Connection, message_id: &str, new_content: &str) -> Result<(), AppError> {
    let rows_affected = conn.execute(
        "UPDATE chat_messages SET content = ?1 WHERE id = ?2",
        params![new_content, message_id],
    )?;
    if rows_affected == 0 {
        return Err(AppError::NotFound(format!("Message not found: {}", message_id)));
    }
    Ok(())
}

/// セッションIDでメッセージ一覧取得（作成日時の昇順）
pub fn get_messages(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<ChatMessageRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, role, content, attachments, tool_calls, tool_call_id, created_at
         FROM chat_messages WHERE session_id = ?1
         ORDER BY created_at ASC",
    )?;

    let rows = stmt.query_map(params![session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, String>(7)?,
        ))
    })?;

    let mut messages = Vec::new();
    for row in rows {
        let (id, session_id, role_str, content, attachments_str, tool_calls_str, tool_call_id, created_at) = row?;

        let role = match role_str.as_str() {
            "user" => ChatRole::User,
            "assistant" => ChatRole::Assistant,
            "spontaneous" => ChatRole::Spontaneous,
            "tool" => ChatRole::Tool,
            _ => ChatRole::User, // フォールバック
        };

        let attachments: Option<Vec<MessageAttachment>> = attachments_str
            .map(|s| serde_json::from_str(&s))
            .transpose()?;

        let tool_calls: Option<Vec<ToolCall>> = tool_calls_str
            .map(|s| serde_json::from_str(&s))
            .transpose()?;

        messages.push(ChatMessageRecord {
            id,
            session_id,
            role,
            content,
            attachments,
            tool_calls,
            tool_call_id,
            created_at,
        });
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::database::Database;

    fn setup_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        // テスト用キャラクター挿入
        db.connection()
            .execute(
                "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "char-001",
                    "Test",
                    "Desc",
                    "Prompt",
                    "2024-01-01T00:00:00Z",
                    "2024-01-01T00:00:00Z"
                ],
            )
            .unwrap();
        db
    }

    fn sample_session() -> ChatSession {
        ChatSession {
            id: "sess-001".to_string(),
            character_id: "char-001".to_string(),
            title: Some("テストセッション".to_string()),
            last_message_at: None,
            last_message_preview: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn sample_message(id: &str, session_id: &str, role: ChatRole) -> ChatMessageRecord {
        ChatMessageRecord {
            id: id.to_string(),
            session_id: session_id.to_string(),
            role,
            content: "テストメッセージ".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_insert_and_get_session() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();

        insert_session(conn, &session).unwrap();
        let result = get_session(conn, "sess-001").unwrap();

        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.id, "sess-001");
        assert_eq!(s.character_id, "char-001");
        assert_eq!(s.title, Some("テストセッション".to_string()));
    }

    #[test]
    fn test_get_session_not_found() {
        let db = setup_db();
        let conn = db.connection();

        let result = get_session(conn, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_sessions_ordered_by_last_message_at() {
        let db = setup_db();
        let conn = db.connection();

        let mut s1 = sample_session();
        s1.id = "sess-001".to_string();
        s1.last_message_at = Some("2024-01-01T10:00:00Z".to_string());

        let mut s2 = sample_session();
        s2.id = "sess-002".to_string();
        s2.last_message_at = Some("2024-01-02T10:00:00Z".to_string());

        insert_session(conn, &s1).unwrap();
        insert_session(conn, &s2).unwrap();

        let list = list_sessions(conn, "char-001").unwrap();
        assert_eq!(list.len(), 2);
        // DESC順なのでs2が先
        assert_eq!(list[0].id, "sess-002");
        assert_eq!(list[1].id, "sess-001");
    }

    #[test]
    fn test_delete_session() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();

        insert_session(conn, &session).unwrap();
        delete_session(conn, "sess-001").unwrap();

        let result = get_session(conn, "sess-001").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_session_metadata() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();

        insert_session(conn, &session).unwrap();
        update_session_metadata(conn, "sess-001", "2024-01-01T12:00:00Z", "最新メッセージ")
            .unwrap();

        let result = get_session(conn, "sess-001").unwrap().unwrap();
        assert_eq!(
            result.last_message_at,
            Some("2024-01-01T12:00:00Z".to_string())
        );
        assert_eq!(
            result.last_message_preview,
            Some("最新メッセージ".to_string())
        );
    }

    #[test]
    fn test_insert_and_get_messages() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();
        insert_session(conn, &session).unwrap();

        let msg1 = ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::User,
            content: "こんにちは".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        let msg2 = ChatMessageRecord {
            id: "msg-002".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::Assistant,
            content: "はい、こんにちは！".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:01:00Z".to_string(),
        };

        insert_message(conn, &msg1).unwrap();
        insert_message(conn, &msg2).unwrap();

        let messages = get_messages(conn, "sess-001").unwrap();
        assert_eq!(messages.len(), 2);
        // ASC順
        assert_eq!(messages[0].id, "msg-001");
        assert_eq!(messages[0].role, ChatRole::User);
        assert_eq!(messages[1].id, "msg-002");
        assert_eq!(messages[1].role, ChatRole::Assistant);
    }

    #[test]
    fn test_insert_message_with_attachments() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();
        insert_session(conn, &session).unwrap();

        let msg = ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::User,
            content: "ファイル添付".to_string(),
            attachments: Some(vec![MessageAttachment {
                file_name: "test.txt".to_string(),
                attachment_type: "text".to_string(),
                extracted_text: Some("ファイル内容".to_string()),
                base64_data: None,
            }]),
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        insert_message(conn, &msg).unwrap();
        let messages = get_messages(conn, "sess-001").unwrap();
        assert_eq!(messages.len(), 1);

        let attachments = messages[0].attachments.as_ref().unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].file_name, "test.txt");
        assert_eq!(
            attachments[0].extracted_text,
            Some("ファイル内容".to_string())
        );
    }

    #[test]
    fn test_insert_message_with_tool_calls() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();
        insert_session(conn, &session).unwrap();

        let msg = ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::Assistant,
            content: "".to_string(),
            attachments: None,
            tool_calls: Some(vec![ToolCall {
                id: "call-001".to_string(),
                name: "calculator".to_string(),
                arguments: serde_json::json!({"expression": "1+1"}),
            }]),
            tool_call_id: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        insert_message(conn, &msg).unwrap();
        let messages = get_messages(conn, "sess-001").unwrap();
        assert_eq!(messages.len(), 1);

        let tool_calls = messages[0].tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "calculator");
    }

    #[test]
    fn test_insert_tool_result_message() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();
        insert_session(conn, &session).unwrap();

        let msg = ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::Tool,
            content: "2".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: Some("call-001".to_string()),
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        insert_message(conn, &msg).unwrap();
        let messages = get_messages(conn, "sess-001").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, ChatRole::Tool);
        assert_eq!(messages[0].tool_call_id, Some("call-001".to_string()));
    }

    #[test]
    fn test_spontaneous_message_role() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();
        insert_session(conn, &session).unwrap();

        let msg = sample_message("msg-001", "sess-001", ChatRole::Spontaneous);
        insert_message(conn, &msg).unwrap();

        let messages = get_messages(conn, "sess-001").unwrap();
        assert_eq!(messages[0].role, ChatRole::Spontaneous);
    }

    #[test]
    fn test_delete_messages_after() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();
        insert_session(conn, &session).unwrap();

        let msg1 = ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::User,
            content: "最初のメッセージ".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };
        let msg2 = ChatMessageRecord {
            id: "msg-002".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::Assistant,
            content: "返答1".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:01:00Z".to_string(),
        };
        let msg3 = ChatMessageRecord {
            id: "msg-003".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::User,
            content: "2番目のメッセージ".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:02:00Z".to_string(),
        };

        insert_message(conn, &msg1).unwrap();
        insert_message(conn, &msg2).unwrap();
        insert_message(conn, &msg3).unwrap();

        // msg-001 以降を削除 → msg-002, msg-003 が削除される
        let deleted = delete_messages_after(conn, "sess-001", "msg-001").unwrap();
        assert_eq!(deleted, 2);

        let messages = get_messages(conn, "sess-001").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "msg-001");
    }

    #[test]
    fn test_delete_messages_after_last_message() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();
        insert_session(conn, &session).unwrap();

        let msg1 = ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::User,
            content: "唯一のメッセージ".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        insert_message(conn, &msg1).unwrap();

        // 最後のメッセージ以降を削除 → 何も削除されない
        let deleted = delete_messages_after(conn, "sess-001", "msg-001").unwrap();
        assert_eq!(deleted, 0);

        let messages = get_messages(conn, "sess-001").unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_update_message_content() {
        let db = setup_db();
        let conn = db.connection();
        let session = sample_session();
        insert_session(conn, &session).unwrap();

        let msg = ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::User,
            content: "元の内容".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        insert_message(conn, &msg).unwrap();
        update_message_content(conn, "msg-001", "更新後の内容").unwrap();

        let messages = get_messages(conn, "sess-001").unwrap();
        assert_eq!(messages[0].content, "更新後の内容");
    }

    #[test]
    fn test_update_message_content_not_found() {
        let db = setup_db();
        let conn = db.connection();

        let result = update_message_content(conn, "nonexistent", "新しい内容");
        assert!(result.is_err());
    }
}
