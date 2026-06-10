// Knowledge repository - CRUD操作

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::models::{KnowledgeEntry, KnowledgeEntryMeta};

/// 512KB上限（バイト）
const MAX_CONTENT_SIZE: usize = 512 * 1024;

/// ナレッジエントリを追加（UPSERT: 同一session_id+file_nameは上書き）
pub fn add_knowledge(conn: &Connection, entry: &KnowledgeEntry) -> Result<(), AppError> {
    // 512KB超過チェック
    if entry.content.len() > MAX_CONTENT_SIZE {
        return Err(AppError::Validation(
            "ファイルサイズが上限(512KB)を超えている".to_string(),
        ));
    }

    conn.execute(
        "INSERT OR REPLACE INTO session_knowledge (id, session_id, file_name, content, size_bytes, enabled, injection_mode, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            entry.id,
            entry.session_id,
            entry.file_name,
            entry.content,
            entry.size_bytes,
            entry.enabled,
            entry.injection_mode,
            entry.created_at,
        ],
    )?;
    Ok(())
}

/// session_id + file_name でナレッジエントリを削除
pub fn remove_knowledge(
    conn: &Connection,
    session_id: &str,
    file_name: &str,
) -> Result<(), AppError> {
    let rows_affected = conn.execute(
        "DELETE FROM session_knowledge WHERE session_id = ?1 AND file_name = ?2",
        params![session_id, file_name],
    )?;

    if rows_affected == 0 {
        return Err(AppError::NotFound(
            "指定されたナレッジエントリが見つからない".to_string(),
        ));
    }
    Ok(())
}

