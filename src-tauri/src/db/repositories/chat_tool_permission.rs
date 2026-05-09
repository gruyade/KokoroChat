// ChatToolPermission repository - チャット別ツール許可設定 CRUD

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::ChatToolPermission;

/// セッションの全ツール許可設定を取得
pub fn get_session_tool_permissions(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<ChatToolPermission>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, tool_name, is_enabled, created_at
         FROM chat_tool_permissions WHERE session_id = ?1
         ORDER BY tool_name ASC",
    )?;

    let rows = stmt.query_map(params![session_id], |row| {
        Ok(ChatToolPermission {
            id: row.get(0)?,
            session_id: row.get(1)?,
            tool_name: row.get(2)?,
            is_enabled: row.get::<_, i32>(3)? != 0,
            created_at: row.get(4)?,
        })
    })?;

    let mut permissions = Vec::new();
    for row in rows {
        permissions.push(row?);
    }
    Ok(permissions)
}

/// ツール許可設定を upsert（存在すれば更新、なければ挿入）
pub fn set_session_tool_permission(
    conn: &Connection,
    session_id: &str,
    tool_name: &str,
    is_enabled: bool,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO chat_tool_permissions (id, session_id, tool_name, is_enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(session_id, tool_name) DO UPDATE SET is_enabled = excluded.is_enabled",
        params![id, session_id, tool_name, is_enabled as i32, now],
    )?;
    Ok(())
}

/// セッション作成時にデフォルト設定で一括初期化
/// defaults: (tool_name, is_enabled) のペアリスト
pub fn init_session_permissions(
    conn: &Connection,
    session_id: &str,
    defaults: &[(&str, bool)],
) -> Result<(), AppError> {
    let now = chrono::Utc::now().to_rfc3339();

    for (tool_name, is_enabled) in defaults {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO chat_tool_permissions (id, session_id, tool_name, is_enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(session_id, tool_name) DO NOTHING",
            params![id, session_id, *tool_name, *is_enabled as i32, now],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::database::Database;

    fn setup_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();
        conn.execute(
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
        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at)
             VALUES (?1, ?2, ?3)",
            params!["sess-001", "char-001", "2024-01-01T00:00:00Z"],
        )
        .unwrap();
        db
    }

    #[test]
    fn test_get_empty_permissions() {
        let db = setup_db();
        let conn = db.connection();

        let perms = get_session_tool_permissions(conn, "sess-001").unwrap();
        assert!(perms.is_empty());
    }

    #[test]
    fn test_set_and_get_permission() {
        let db = setup_db();
        let conn = db.connection();

        set_session_tool_permission(conn, "sess-001", "calculator", true).unwrap();
        set_session_tool_permission(conn, "sess-001", "web_search", false).unwrap();

        let perms = get_session_tool_permissions(conn, "sess-001").unwrap();
        assert_eq!(perms.len(), 2);

        let calc = perms.iter().find(|p| p.tool_name == "calculator").unwrap();
        assert!(calc.is_enabled);

        let web = perms.iter().find(|p| p.tool_name == "web_search").unwrap();
        assert!(!web.is_enabled);
    }

    #[test]
    fn test_upsert_permission() {
        let db = setup_db();
        let conn = db.connection();

        set_session_tool_permission(conn, "sess-001", "calculator", true).unwrap();
        set_session_tool_permission(conn, "sess-001", "calculator", false).unwrap();

        let perms = get_session_tool_permissions(conn, "sess-001").unwrap();
        assert_eq!(perms.len(), 1);
        assert!(!perms[0].is_enabled);
    }

    #[test]
    fn test_init_session_permissions() {
        let db = setup_db();
        let conn = db.connection();

        let defaults = vec![
            ("calculator", true),
            ("web_search", true),
            ("file_ops", false),
        ];
        init_session_permissions(conn, "sess-001", &defaults).unwrap();

        let perms = get_session_tool_permissions(conn, "sess-001").unwrap();
        assert_eq!(perms.len(), 3);

        let calc = perms.iter().find(|p| p.tool_name == "calculator").unwrap();
        assert!(calc.is_enabled);

        let file_ops = perms.iter().find(|p| p.tool_name == "file_ops").unwrap();
        assert!(!file_ops.is_enabled);
    }

    #[test]
    fn test_init_does_not_overwrite_existing() {
        let db = setup_db();
        let conn = db.connection();

        // 先に手動設定
        set_session_tool_permission(conn, "sess-001", "calculator", false).unwrap();

        // init で上書きされないことを確認
        let defaults = vec![("calculator", true), ("web_search", true)];
        init_session_permissions(conn, "sess-001", &defaults).unwrap();

        let perms = get_session_tool_permissions(conn, "sess-001").unwrap();
        let calc = perms.iter().find(|p| p.tool_name == "calculator").unwrap();
        assert!(!calc.is_enabled); // 元の false のまま
    }

    #[test]
    fn test_cascade_delete_on_session_removal() {
        let db = setup_db();
        let conn = db.connection();

        set_session_tool_permission(conn, "sess-001", "calculator", true).unwrap();

        // セッション削除
        conn.execute(
            "DELETE FROM chat_sessions WHERE id = ?1",
            params!["sess-001"],
        )
        .unwrap();

        let perms = get_session_tool_permissions(conn, "sess-001").unwrap();
        assert!(perms.is_empty());
    }
}
