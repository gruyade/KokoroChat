// CustomTool repository - カスタムツール CRUD

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CustomToolRecord, CustomToolType};

/// 全カスタムツールを取得
pub fn get_all_custom_tools(conn: &Connection) -> Result<Vec<CustomToolRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, tool_type, description, parameters_schema, config_json, enabled, created_at
         FROM custom_tools ORDER BY name ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let tool_type_str: String = row.get(2)?;
        let params_str: String = row.get(4)?;
        let config_str: String = row.get(5)?;

        Ok(CustomToolRecord {
            id: row.get(0)?,
            name: row.get(1)?,
            tool_type: CustomToolType::from_str(&tool_type_str).unwrap_or(CustomToolType::Http),
            description: row.get(3)?,
            parameters_schema: serde_json::from_str(&params_str).unwrap_or_default(),
            config_json: serde_json::from_str(&config_str).unwrap_or_default(),
            enabled: row.get::<_, i32>(6)? != 0,
            created_at: row.get(7)?,
        })
    })?;

    let mut tools = Vec::new();
    for row in rows {
        tools.push(row?);
    }
    Ok(tools)
}

/// 有効なカスタムツールのみ取得
pub fn get_enabled_custom_tools(conn: &Connection) -> Result<Vec<CustomToolRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, tool_type, description, parameters_schema, config_json, enabled, created_at
         FROM custom_tools WHERE enabled = 1 ORDER BY name ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let tool_type_str: String = row.get(2)?;
        let params_str: String = row.get(4)?;
        let config_str: String = row.get(5)?;

        Ok(CustomToolRecord {
            id: row.get(0)?,
            name: row.get(1)?,
            tool_type: CustomToolType::from_str(&tool_type_str).unwrap_or(CustomToolType::Http),
            description: row.get(3)?,
            parameters_schema: serde_json::from_str(&params_str).unwrap_or_default(),
            config_json: serde_json::from_str(&config_str).unwrap_or_default(),
            enabled: row.get::<_, i32>(6)? != 0,
            created_at: row.get(7)?,
        })
    })?;

    let mut tools = Vec::new();
    for row in rows {
        tools.push(row?);
    }
    Ok(tools)
}

/// カスタムツールを名前で取得
pub fn get_custom_tool_by_name(
    conn: &Connection,
    name: &str,
) -> Result<Option<CustomToolRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, tool_type, description, parameters_schema, config_json, enabled, created_at
         FROM custom_tools WHERE name = ?1",
    )?;

    let mut rows = stmt.query_map(params![name], |row| {
        let tool_type_str: String = row.get(2)?;
        let params_str: String = row.get(4)?;
        let config_str: String = row.get(5)?;

        Ok(CustomToolRecord {
            id: row.get(0)?,
            name: row.get(1)?,
            tool_type: CustomToolType::from_str(&tool_type_str).unwrap_or(CustomToolType::Http),
            description: row.get(3)?,
            parameters_schema: serde_json::from_str(&params_str).unwrap_or_default(),
            config_json: serde_json::from_str(&config_str).unwrap_or_default(),
            enabled: row.get::<_, i32>(6)? != 0,
            created_at: row.get(7)?,
        })
    })?;

    match rows.next() {
        Some(Ok(record)) => Ok(Some(record)),
        Some(Err(e)) => Err(AppError::Database(e.to_string())),
        None => Ok(None),
    }
}

/// カスタムツールを作成
pub fn create_custom_tool(conn: &Connection, record: &CustomToolRecord) -> Result<(), AppError> {
    let params_str = serde_json::to_string(&record.parameters_schema)?;
    let config_str = serde_json::to_string(&record.config_json)?;

    conn.execute(
        "INSERT INTO custom_tools (id, name, tool_type, description, parameters_schema, config_json, enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            record.id,
            record.name,
            record.tool_type.as_str(),
            record.description,
            params_str,
            config_str,
            record.enabled as i32,
            record.created_at,
        ],
    )?;
    Ok(())
}

/// カスタムツールを更新
pub fn update_custom_tool(conn: &Connection, record: &CustomToolRecord) -> Result<(), AppError> {
    let params_str = serde_json::to_string(&record.parameters_schema)?;
    let config_str = serde_json::to_string(&record.config_json)?;

    let affected = conn.execute(
        "UPDATE custom_tools SET name = ?1, tool_type = ?2, description = ?3,
         parameters_schema = ?4, config_json = ?5, enabled = ?6
         WHERE id = ?7",
        params![
            record.name,
            record.tool_type.as_str(),
            record.description,
            params_str,
            config_str,
            record.enabled as i32,
            record.id,
        ],
    )?;

    if affected == 0 {
        return Err(AppError::NotFound(format!(
            "Custom tool '{}' not found",
            record.id
        )));
    }
    Ok(())
}

