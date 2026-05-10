// ChatPluginConfig repository - チャット別プラグイン設定 CRUD

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::ChatPluginConfig;

/// セッション×プラグイン名で設定を取得
pub fn get_config(
    conn: &Connection,
    session_id: &str,
    plugin_name: &str,
) -> Result<Option<ChatPluginConfig>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, plugin_name, config_json, updated_at
         FROM chat_plugin_configs
         WHERE session_id = ?1 AND plugin_name = ?2",
    )?;

    let mut rows = stmt.query_map(params![session_id, plugin_name], |row| {
        Ok(ChatPluginConfig {
            id: row.get(0)?,
            session_id: row.get(1)?,
            plugin_name: row.get(2)?,
            config_json: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;

    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// 設定を upsert（存在すれば更新、なければ挿入）
pub fn upsert_config(
    conn: &Connection,
    session_id: &str,
    plugin_name: &str,
    config_json: &str,
) -> Result<ChatPluginConfig, AppError> {
    let now = chrono::Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO chat_plugin_configs (id, session_id, plugin_name, config_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(session_id, plugin_name) DO UPDATE SET
           config_json = excluded.config_json,
           updated_at = excluded.updated_at",
        params![id, session_id, plugin_name, config_json, now],
    )?;

    // upsert 後の実レコードを返す
    get_config(conn, session_id, plugin_name)?
        .ok_or_else(|| AppError::Database("upsert後のレコード取得に失敗".to_string()))
}

/// 設定を削除
pub fn delete_config(
    conn: &Connection,
    session_id: &str,
    plugin_name: &str,
) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM chat_plugin_configs WHERE session_id = ?1 AND plugin_name = ?2",
        params![session_id, plugin_name],
    )?;
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
    fn test_get_config_not_found() {
        let db = setup_db();
        let conn = db.connection();

        let result = get_config(conn, "sess-001", "file_ops").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_upsert_and_get_config() {
        let db = setup_db();
        let conn = db.connection();

        let config_json = r#"{"directories":[]}"#;
        let config = upsert_config(conn, "sess-001", "file_ops", config_json).unwrap();

        assert_eq!(config.session_id, "sess-001");
        assert_eq!(config.plugin_name, "file_ops");
        assert_eq!(config.config_json, config_json);

        // get で取得確認
        let fetched = get_config(conn, "sess-001", "file_ops").unwrap().unwrap();
        assert_eq!(fetched.id, config.id);
    }

    #[test]
    fn test_upsert_updates_existing() {
        let db = setup_db();
        let conn = db.connection();

        let json1 = r#"{"directories":[]}"#;
        let json2 = r#"{"directories":[{"path":"D:/proj","allow_read":true,"allow_write":true}]}"#;

        let first = upsert_config(conn, "sess-001", "file_ops", json1).unwrap();
        let second = upsert_config(conn, "sess-001", "file_ops", json2).unwrap();

        // ID は初回挿入時のものが維持される
        assert_eq!(first.id, second.id);
        // config_json は更新される
        assert_eq!(second.config_json, json2);
    }

    #[test]
    fn test_delete_config() {
        let db = setup_db();
        let conn = db.connection();

        upsert_config(conn, "sess-001", "file_ops", r#"{}"#).unwrap();
        delete_config(conn, "sess-001", "file_ops").unwrap();

        let result = get_config(conn, "sess-001", "file_ops").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_cascade_delete_on_session_removal() {
        let db = setup_db();
        let conn = db.connection();

        upsert_config(conn, "sess-001", "file_ops", r#"{}"#).unwrap();

        conn.execute(
            "DELETE FROM chat_sessions WHERE id = ?1",
            params!["sess-001"],
        )
        .unwrap();

        let result = get_config(conn, "sess-001", "file_ops").unwrap();
        assert!(result.is_none());
    }
}
