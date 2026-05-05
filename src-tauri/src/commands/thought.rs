// Thought Tauri Commands — 思考閲覧・エンジン制御・削除

use tauri::{AppHandle, State};

use crate::db::repositories::thought as thought_repo;
use crate::error::AppError;
use crate::models::Thought;
use crate::state::AppState;

/// キャラクターの思考履歴取得
#[tauri::command]
pub async fn get_thoughts(
    character_id: String,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<Thought>, AppError> {
    state.thought_engine.get_thoughts(&character_id, limit).await
}

/// 思考を1件削除
#[tauri::command]
pub async fn delete_thought(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let db_guard = state.db.lock().map_err(|e| {
        AppError::Database(format!("DB lock failed: {}", e))
    })?;
    let conn = db_guard.connection();
    let deleted = thought_repo::delete_thought(conn, &id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("thought {}", id)));
    }
    Ok(())
}

/// 思考エンジン起動（キャラクター選択時にフロントエンドから呼ぶ）
#[tauri::command]
pub async fn start_thought_engine(
    character_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let config = state.config_manager.get_config();
    if !config.thought.enabled {
        return Ok(());
    }
    state.thought_engine.set_frequency(&character_id, config.thought.interval_minutes);
    state.thought_engine.start(&character_id, app_handle);
    println!("[thought] engine started for character={}, interval={}min", character_id, config.thought.interval_minutes);
    Ok(())
}

/// 思考エンジン停止
#[tauri::command]
pub async fn stop_thought_engine(
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.thought_engine.stop();
    println!("[thought] engine stopped");
    Ok(())
}
