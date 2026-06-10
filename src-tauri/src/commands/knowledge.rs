// Knowledge Tauri Commands — ナレッジCRUD操作

use std::path::Path;

use tauri::State;
use uuid::Uuid;

use crate::db::repositories::knowledge as knowledge_repo;
use crate::error::AppError;
use crate::models::{KnowledgeEntry, KnowledgeEntryMeta};
use crate::state::AppState;

/// 512KB上限（バイト）
const MAX_KNOWLEDGE_FILE_SIZE: u64 = 512 * 1024;

/// ナレッジ用にファイルパスからUTF-8テキストを読み取る（512KBサイズチェック付き）
#[tauri::command]
pub async fn read_text_file_for_knowledge(file_path: String) -> Result<String, AppError> {
    let path = Path::new(&file_path);

    // ファイルサイズチェック
    let metadata = std::fs::metadata(path)
        .map_err(|e| AppError::Validation(format!("ファイルにアクセスできない: {}", e)))?;
    if metadata.len() > MAX_KNOWLEDGE_FILE_SIZE {
        return Err(AppError::Validation(
            "ファイルサイズが上限(512KB)を超えている".to_string(),
        ));
    }

    // UTF-8テキストとして読み取り
    std::fs::read_to_string(path)
        .map_err(|e| AppError::Validation(format!("UTF-8テキストとして読み取れない: {}", e)))
}

