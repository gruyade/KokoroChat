// Database - SQLite接続・初期化

use std::path::Path;

use rusqlite::Connection;

use crate::error::AppError;

use super::migrations;

/// SQLiteデータベースラッパー。
/// WALモード・外部キー制約を有効化し、スキーマ初期化を行う。
pub struct Database {
    conn: Connection,
}

impl Database {
    /// 指定パスでデータベースを開き、初期化する。
    /// ファイルが存在しない場合は新規作成。
    pub fn open(db_path: &Path) -> Result<Self, AppError> {
        // 親ディレクトリが存在しない場合は作成
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let conn = Connection::open(db_path)?;
        let db = Self { conn };
        db.configure_pragmas()?;
        db.run_migrations()?;
        Ok(db)
    }

    /// インメモリデータベースを作成（テスト用）。
    pub fn open_in_memory() -> Result<Self, AppError> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.configure_pragmas()?;
        db.run_migrations()?;
        Ok(db)
    }

    /// 内部のConnectionへの参照を返す。
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// PRAGMAを設定（WALモード、外部キー有効化）。
    fn configure_pragmas(&self) -> Result<(), AppError> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;
        Ok(())
    }

    /// マイグレーション実行（テーブル・インデックス作成）。
    fn run_migrations(&self) -> Result<(), AppError> {
        self.conn.execute_batch(migrations::create_tables_sql())?;
        self.conn.execute_batch(migrations::create_indexes_sql())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_open_in_memory() {
        let db = Database::open_in_memory();
        assert!(db.is_ok());
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let db = Database::open_in_memory().unwrap();
        let fk_enabled: i32 = db
            .connection()
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk_enabled, 1);
    }

    #[test]
    fn test_wal_mode_enabled() {
        let db = Database::open_in_memory().unwrap();
        let journal_mode: String = db
            .connection()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        // インメモリDBではWALではなくmemoryが返る場合がある
        assert!(
            journal_mode == "wal" || journal_mode == "memory",
            "journal_mode was: {}",
            journal_mode
        );
    }

    #[test]
    fn test_tables_created() {
        let db = Database::open_in_memory().unwrap();
        let tables: Vec<String> = db
            .connection()
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"characters".to_string()));
        assert!(tables.contains(&"chat_sessions".to_string()));
        assert!(tables.contains(&"chat_messages".to_string()));
        assert!(tables.contains(&"memories".to_string()));
        assert!(tables.contains(&"thoughts".to_string()));
        assert!(tables.contains(&"plugins".to_string()));
        assert!(tables.contains(&"attachments".to_string()));
    }

    #[test]
    fn test_indexes_created() {
        let db = Database::open_in_memory().unwrap();
        let indexes: Vec<String> = db
            .connection()
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(indexes.contains(&"idx_chat_sessions_character".to_string()));
        assert!(indexes.contains(&"idx_chat_messages_session".to_string()));
        assert!(indexes.contains(&"idx_memories_character".to_string()));
        assert!(indexes.contains(&"idx_thoughts_character".to_string()));
        assert!(indexes.contains(&"idx_attachments_message".to_string()));
    }

    #[test]
    fn test_open_creates_parent_directories() {
        let tmp_dir = TempDir::new().unwrap();
        let db_path = tmp_dir.path().join("subdir").join("nested").join("test.db");
        let db = Database::open(&db_path);
        assert!(db.is_ok());
        assert!(db_path.exists());
    }

    #[test]
    fn test_open_file_based_db() {
        let tmp_dir = TempDir::new().unwrap();
        let db_path = tmp_dir.path().join("test.db");
        let db = Database::open(&db_path);
        assert!(db.is_ok());
        assert!(db_path.exists());
    }

    #[test]
    fn test_idempotent_migrations() {
        let db = Database::open_in_memory().unwrap();
        // 2回目のマイグレーション実行もエラーにならない
        let result = db.connection().execute_batch(migrations::create_tables_sql());
        assert!(result.is_ok());
        let result = db.connection().execute_batch(migrations::create_indexes_sql());
        assert!(result.is_ok());
    }

    #[test]
    fn test_cascade_delete_character() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        // キャラクター作成
        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["char1", "Test", "Desc", "Prompt", "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z"],
        ).unwrap();

        // セッション作成
        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["sess1", "char1", "2024-01-01T00:00:00Z"],
        ).unwrap();

        // メッセージ作成
        conn.execute(
            "INSERT INTO chat_messages (id, session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["msg1", "sess1", "user", "Hello", "2024-01-01T00:00:00Z"],
        ).unwrap();

        // メモリ作成
        conn.execute(
            "INSERT INTO memories (id, character_id, content, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["mem1", "char1", "Memory content", "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z"],
        ).unwrap();

        // 思考作成
        conn.execute(
            "INSERT INTO thoughts (id, character_id, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["thought1", "char1", "Thought content", "2024-01-01T00:00:00Z"],
        ).unwrap();

        // キャラクター削除
        conn.execute("DELETE FROM characters WHERE id = ?1", rusqlite::params!["char1"]).unwrap();

        // 関連データが全て削除されていることを確認
        let session_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM chat_sessions WHERE character_id = ?1",
            rusqlite::params!["char1"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(session_count, 0);

        let message_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM chat_messages WHERE session_id = ?1",
            rusqlite::params!["sess1"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(message_count, 0);

        let memory_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE character_id = ?1",
            rusqlite::params!["char1"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(memory_count, 0);

        let thought_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM thoughts WHERE character_id = ?1",
            rusqlite::params!["char1"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(thought_count, 0);
    }

    #[test]
    fn test_cascade_delete_session_removes_messages() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["char1", "Test", "Desc", "Prompt", "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z"],
        ).unwrap();

        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["sess1", "char1", "2024-01-01T00:00:00Z"],
        ).unwrap();

        conn.execute(
            "INSERT INTO chat_messages (id, session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["msg1", "sess1", "user", "Hello", "2024-01-01T00:00:00Z"],
        ).unwrap();

        // セッション削除
        conn.execute("DELETE FROM chat_sessions WHERE id = ?1", rusqlite::params!["sess1"]).unwrap();

        let message_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM chat_messages WHERE session_id = ?1",
            rusqlite::params!["sess1"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(message_count, 0);
    }

    #[test]
    fn test_cascade_delete_message_removes_attachments() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["char1", "Test", "Desc", "Prompt", "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z"],
        ).unwrap();

        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["sess1", "char1", "2024-01-01T00:00:00Z"],
        ).unwrap();

        conn.execute(
            "INSERT INTO chat_messages (id, session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["msg1", "sess1", "user", "Hello", "2024-01-01T00:00:00Z"],
        ).unwrap();

        conn.execute(
            "INSERT INTO attachments (id, message_id, file_name, attachment_type, file_path, size_bytes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params!["att1", "msg1", "test.txt", "text", "/path/to/test.txt", 1024, "2024-01-01T00:00:00Z"],
        ).unwrap();

        // メッセージ削除
        conn.execute("DELETE FROM chat_messages WHERE id = ?1", rusqlite::params!["msg1"]).unwrap();

        let attachment_count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM attachments WHERE message_id = ?1",
            rusqlite::params!["msg1"],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(attachment_count, 0);
    }

    #[test]
    fn test_role_check_constraint() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["char1", "Test", "Desc", "Prompt", "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z"],
        ).unwrap();

        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["sess1", "char1", "2024-01-01T00:00:00Z"],
        ).unwrap();

        // 不正なroleでの挿入はエラーになる
        let result = conn.execute(
            "INSERT INTO chat_messages (id, session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["msg1", "sess1", "invalid_role", "Hello", "2024-01-01T00:00:00Z"],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_attachment_type_check_constraint() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["char1", "Test", "Desc", "Prompt", "2024-01-01T00:00:00Z", "2024-01-01T00:00:00Z"],
        ).unwrap();

        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["sess1", "char1", "2024-01-01T00:00:00Z"],
        ).unwrap();

        conn.execute(
            "INSERT INTO chat_messages (id, session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["msg1", "sess1", "user", "Hello", "2024-01-01T00:00:00Z"],
        ).unwrap();

        // 不正なattachment_typeでの挿入はエラーになる
        let result = conn.execute(
            "INSERT INTO attachments (id, message_id, file_name, attachment_type, file_path, size_bytes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params!["att1", "msg1", "test.bin", "binary", "/path/to/test.bin", 1024, "2024-01-01T00:00:00Z"],
        );
        assert!(result.is_err());
    }
}
