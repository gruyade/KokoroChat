// Thought repository - CRUD操作

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::models::Thought;

/// 思考をDBに挿入
pub fn insert_thought(conn: &Connection, thought: &Thought) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO thoughts (id, character_id, content, context, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            thought.id,
            thought.character_id,
            thought.content,
            thought.context,
            thought.created_at,
        ],
    )?;
    Ok(())
}

/// キャラクターIDで思考一覧取得（作成日時の降順、件数制限あり）
pub fn get_thoughts(
    conn: &Connection,
    character_id: &str,
    limit: Option<u32>,
) -> Result<Vec<Thought>, AppError> {
    let sql = match limit {
        Some(n) => format!(
            "SELECT id, character_id, content, context, created_at
             FROM thoughts WHERE character_id = ?1
             ORDER BY created_at DESC LIMIT {}",
            n
        ),
        None => "SELECT id, character_id, content, context, created_at
             FROM thoughts WHERE character_id = ?1
             ORDER BY created_at DESC"
            .to_string(),
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![character_id], |row| {
        Ok(Thought {
            id: row.get(0)?,
            character_id: row.get(1)?,
            content: row.get(2)?,
            context: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;

    let mut thoughts = Vec::new();
    for row in rows {
        thoughts.push(row?);
    }
    Ok(thoughts)
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

    fn sample_thought(id: &str, created_at: &str) -> Thought {
        Thought {
            id: id.to_string(),
            character_id: "char-001".to_string(),
            content: "今日は天気がいいな".to_string(),
            context: Some("直近の会話で天気の話題が出た".to_string()),
            created_at: created_at.to_string(),
        }
    }

    #[test]
    fn test_insert_and_get_thoughts() {
        let db = setup_db();
        let conn = db.connection();

        let thought = sample_thought("thought-001", "2024-01-01T10:00:00Z");
        insert_thought(conn, &thought).unwrap();

        let thoughts = get_thoughts(conn, "char-001", None).unwrap();
        assert_eq!(thoughts.len(), 1);
        assert_eq!(thoughts[0].id, "thought-001");
        assert_eq!(thoughts[0].content, "今日は天気がいいな");
        assert_eq!(
            thoughts[0].context,
            Some("直近の会話で天気の話題が出た".to_string())
        );
    }

    #[test]
    fn test_insert_thought_without_context() {
        let db = setup_db();
        let conn = db.connection();

        let thought = Thought {
            id: "thought-001".to_string(),
            character_id: "char-001".to_string(),
            content: "ランダムな思考".to_string(),
            context: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        insert_thought(conn, &thought).unwrap();
        let thoughts = get_thoughts(conn, "char-001", None).unwrap();
        assert_eq!(thoughts.len(), 1);
        assert!(thoughts[0].context.is_none());
    }

    #[test]
    fn test_get_thoughts_with_limit() {
        let db = setup_db();
        let conn = db.connection();

        for i in 0..5 {
            let thought = sample_thought(
                &format!("thought-{:03}", i),
                &format!("2024-01-01T{:02}:00:00Z", 10 + i),
            );
            insert_thought(conn, &thought).unwrap();
        }

        let thoughts = get_thoughts(conn, "char-001", Some(3)).unwrap();
        assert_eq!(thoughts.len(), 3);
        // DESC順なので最新のものから
        assert_eq!(thoughts[0].id, "thought-004");
        assert_eq!(thoughts[1].id, "thought-003");
        assert_eq!(thoughts[2].id, "thought-002");
    }

    #[test]
    fn test_get_thoughts_no_limit() {
        let db = setup_db();
        let conn = db.connection();

        for i in 0..5 {
            let thought = sample_thought(
                &format!("thought-{:03}", i),
                &format!("2024-01-01T{:02}:00:00Z", 10 + i),
            );
            insert_thought(conn, &thought).unwrap();
        }

        let thoughts = get_thoughts(conn, "char-001", None).unwrap();
        assert_eq!(thoughts.len(), 5);
    }

    #[test]
    fn test_get_thoughts_empty() {
        let db = setup_db();
        let conn = db.connection();

        let thoughts = get_thoughts(conn, "char-001", None).unwrap();
        assert!(thoughts.is_empty());
    }

    #[test]
    fn test_get_thoughts_only_for_character() {
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

        let t1 = Thought {
            id: "thought-001".to_string(),
            character_id: "char-001".to_string(),
            content: "キャラ1の思考".to_string(),
            context: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        let t2 = Thought {
            id: "thought-002".to_string(),
            character_id: "char-002".to_string(),
            content: "キャラ2の思考".to_string(),
            context: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        };

        insert_thought(conn, &t1).unwrap();
        insert_thought(conn, &t2).unwrap();

        let thoughts = get_thoughts(conn, "char-001", None).unwrap();
        assert_eq!(thoughts.len(), 1);
        assert_eq!(thoughts[0].id, "thought-001");
    }
}
