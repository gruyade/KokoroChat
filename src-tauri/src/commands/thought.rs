// Thought Tauri Commands — 思考閲覧

use tauri::State;

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
