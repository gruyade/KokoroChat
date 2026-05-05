// Character Tauri Commands — キャラクターCRUD操作

use tauri::State;

use crate::error::AppError;
use crate::models::{Character, CharacterUpdate};
use crate::state::AppState;

/// キャラクター新規作成
///
/// system_promptが指定されていればそれを使用、未指定ならLLMで自動生成。
#[tauri::command]
pub async fn create_character(
    name: String,
    description: String,
    system_prompt: Option<String>,
    state: State<'_, AppState>,
) -> Result<Character, AppError> {
    let creator = &state.character_creator;

    // system_promptが空でなければそのまま使用、なければLLMで生成
    let final_prompt = match system_prompt {
        Some(ref p) if !p.trim().is_empty() => p.clone(),
        _ => creator.generate_system_prompt(&name, &description).await?,
    };

    let now = chrono::Utc::now().to_rfc3339();
    let character = Character {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        description,
        system_prompt: final_prompt,
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

/// System Prompt生成（説明内容から新規作成）
#[tauri::command]
pub async fn generate_system_prompt(
    name: String,
    description: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    state.character_creator.generate_system_prompt(&name, &description).await
}

/// System Prompt改善（既存プロンプト + 説明内容から改良）
#[tauri::command]
pub async fn improve_system_prompt(
    name: String,
    description: String,
    current_prompt: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    use crate::llm::client::{ChatMessage, LLMClientConfig, LLMResponse, MessageRole};
    use crate::models::config::ModelPurpose;

    // キャラクター生成用のLLM設定を取得
    let llm_config = state.config_manager
        .get_model_settings(&ModelPurpose::CharacterGeneration)
        .map(|s| LLMClientConfig {
            base_url: s.base_url,
            model: s.model,
            api_key: s.api_key,
            temperature: s.temperature,
        })
        .unwrap_or_else(|| LLMClientConfig {
            base_url: String::new(),
            model: String::new(),
            api_key: None,
            temperature: 0.7,
        });

    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: format!(
            "あなたはAIキャラクター設計の専門家です。\n\
             以下の既存System Promptを改善してください。\n\n\
             【キャラクター名】{}\n\
             【概要説明】{}\n\
             【現在のSystem Prompt】\n{}\n\n\
             以下の観点で改善してください：\n\
             1. キャラクターの性格・人格をより具体的に\n\
             2. 話し方・口調のパターンをより明確に\n\
             3. 行動原理・価値観を追加\n\
             4. 会話における振る舞いのガイドラインを充実\n\
             5. 矛盾や曖昧な部分を解消\n\n\
             改善後のSystem Promptのみを出力してください。説明や前置きは不要です。",
            name, description, current_prompt
        ),
        tool_call_id: None,
    }];

    let response = state.llm_client.chat(&messages, &llm_config, None).await?;

    match response {
        LLMResponse::Text(text) => Ok(text),
        LLMResponse::ToolCalls(_) => Err(AppError::LlmApi(
            "Unexpected tool_call response during prompt improvement".to_string(),
        )),
    }
}
