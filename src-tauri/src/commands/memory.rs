// Memory Tauri Commands — 記憶CRUD操作

use tauri::State;

use crate::error::AppError;
use crate::memory::manager::MemoryManager;
use crate::models::Memory;
use crate::state::AppState;

/// キャラクターの記憶一覧取得
#[tauri::command]
pub async fn list_memories(
    character_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Memory>, AppError> {
    state.memory_manager.list_memories(&character_id).await
}

/// 記憶の内容を更新
#[tauri::command]
pub async fn update_memory(
    id: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.memory_manager.update_memory(&id, &content).await
}

/// 記憶を削除
#[tauri::command]
pub async fn delete_memory(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.memory_manager.delete_memory(&id).await
}