/// session_id でメタデータ一覧取得（content除外、created_at昇順）
pub fn list_knowledge(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<KnowledgeEntryMeta>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, file_name, size_bytes, enabled, injection_mode, created_at
         FROM session_knowledge WHERE session_id = ?1
         ORDER BY created_at ASC",
    )?;

    let rows = stmt.query_map(params![session_id], |row| {
        let enabled_int: i32 = row.get(3)?;
        Ok(KnowledgeEntryMeta {
            id: row.get(0)?,
            file_name: row.get(1)?,
            size_bytes: row.get(2)?,
            enabled: enabled_int != 0,
            injection_mode: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

/// enabled フラグを更新
pub fn toggle_knowledge(
    conn: &Connection,
    session_id: &str,
    file_name: &str,
    enabled: bool,
) -> Result<(), AppError> {
    let rows_affected = conn.execute(
        "UPDATE session_knowledge SET enabled = ?1 WHERE session_id = ?2 AND file_name = ?3",
        params![enabled, session_id, file_name],
    )?;

    if rows_affected == 0 {
        return Err(AppError::NotFound(
            "指定されたナレッジエントリが見つからない".to_string(),
        ));
    }
    Ok(())
}

/// injection_mode を更新（値バリデーション付き）
pub fn set_injection_mode(
    conn: &Connection,
    session_id: &str,
    file_name: &str,
    mode: &str,
) -> Result<(), AppError> {
    // バリデーション
    if mode != "system_prompt" && mode != "tool_reference" {
        return Err(AppError::Validation(
            "injection_modeは'system_prompt'または'tool_reference'のみ許可".to_string(),
        ));
    }

    let rows_affected = conn.execute(
        "UPDATE session_knowledge SET injection_mode = ?1 WHERE session_id = ?2 AND file_name = ?3",
        params![mode, session_id, file_name],
    )?;

    if rows_affected == 0 {
        return Err(AppError::NotFound(
            "指定されたナレッジエントリが見つからない".to_string(),
        ));
    }
    Ok(())
}

/// session_id + file_name でcontent取得
pub fn get_knowledge_content(
    conn: &Connection,
    session_id: &str,
    file_name: &str,
) -> Result<String, AppError> {
    let content = conn
        .query_row(
            "SELECT content FROM session_knowledge WHERE session_id = ?1 AND file_name = ?2",
            params![session_id, file_name],
            |row| row.get::<_, String>(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("指定されたナレッジエントリが見つからない".to_string())
            }
            _ => AppError::Database(e.to_string()),
        })?;

    Ok(content)
}

/// enabled=true かつ injection_mode=system_prompt のエントリ取得
pub fn get_system_prompt_entries(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<KnowledgeEntry>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, file_name, content, size_bytes, enabled, injection_mode, created_at
         FROM session_knowledge
         WHERE session_id = ?1 AND enabled = 1 AND injection_mode = 'system_prompt'
         ORDER BY created_at ASC",
    )?;

    let rows = stmt.query_map(params![session_id], |row| {
        let enabled_int: i32 = row.get(5)?;
        Ok(KnowledgeEntry {
            id: row.get(0)?,
            session_id: row.get(1)?,
            file_name: row.get(2)?,
            content: row.get(3)?,
            size_bytes: row.get(4)?,
            enabled: enabled_int != 0,
            injection_mode: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

/// enabled=true かつ injection_mode=tool_reference のエントリ取得
pub fn get_tool_reference_entries(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<KnowledgeEntry>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, file_name, content, size_bytes, enabled, injection_mode, created_at
         FROM session_knowledge
         WHERE session_id = ?1 AND enabled = 1 AND injection_mode = 'tool_reference'
         ORDER BY created_at ASC",
    )?;

    let rows = stmt.query_map(params![session_id], |row| {
        let enabled_int: i32 = row.get(5)?;
        Ok(KnowledgeEntry {
            id: row.get(0)?,
            session_id: row.get(1)?,
            file_name: row.get(2)?,
            content: row.get(3)?,
            size_bytes: row.get(4)?,
            enabled: enabled_int != 0,
            injection_mode: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::database::Database;

    fn setup_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        // キャラクター作成
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

        // セッション作成
        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            params!["sess-001", "char-001", "2024-01-01T00:00:00Z"],
        )
        .unwrap();

        db
    }

    fn sample_entry() -> KnowledgeEntry {
        KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "test.txt".to_string(),
            content: "Hello, World!".to_string(),
            size_bytes: 13,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_add_and_list_knowledge() {
        let db = setup_db();
        let conn = db.connection();

        let entry = sample_entry();
        add_knowledge(conn, &entry).unwrap();

        let list = list_knowledge(conn, "sess-001").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].file_name, "test.txt");
        assert_eq!(list[0].size_bytes, 13);
        assert!(list[0].enabled);
        assert_eq!(list[0].injection_mode, "system_prompt");
    }

    #[test]
    fn test_add_knowledge_size_limit() {
        let db = setup_db();
        let conn = db.connection();

        let oversized_content = "x".repeat(MAX_CONTENT_SIZE + 1);
        let entry = KnowledgeEntry {
            id: "know-big".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "big.txt".to_string(),
            content: oversized_content.clone(),
            size_bytes: oversized_content.len() as i64,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let result = add_knowledge(conn, &entry);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_knowledge_upsert() {
        let db = setup_db();
        let conn = db.connection();

        let entry1 = sample_entry();
        add_knowledge(conn, &entry1).unwrap();

        // 同じsession_id + file_nameで上書き
        let entry2 = KnowledgeEntry {
            id: "know-002".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "test.txt".to_string(),
            content: "Updated content".to_string(),
            size_bytes: 15,
            enabled: true,
            injection_mode: "tool_reference".to_string(),
            created_at: "2024-01-02T00:00:00Z".to_string(),
        };
        add_knowledge(conn, &entry2).unwrap();

        let list = list_knowledge(conn, "sess-001").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].size_bytes, 15);
        assert_eq!(list[0].injection_mode, "tool_reference");
    }

    #[test]
    fn test_remove_knowledge() {
        let db = setup_db();
        let conn = db.connection();

        let entry = sample_entry();
        add_knowledge(conn, &entry).unwrap();

        remove_knowledge(conn, "sess-001", "test.txt").unwrap();

        let list = list_knowledge(conn, "sess-001").unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_remove_knowledge_not_found() {
        let db = setup_db();
        let conn = db.connection();

        let result = remove_knowledge(conn, "sess-001", "nonexistent.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_toggle_knowledge() {
        let db = setup_db();
        let conn = db.connection();

        let entry = sample_entry();
        add_knowledge(conn, &entry).unwrap();

        toggle_knowledge(conn, "sess-001", "test.txt", false).unwrap();
        let list = list_knowledge(conn, "sess-001").unwrap();
        assert!(!list[0].enabled);

        toggle_knowledge(conn, "sess-001", "test.txt", true).unwrap();
        let list = list_knowledge(conn, "sess-001").unwrap();
        assert!(list[0].enabled);
    }

    #[test]
    fn test_toggle_knowledge_not_found() {
        let db = setup_db();
        let conn = db.connection();

        let result = toggle_knowledge(conn, "sess-001", "nonexistent.txt", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_injection_mode() {
        let db = setup_db();
        let conn = db.connection();

        let entry = sample_entry();
        add_knowledge(conn, &entry).unwrap();

        set_injection_mode(conn, "sess-001", "test.txt", "tool_reference").unwrap();
        let list = list_knowledge(conn, "sess-001").unwrap();
        assert_eq!(list[0].injection_mode, "tool_reference");
    }

    #[test]
    fn test_set_injection_mode_invalid() {
        let db = setup_db();
        let conn = db.connection();

        let entry = sample_entry();
        add_knowledge(conn, &entry).unwrap();

        let result = set_injection_mode(conn, "sess-001", "test.txt", "invalid_mode");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_injection_mode_not_found() {
        let db = setup_db();
        let conn = db.connection();

        let result = set_injection_mode(conn, "sess-001", "nonexistent.txt", "system_prompt");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_knowledge_content() {
        let db = setup_db();
        let conn = db.connection();

        let entry = sample_entry();
        add_knowledge(conn, &entry).unwrap();

        let content = get_knowledge_content(conn, "sess-001", "test.txt").unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_get_knowledge_content_not_found() {
        let db = setup_db();
        let conn = db.connection();

        let result = get_knowledge_content(conn, "sess-001", "nonexistent.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_system_prompt_entries() {
        let db = setup_db();
        let conn = db.connection();

        // system_prompt エントリ追加
        let entry1 = sample_entry();
        add_knowledge(conn, &entry1).unwrap();

        // tool_reference エントリ追加
        let entry2 = KnowledgeEntry {
            id: "know-002".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "ref.txt".to_string(),
            content: "Reference content".to_string(),
            size_bytes: 17,
            enabled: true,
            injection_mode: "tool_reference".to_string(),
            created_at: "2024-01-02T00:00:00Z".to_string(),
        };
        add_knowledge(conn, &entry2).unwrap();

        // disabled system_prompt エントリ追加
        let entry3 = KnowledgeEntry {
            id: "know-003".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "disabled.txt".to_string(),
            content: "Disabled content".to_string(),
            size_bytes: 16,
            enabled: false,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-03T00:00:00Z".to_string(),
        };
        add_knowledge(conn, &entry3).unwrap();

        let entries = get_system_prompt_entries(conn, "sess-001").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name, "test.txt");
        assert_eq!(entries[0].content, "Hello, World!");
    }

    #[test]
    fn test_get_tool_reference_entries() {
        let db = setup_db();
        let conn = db.connection();

        // system_prompt エントリ追加
        let entry1 = sample_entry();
        add_knowledge(conn, &entry1).unwrap();

        // tool_reference エントリ追加
        let entry2 = KnowledgeEntry {
            id: "know-002".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "ref.txt".to_string(),
            content: "Reference content".to_string(),
            size_bytes: 17,
            enabled: true,
            injection_mode: "tool_reference".to_string(),
            created_at: "2024-01-02T00:00:00Z".to_string(),
        };
        add_knowledge(conn, &entry2).unwrap();

        let entries = get_tool_reference_entries(conn, "sess-001").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name, "ref.txt");
        assert_eq!(entries[0].content, "Reference content");
    }

    #[test]
    fn test_list_knowledge_ordered_by_created_at() {
        let db = setup_db();
        let conn = db.connection();

        let entry1 = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "b.txt".to_string(),
            content: "B".to_string(),
            size_bytes: 1,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-02T00:00:00Z".to_string(),
        };
        let entry2 = KnowledgeEntry {
            id: "know-002".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "a.txt".to_string(),
            content: "A".to_string(),
            size_bytes: 1,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        add_knowledge(conn, &entry1).unwrap();
        add_knowledge(conn, &entry2).unwrap();

        let list = list_knowledge(conn, "sess-001").unwrap();
        assert_eq!(list.len(), 2);
        // created_at昇順: a.txt(01-01) → b.txt(01-02)
        assert_eq!(list[0].file_name, "a.txt");
        assert_eq!(list[1].file_name, "b.txt");
    }

    #[test]
    fn test_cascade_delete_session_removes_knowledge_entries() {
        let db = setup_db();
        let conn = db.connection();

        // 複数のknowledgeエントリを追加
        let entry1 = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "file1.txt".to_string(),
            content: "Content 1".to_string(),
            size_bytes: 9,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let entry2 = KnowledgeEntry {
            id: "know-002".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "file2.txt".to_string(),
            content: "Content 2".to_string(),
            size_bytes: 9,
            enabled: true,
            injection_mode: "tool_reference".to_string(),
            created_at: "2024-01-02T00:00:00Z".to_string(),
        };
        let entry3 = KnowledgeEntry {
            id: "know-003".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "file3.txt".to_string(),
            content: "Content 3".to_string(),
            size_bytes: 9,
            enabled: false,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-03T00:00:00Z".to_string(),
        };

        add_knowledge(conn, &entry1).unwrap();
        add_knowledge(conn, &entry2).unwrap();
        add_knowledge(conn, &entry3).unwrap();

        // エントリが存在することを確認
        let list = list_knowledge(conn, "sess-001").unwrap();
        assert_eq!(list.len(), 3);

        // セッションを削除（CASCADE により関連 knowledge エントリも自動削除）
        conn.execute(
            "DELETE FROM chat_sessions WHERE id = ?1",
            params!["sess-001"],
        )
        .unwrap();

        // knowledge エントリが全て削除されていることを確認
        let list_after = list_knowledge(conn, "sess-001").unwrap();
        assert!(list_after.is_empty());

        // 直接 COUNT でも確認
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM session_knowledge WHERE session_id = ?1",
                params!["sess-001"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_cascade_delete_session_does_not_affect_other_sessions() {
        let db = setup_db();
        let conn = db.connection();

        // 2つ目のセッションを作成
        conn.execute(
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            params!["sess-002", "char-001", "2024-01-01T00:00:00Z"],
        )
        .unwrap();

        // sess-001 にエントリ追加
        let entry1 = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "file1.txt".to_string(),
            content: "Content 1".to_string(),
            size_bytes: 9,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        add_knowledge(conn, &entry1).unwrap();

        // sess-002 にエントリ追加
        let entry2 = KnowledgeEntry {
            id: "know-002".to_string(),
            session_id: "sess-002".to_string(),
            file_name: "file2.txt".to_string(),
            content: "Content 2".to_string(),
            size_bytes: 9,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        add_knowledge(conn, &entry2).unwrap();

        // sess-001 を削除
        conn.execute(
            "DELETE FROM chat_sessions WHERE id = ?1",
            params!["sess-001"],
        )
        .unwrap();

        // sess-001 のエントリは消えている
        let list1 = list_knowledge(conn, "sess-001").unwrap();
        assert!(list1.is_empty());

        // sess-002 のエントリは残っている
        let list2 = list_knowledge(conn, "sess-002").unwrap();
        assert_eq!(list2.len(), 1);
        assert_eq!(list2[0].file_name, "file2.txt");
    }
}
