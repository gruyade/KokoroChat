// Character repository - CRUD操作

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::models::{Character, CharacterUpdate};

/// キャラクターをDBに挿入
pub fn insert_character(conn: &Connection, character: &Character) -> Result<(), AppError> {
    let tts_config_json = character
        .tts_config
        .as_ref()
        .map(|c| serde_json::to_string(c))
        .transpose()?;

    conn.execute(
        "INSERT INTO characters (id, name, description, system_prompt, avatar_path, tts_config, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            character.id,
            character.name,
            character.description,
            character.system_prompt,
            character.avatar_path,
            tts_config_json,
            character.created_at,
            character.updated_at,
        ],
    )?;
    Ok(())
}

/// IDでキャラクターを取得
pub fn get_character(conn: &Connection, id: &str) -> Result<Option<Character>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, system_prompt, avatar_path, tts_config, created_at, updated_at
         FROM characters WHERE id = ?1",
    )?;

    let mut rows = stmt.query(params![id])?;
    match rows.next()? {
        Some(row) => {
            let tts_config_str: Option<String> = row.get(5)?;
            let tts_config = tts_config_str
                .map(|s| serde_json::from_str(&s))
                .transpose()?;

            Ok(Some(Character {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                system_prompt: row.get(3)?,
                avatar_path: row.get(4)?,
                tts_config,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            }))
        }
        None => Ok(None),
    }
}

/// 全キャラクター一覧取得
pub fn list_characters(conn: &Connection) -> Result<Vec<Character>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, system_prompt, avatar_path, tts_config, created_at, updated_at
         FROM characters ORDER BY created_at DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        let tts_config_str: Option<String> = row.get(5)?;
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            tts_config_str,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
        ))
    })?;

    let mut characters = Vec::new();
    for row in rows {
        let (id, name, description, system_prompt, avatar_path, tts_config_str, created_at, updated_at) = row?;
        let tts_config = tts_config_str
            .map(|s| serde_json::from_str(&s))
            .transpose()?;

        characters.push(Character {
            id,
            name,
            description,
            system_prompt,
            avatar_path,
            tts_config,
            created_at,
            updated_at,
        });
    }
    Ok(characters)
}

/// キャラクターを部分更新
pub fn update_character(
    conn: &Connection,
    id: &str,
    updates: &CharacterUpdate,
) -> Result<(), AppError> {
    let mut set_clauses = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref name) = updates.name {
        set_clauses.push("name = ?");
        param_values.push(Box::new(name.clone()));
    }
    if let Some(ref description) = updates.description {
        set_clauses.push("description = ?");
        param_values.push(Box::new(description.clone()));
    }
    if let Some(ref system_prompt) = updates.system_prompt {
        set_clauses.push("system_prompt = ?");
        param_values.push(Box::new(system_prompt.clone()));
    }
    if let Some(ref avatar_path) = updates.avatar_path {
        set_clauses.push("avatar_path = ?");
        param_values.push(Box::new(avatar_path.clone()));
    }
    if let Some(ref tts_config) = updates.tts_config {
        set_clauses.push("tts_config = ?");
        let json = serde_json::to_string(tts_config)?;
        param_values.push(Box::new(json));
    }

    if set_clauses.is_empty() {
        return Ok(());
    }

    // updated_atも更新
    set_clauses.push("updated_at = ?");
    param_values.push(Box::new(chrono::Utc::now().to_rfc3339()));

    // WHERE句のIDパラメータ
    param_values.push(Box::new(id.to_string()));

    let sql = format!(
        "UPDATE characters SET {} WHERE id = ?",
        set_clauses.join(", ")
    );

    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    conn.execute(&sql, params_ref.as_slice())?;
    Ok(())
}

