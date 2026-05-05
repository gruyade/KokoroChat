// Character Tauri Commands — キャラクターCRUD操作

use tauri::State;

use crate::error::AppError;
use crate::models::{Character, CharacterUpdate};
use crate::state::AppState;

/// キャラクター新規作成
///
/// LLMでSystem Promptを自動生成し、DBに保存して返却する。
#[tauri::command]
pub async fn create_character(
    name: String,
    description: String,
    state: State<'_, AppState>,
) -> Result<Character, AppError> {
    let creator = &state.character_creator;

    // LLMでシステムプロンプト生成
    let system_prompt = creator.generate_system_prompt(&name, &description).await?;

    let now = chrono::Utc::now().to_rfc3339();
    let character = Character {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        description,
        system_prompt,
        avatar_path: None,
        tts_config: None,
        created_at: now.clone(),
        updated_at: now,
    };

    creator.save_character(&character).await?;

    Ok(character)
}

/// 全キャラクター一覧取得
#[tauri::command]
pub async fn list_characters(
    state: State<'_, AppState>,
) -> Result<Vec<Character>, AppError> {
    state.character_creator.list_characters().await
}

/// IDでキャラクター取得
#[tauri::command]
pub async fn get_character(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<Character>, AppError> {
    state.character_creator.get_character(&id).await
}

/// キャラクター部分更新
#[tauri::command]
pub async fn update_character(
    id: String,
    updates: CharacterUpdate,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.character_creator.update_character(&id, updates).await
}

/// キャラクター削除（関連データもCASCADE削除）
#[tauri::command]
pub async fn delete_character(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.character_creator.delete_character(&id).await
}