/// ナレッジエントリ追加（session_id + file_name でUPSERT）
#[tauri::command]
pub async fn add_knowledge(
    session_id: String,
    file_name: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<KnowledgeEntryMeta, AppError> {
    let id = Uuid::new_v4().to_string();
    let size_bytes = content.len() as i64;
    let created_at = chrono::Utc::now().to_rfc3339();

    let entry = KnowledgeEntry {
        id: id.clone(),
        session_id: session_id.clone(),
        file_name: file_name.clone(),
        content,
        size_bytes,
        enabled: true,
        injection_mode: "system_prompt".to_string(),
        created_at: created_at.clone(),
    };

    let db = state.db.lock().unwrap();
    let conn = db.connection();
    knowledge_repo::add_knowledge(conn, &entry)?;

    Ok(KnowledgeEntryMeta {
        id,
        file_name,
        size_bytes,
        enabled: true,
        injection_mode: "system_prompt".to_string(),
        created_at,
    })
}

/// ナレッジエントリ削除
#[tauri::command]
pub async fn remove_knowledge(
    session_id: String,
    file_name: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db = state.db.lock().unwrap();
    let conn = db.connection();
    knowledge_repo::remove_knowledge(conn, &session_id, &file_name)
}

/// ナレッジメタデータ一覧取得
#[tauri::command]
pub async fn list_knowledge(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<KnowledgeEntryMeta>, AppError> {
    let db = state.db.lock().unwrap();
    let conn = db.connection();
    knowledge_repo::list_knowledge(conn, &session_id)
}

/// ナレッジ有効/無効切替
#[tauri::command]
pub async fn toggle_knowledge(
    session_id: String,
    file_name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db = state.db.lock().unwrap();
    let conn = db.connection();
    knowledge_repo::toggle_knowledge(conn, &session_id, &file_name, enabled)
}

/// 注入モード変更
#[tauri::command]
pub async fn set_knowledge_injection_mode(
    session_id: String,
    file_name: String,
    injection_mode: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db = state.db.lock().unwrap();
    let conn = db.connection();
    knowledge_repo::set_injection_mode(conn, &session_id, &file_name, &injection_mode)
}

/// ナレッジcontent取得（エクスポート用）
#[tauri::command]
pub async fn export_knowledge(
    session_id: String,
    file_name: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let db = state.db.lock().unwrap();
    let conn = db.connection();
    knowledge_repo::get_knowledge_content(conn, &session_id, &file_name)
}

#[cfg(test)]
mod tests {
    use crate::db::database::Database;
    use crate::db::repositories::knowledge as knowledge_repo;
    use crate::models::KnowledgeEntry;

    /// テスト用DBセットアップ（キャラクター＋セッション作成済み）
    fn setup_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        let conn = db.connection();

        conn.execute(
            "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
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
            "INSERT INTO chat_sessions (id, character_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["sess-001", "char-001", "2024-01-01T00:00:00Z"],
        )
        .unwrap();

        db
    }

    // ===== 正常系テスト =====

    /// add_knowledge コマンドと同等のロジック: content.len() で size_bytes 計算
    #[test]
    fn test_add_knowledge_calculates_size_bytes_from_content() {
        let db = setup_db();
        let conn = db.connection();

        let content = "Hello, World!";
        let size_bytes = content.len() as i64;

        let entry = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "test.txt".to_string(),
            content: content.to_string(),
            size_bytes,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        knowledge_repo::add_knowledge(conn, &entry).unwrap();

        let list = knowledge_repo::list_knowledge(conn, "sess-001").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].size_bytes, 13); // "Hello, World!" = 13 bytes
    }

    /// add_knowledge コマンドのデフォルト値: enabled=true, injection_mode="system_prompt"
    #[test]
    fn test_add_knowledge_defaults_enabled_true_and_system_prompt() {
        let db = setup_db();
        let conn = db.connection();

        let content = "Some content";
        let entry = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "defaults.txt".to_string(),
            content: content.to_string(),
            size_bytes: content.len() as i64,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        knowledge_repo::add_knowledge(conn, &entry).unwrap();

        let list = knowledge_repo::list_knowledge(conn, "sess-001").unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].enabled);
        assert_eq!(list[0].injection_mode, "system_prompt");
    }

    /// マルチバイト文字列の size_bytes はUTF-8バイト長
    #[test]
    fn test_add_knowledge_multibyte_size_bytes() {
        let db = setup_db();
        let conn = db.connection();

        let content = "日本語テスト"; // 6文字 × 3バイト = 18バイト
        let size_bytes = content.len() as i64;

        let entry = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "jp.txt".to_string(),
            content: content.to_string(),
            size_bytes,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        knowledge_repo::add_knowledge(conn, &entry).unwrap();

        let list = knowledge_repo::list_knowledge(conn, "sess-001").unwrap();
        assert_eq!(list[0].size_bytes, 18);
    }

    /// list_knowledge は正しいメタデータを返す（content除外）
    #[test]
    fn test_list_knowledge_returns_meta_fields() {
        let db = setup_db();
        let conn = db.connection();

        let entry = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "meta_test.txt".to_string(),
            content: "content data".to_string(),
            size_bytes: 12,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-06-15T10:30:00Z".to_string(),
        };

        knowledge_repo::add_knowledge(conn, &entry).unwrap();

        let list = knowledge_repo::list_knowledge(conn, "sess-001").unwrap();
        assert_eq!(list.len(), 1);

        let meta = &list[0];
        assert_eq!(meta.id, "know-001");
        assert_eq!(meta.file_name, "meta_test.txt");
        assert_eq!(meta.size_bytes, 12);
        assert!(meta.enabled);
        assert_eq!(meta.injection_mode, "system_prompt");
        assert_eq!(meta.created_at, "2024-06-15T10:30:00Z");
    }

    /// export_knowledge と同等のロジック: content取得
    #[test]
    fn test_export_knowledge_returns_content() {
        let db = setup_db();
        let conn = db.connection();

        let entry = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "export.txt".to_string(),
            content: "Export this content".to_string(),
            size_bytes: 19,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        knowledge_repo::add_knowledge(conn, &entry).unwrap();

        let content =
            knowledge_repo::get_knowledge_content(conn, "sess-001", "export.txt").unwrap();
        assert_eq!(content, "Export this content");
    }

    // ===== 異常系テスト: 存在しないエントリへの操作 (Req 10.7) =====

    /// remove_knowledge: 存在しないエントリ → NotFoundエラー
    #[test]
    fn test_remove_knowledge_nonexistent_entry_returns_error() {
        let db = setup_db();
        let conn = db.connection();

        let result = knowledge_repo::remove_knowledge(conn, "sess-001", "nonexistent.txt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            crate::error::AppError::NotFound(msg) => {
                assert!(msg.contains("見つからない"));
            }
            _ => panic!("Expected NotFound error, got: {:?}", err),
        }
    }

    /// toggle_knowledge: 存在しないエントリ → NotFoundエラー
    #[test]
    fn test_toggle_knowledge_nonexistent_entry_returns_error() {
        let db = setup_db();
        let conn = db.connection();

        let result = knowledge_repo::toggle_knowledge(conn, "sess-001", "ghost.txt", false);
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            crate::error::AppError::NotFound(msg) => {
                assert!(msg.contains("見つからない"));
            }
            _ => panic!("Expected NotFound error, got: {:?}", err),
        }
    }

    /// set_injection_mode: 存在しないエントリ → NotFoundエラー
    #[test]
    fn test_set_injection_mode_nonexistent_entry_returns_error() {
        let db = setup_db();
        let conn = db.connection();

        let result =
            knowledge_repo::set_injection_mode(conn, "sess-001", "missing.txt", "system_prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            crate::error::AppError::NotFound(msg) => {
                assert!(msg.contains("見つからない"));
            }
            _ => panic!("Expected NotFound error, got: {:?}", err),
        }
    }

    /// export_knowledge: 存在しないエントリ → NotFoundエラー
    #[test]
    fn test_export_knowledge_nonexistent_entry_returns_error() {
        let db = setup_db();
        let conn = db.connection();

        let result = knowledge_repo::get_knowledge_content(conn, "sess-001", "no_such_file.txt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            crate::error::AppError::NotFound(msg) => {
                assert!(msg.contains("見つからない"));
            }
            _ => panic!("Expected NotFound error, got: {:?}", err),
        }
    }

    // ===== 異常系テスト: 無効な injection_mode (Req 10.8) =====

    /// set_injection_mode: 無効な値 → Validationエラー
    #[test]
    fn test_set_injection_mode_invalid_value_returns_validation_error() {
        let db = setup_db();
        let conn = db.connection();

        // まずエントリを追加
        let entry = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "test.txt".to_string(),
            content: "content".to_string(),
            size_bytes: 7,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        knowledge_repo::add_knowledge(conn, &entry).unwrap();

        let result = knowledge_repo::set_injection_mode(conn, "sess-001", "test.txt", "invalid");
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            crate::error::AppError::Validation(msg) => {
                assert!(msg.contains("system_prompt"));
                assert!(msg.contains("tool_reference"));
            }
            _ => panic!("Expected Validation error, got: {:?}", err),
        }
    }

    /// set_injection_mode: 空文字列 → Validationエラー
    #[test]
    fn test_set_injection_mode_empty_string_returns_validation_error() {
        let db = setup_db();
        let conn = db.connection();

        let entry = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "test.txt".to_string(),
            content: "content".to_string(),
            size_bytes: 7,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        knowledge_repo::add_knowledge(conn, &entry).unwrap();

        let result = knowledge_repo::set_injection_mode(conn, "sess-001", "test.txt", "");
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            crate::error::AppError::Validation(msg) => {
                assert!(msg.contains("system_prompt"));
                assert!(msg.contains("tool_reference"));
            }
            _ => panic!("Expected Validation error, got: {:?}", err),
        }
    }

    /// set_injection_mode: 大文字バリエーション → Validationエラー（大文字小文字区別）
    #[test]
    fn test_set_injection_mode_case_sensitive_validation() {
        let db = setup_db();
        let conn = db.connection();

        let entry = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "test.txt".to_string(),
            content: "content".to_string(),
            size_bytes: 7,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        knowledge_repo::add_knowledge(conn, &entry).unwrap();

        // "System_Prompt" (大文字混在) は無効
        let result =
            knowledge_repo::set_injection_mode(conn, "sess-001", "test.txt", "System_Prompt");
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            crate::error::AppError::Validation(_) => {} // 期待通り
            _ => panic!("Expected Validation error, got: {:?}", err),
        }
    }

    /// set_injection_mode: 有効な値("tool_reference") → 成功
    #[test]
    fn test_set_injection_mode_valid_tool_reference_succeeds() {
        let db = setup_db();
        let conn = db.connection();

        let entry = KnowledgeEntry {
            id: "know-001".to_string(),
            session_id: "sess-001".to_string(),
            file_name: "test.txt".to_string(),
            content: "content".to_string(),
            size_bytes: 7,
            enabled: true,
            injection_mode: "system_prompt".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        knowledge_repo::add_knowledge(conn, &entry).unwrap();

        let result =
            knowledge_repo::set_injection_mode(conn, "sess-001", "test.txt", "tool_reference");
        assert!(result.is_ok());

        let list = knowledge_repo::list_knowledge(conn, "sess-001").unwrap();
        assert_eq!(list[0].injection_mode, "tool_reference");
    }
}
