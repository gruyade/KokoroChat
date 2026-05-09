// Thought Tauri Commands — 思考閲覧・エンジン制御・削除

use tauri::{AppHandle, State};

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
    state
        .thought_engine
        .get_thoughts(&character_id, limit)
        .await
}

/// 思考を1件削除
#[tauri::command]
pub async fn delete_thought(id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    state.thought_engine.delete_thought(&id).await
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
    state
        .thought_engine
        .set_frequency(&character_id, config.thought.interval_minutes);
    state.thought_engine.start(&character_id, app_handle);
    println!(
        "[thought] engine started for character={}, interval={}min",
        character_id, config.thought.interval_minutes
    );
    Ok(())
}

/// 思考エンジン停止
#[tauri::command]
pub async fn stop_thought_engine(state: State<'_, AppState>) -> Result<(), AppError> {
    state.thought_engine.stop();
    println!("[thought] engine stopped");
    Ok(())
}

/// 思考エンジン一時停止
#[tauri::command]
pub async fn pause_thought_engine(state: State<'_, AppState>) -> Result<(), AppError> {
    state.thought_engine.pause();
    Ok(())
}

/// 思考エンジン再開
#[tauri::command]
pub async fn resume_thought_engine(state: State<'_, AppState>) -> Result<(), AppError> {
    state.thought_engine.resume();
    Ok(())
}

/// 自発的発話一時停止
#[tauri::command]
pub async fn pause_spontaneous(state: State<'_, AppState>) -> Result<(), AppError> {
    use std::sync::atomic::Ordering;
    state.spontaneous_paused.store(true, Ordering::SeqCst);
    println!("[spontaneous] paused");
    Ok(())
}

/// 自発的発話再開
#[tauri::command]
pub async fn resume_spontaneous(state: State<'_, AppState>) -> Result<(), AppError> {
    use std::sync::atomic::Ordering;
    state.spontaneous_paused.store(false, Ordering::SeqCst);
    println!("[spontaneous] resumed");
    Ok(())
}