/// キャラクターを削除（CASCADE DELETEにより関連データも全削除）
pub fn delete_character(conn: &Connection, id: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM characters WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::database::Database;
    use crate::models::tts::{TTSConfig, TTSProvider};

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn sample_character() -> Character {
        Character {
            id: "char-001".to_string(),
            name: "テストキャラ".to_string(),
            description: "テスト用キャラクター".to_string(),
            system_prompt: "あなたはテストキャラです。".to_string(),
            avatar_path: None,
            tts_config: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_insert_and_get_character() {
        let db = setup_db();
        let conn = db.connection();
        let character = sample_character();

        insert_character(conn, &character).unwrap();
        let result = get_character(conn, "char-001").unwrap();

        assert!(result.is_some());
        let c = result.unwrap();
        assert_eq!(c.id, "char-001");
        assert_eq!(c.name, "テストキャラ");
        assert_eq!(c.description, "テスト用キャラクター");
        assert_eq!(c.system_prompt, "あなたはテストキャラです。");
        assert!(c.avatar_path.is_none());
        assert!(c.tts_config.is_none());
    }

    #[test]
    fn test_get_character_not_found() {
        let db = setup_db();
        let conn = db.connection();

        let result = get_character(conn, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_insert_character_with_tts_config() {
        let db = setup_db();
        let conn = db.connection();
        let mut character = sample_character();
        character.tts_config = Some(TTSConfig {
            provider: TTSProvider::Voicepeak,
            base_url: Some("http://localhost:8080".to_string()),
            reference_audio_path: None,
            caption: None,
            narrator: Some("narrator1".to_string()),
            emotion: None,
            speed: Some(1.0),
            pitch: None,
            irodori_mode: None,
        });

        insert_character(conn, &character).unwrap();
        let result = get_character(conn, "char-001").unwrap().unwrap();

        let tts = result.tts_config.unwrap();
        assert_eq!(tts.provider, TTSProvider::Voicepeak);
        assert_eq!(tts.narrator, Some("narrator1".to_string()));
    }

    #[test]
    fn test_list_characters() {
        let db = setup_db();
        let conn = db.connection();

        let mut c1 = sample_character();
        c1.id = "char-001".to_string();
        c1.created_at = "2024-01-01T00:00:00Z".to_string();

        let mut c2 = sample_character();
        c2.id = "char-002".to_string();
        c2.name = "キャラ2".to_string();
        c2.created_at = "2024-01-02T00:00:00Z".to_string();

        insert_character(conn, &c1).unwrap();
        insert_character(conn, &c2).unwrap();

        let list = list_characters(conn).unwrap();
        assert_eq!(list.len(), 2);
        // DESC順なのでc2が先
        assert_eq!(list[0].id, "char-002");
        assert_eq!(list[1].id, "char-001");
    }

    #[test]
    fn test_update_character() {
        let db = setup_db();
        let conn = db.connection();
        let character = sample_character();
        insert_character(conn, &character).unwrap();

        let updates = CharacterUpdate {
            name: Some("更新後の名前".to_string()),
            description: None,
            system_prompt: None,
            avatar_path: Some("/path/to/avatar.png".to_string()),
            tts_config: None,
        };

        update_character(conn, "char-001", &updates).unwrap();
        let result = get_character(conn, "char-001").unwrap().unwrap();

        assert_eq!(result.name, "更新後の名前");
        assert_eq!(result.avatar_path, Some("/path/to/avatar.png".to_string()));
        // 変更していないフィールドは元のまま
        assert_eq!(result.description, "テスト用キャラクター");
    }

    #[test]
    fn test_update_character_empty_updates() {
        let db = setup_db();
        let conn = db.connection();
        let character = sample_character();
        insert_character(conn, &character).unwrap();

        let updates = CharacterUpdate {
            name: None,
            description: None,
            system_prompt: None,
            avatar_path: None,
            tts_config: None,
        };

        // 空の更新はエラーにならない
        update_character(conn, "char-001", &updates).unwrap();
    }

    #[test]
    fn test_delete_character() {
        let db = setup_db();
        let conn = db.connection();
        let character = sample_character();
        insert_character(conn, &character).unwrap();

        delete_character(conn, "char-001").unwrap();
        let result = get_character(conn, "char-001").unwrap();
        assert!(result.is_none());
    }
}
