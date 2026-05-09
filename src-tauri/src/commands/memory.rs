// Memory Tauri Commands — 記憶CRUD操作

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::error::AppError;
#[allow(unused_imports)]
use crate::memory::manager::MemoryManager;
use crate::models::Memory;
use crate::state::AppState;

/// 記憶生成完了イベント
#[derive(Clone, Serialize)]
pub struct MemoryGeneratedEvent {
    pub character_id: String,
    pub memory_id: String,
}

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
pub async fn delete_memory(id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    state.memory_manager.delete_memory(&id).await
}

/// 手動メモリ生成（閾値チェックをスキップして強制実行）
#[tauri::command]
pub async fn generate_memory_manual(
    session_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let result = state.memory_manager.force_compress(&session_id).await?;

    if let Some(memory) = result {
        if let Err(e) = app_handle.emit(
            "memory:generated",
            MemoryGeneratedEvent {
                character_id: memory.character_id.clone(),
                memory_id: memory.id.clone(),
            },
        ) {
            println!("[memory] Failed to emit memory:generated event: {}", e);
        }
    }

    Ok(())
}
