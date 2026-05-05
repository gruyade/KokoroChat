// Memory repository - CRUD操作

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::models::Memory;

/// メモリをDBに挿入
pub fn insert_memory(conn: &Connection, memory: &Memory) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO memories (id, character_id, content, source_session_id, source_message_from, source_message_to, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            memory.id,
            memory.character_id,
            memory.content,
            memory.source_session_id,
            memory.source_message_from,
            memory.source_message_to,
            memory.created_at,
            memory.updated_at,
        ],
    )?;
    Ok(())
}

/// キャラクターIDでメモリ一覧取得（作成日時の降順）
pub fn list_memories(conn: &Connection, character_id: &str) -> Result<Vec<Memory>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, character_id, content, source_session_id, source_message_from, source_message_to, created_at, updated_at
         FROM memories WHERE character_id = ?1
         ORDER BY created_at DESC",
    )?;

    let rows = stmt.query_map(params![character_id], |row| {
        Ok(Memory {
            id: row.get(0)?,
            character_id: row.get(1)?,
            content: row.get(2)?,
            source_session_id: row.get(3)?,
            source_message_from: row.get(4)?,
            source_message_to: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;

    let mut memories = Vec::new();
    for row in rows {
        memories.push(row?);
    }
    Ok(memories)
}

/// メモリの内容を更新
pub fn update_memory(conn: &Connection, id: &str, content: &str) -> Result<(), AppError> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE memories SET content = ?1, updated_at = ?2 WHERE id = ?3",
        params![content, updated_at, id],
    )?;
    Ok(())
}

/// メモリを削除
pub fn delete_memory(conn: &Connection, id: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::database::Database;

    fn setup_db() -> Database {
        let db = Database::open_in_memory().unwrap();
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

    fn sample_memory() -> Memory {
        Memory {
            id: "mem-001".to_string(),
            character_id: "char-001".to_string(),
            content: "ユーザーは猫が好き".to_string(),
            source_session_id: Some("sess-001".to_string()),
            source_message_from: Some("msg-001".to_string()),
            source_message_to: Some("msg-010".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_insert_and_list_memories() {
        let db = setup_db();
        let conn = db.connection();

        // セッション作成（source_session_id用）
        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            params!["sess-001", "char-001", "2024-01-01T00:00:00Z"],
        )
        .unwrap();

        let memory = sample_memory();
        insert_memory(conn, &memory).unwrap();

        let list = list_memories(conn, "char-001").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "mem-001");
        assert_eq!(list[0].content, "ユーザーは猫が好き");
        assert_eq!(list[0].source_session_id, Some("sess-001".to_string()));
    }

    #[test]
    fn test_insert_memory_without_source() {
        let db = setup_db();
        let conn = db.connection();

        let memory = Memory {
            id: "mem-001".to_string(),
            character_id: "char-001".to_string(),
            content: "手動追加メモリ".to_string(),
            source_session_id: None,
            source_message_from: None,
            source_message_to: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        insert_memory(conn, &memory).unwrap();
        let list = list_memories(conn, "char-001").unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].source_session_id.is_none());
    }

    #[test]
    fn test_list_memories_empty() {
        let db = setup_db();
        let conn = db.connection();

        let list = list_memories(conn, "char-001").unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_update_memory() {
        let db = setup_db();
        let conn = db.connection();

        let memory = Memory {
            id: "mem-001".to_string(),
            character_id: "char-001".to_string(),
            content: "元の内容".to_string(),
            source_session_id: None,
            source_message_from: None,
            source_message_to: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        insert_memory(conn, &memory).unwrap();
        update_memory(conn, "mem-001", "更新後の内容").unwrap();

        let list = list_memories(conn, "char-001").unwrap();
        assert_eq!(list[0].content, "更新後の内容");
        // updated_atが更新されている
        assert_ne!(list[0].updated_at, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_delete_memory() {
        let db = setup_db();
        let conn = db.connection();

        let memory = Memory {
            id: "mem-001".to_string(),
            character_id: "char-001".to_string(),
            content: "削除対象".to_string(),
            source_session_id: None,
            source_message_from: None,
            source_message_to: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        insert_memory(conn, &memory).unwrap();
        delete_memory(conn, "mem-001").unwrap();

        let list = list_memories(conn, "char-001").unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_memories_only_for_character() {
        let db = setup_db();
        let conn = db.connection();

        // 別キャラクター作成
        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "char-002",
                "Other",
                "Desc",
                "Prompt",
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z"
            ],
        )
        .unwrap();

        let mem1 = Memory {
            id: "mem-001".to_string(),
            character_id: "char-001".to_string(),
            content: "キャラ1のメモリ".to_string(),
            source_session_id: None,
            source_message_from: None,
            source_message_to: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let mem2 = Memory {
            id: "mem-002".to_string(),
            character_id: "char-002".to_string(),
            content: "キャラ2のメモリ".to_string(),
            source_session_id: None,
            source_message_from: None,
            source_message_to: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        insert_memory(conn, &mem1).unwrap();
        insert_memory(conn, &mem2).unwrap();

        let list = list_memories(conn, "char-001").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "mem-001");
    }
}