/// カスタムツールを削除
pub fn delete_custom_tool(conn: &Connection, id: &str) -> Result<(), AppError> {
    let affected = conn.execute("DELETE FROM custom_tools WHERE id = ?1", params![id])?;

    if affected == 0 {
        return Err(AppError::NotFound(format!(
            "Custom tool '{}' not found",
            id
        )));
    }
    Ok(())
}

/// 新しいカスタムツールレコードを生成するヘルパー
pub fn new_custom_tool_record(
    name: String,
    tool_type: CustomToolType,
    description: String,
    parameters_schema: serde_json::Value,
    config_json: serde_json::Value,
) -> CustomToolRecord {
    CustomToolRecord {
        id: Uuid::new_v4().to_string(),
        name,
        tool_type,
        description,
        parameters_schema,
        config_json,
        enabled: true,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::database::Database;
    use serde_json::json;

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn test_create_and_get_custom_tool() {
        let db = setup_db();
        let conn = db.connection();

        let record = new_custom_tool_record(
            "get_weather".to_string(),
            CustomToolType::Http,
            "天気を取得する".to_string(),
            json!({"type": "object", "properties": {"location": {"type": "string"}}}),
            json!({"url": "https://api.example.com/weather", "method": "POST", "headers": {}}),
        );

        create_custom_tool(conn, &record).unwrap();

        let tools = get_all_custom_tools(conn).unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "get_weather");
        assert_eq!(tools[0].tool_type, CustomToolType::Http);
    }

    #[test]
    fn test_get_by_name() {
        let db = setup_db();
        let conn = db.connection();

        let record = new_custom_tool_record(
            "read_file".to_string(),
            CustomToolType::Cli,
            "ファイルを読む".to_string(),
            json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            json!({"command": "cat", "args": ["{{path}}"]}),
        );

        create_custom_tool(conn, &record).unwrap();

        let found = get_custom_tool_by_name(conn, "read_file").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().tool_type, CustomToolType::Cli);

        let not_found = get_custom_tool_by_name(conn, "nonexistent").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_get_enabled_only() {
        let db = setup_db();
        let conn = db.connection();

        let mut record1 = new_custom_tool_record(
            "tool_a".to_string(),
            CustomToolType::Http,
            "Tool A".to_string(),
            json!({"type": "object"}),
            json!({"url": "http://a.com"}),
        );
        record1.enabled = true;

        let mut record2 = new_custom_tool_record(
            "tool_b".to_string(),
            CustomToolType::Cli,
            "Tool B".to_string(),
            json!({"type": "object"}),
            json!({"command": "echo"}),
        );
        record2.enabled = false;

        create_custom_tool(conn, &record1).unwrap();
        create_custom_tool(conn, &record2).unwrap();

        let all = get_all_custom_tools(conn).unwrap();
        assert_eq!(all.len(), 2);

        let enabled = get_enabled_custom_tools(conn).unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "tool_a");
    }

    #[test]
    fn test_update_custom_tool() {
        let db = setup_db();
        let conn = db.connection();

        let mut record = new_custom_tool_record(
            "updatable".to_string(),
            CustomToolType::Http,
            "Original".to_string(),
            json!({"type": "object"}),
            json!({"url": "http://old.com"}),
        );

        create_custom_tool(conn, &record).unwrap();

        record.description = "Updated".to_string();
        record.config_json = json!({"url": "http://new.com"});
        update_custom_tool(conn, &record).unwrap();

        let found = get_custom_tool_by_name(conn, "updatable").unwrap().unwrap();
        assert_eq!(found.description, "Updated");
    }

    #[test]
    fn test_delete_custom_tool() {
        let db = setup_db();
        let conn = db.connection();

        let record = new_custom_tool_record(
            "deletable".to_string(),
            CustomToolType::Cli,
            "To delete".to_string(),
            json!({"type": "object"}),
            json!({"command": "rm"}),
        );

        create_custom_tool(conn, &record).unwrap();
        assert_eq!(get_all_custom_tools(conn).unwrap().len(), 1);

        delete_custom_tool(conn, &record.id).unwrap();
        assert_eq!(get_all_custom_tools(conn).unwrap().len(), 0);
    }

    #[test]
    fn test_delete_nonexistent_fails() {
        let db = setup_db();
        let conn = db.connection();

        let result = delete_custom_tool(conn, "nonexistent-id");
        assert!(result.is_err());
    }
}
