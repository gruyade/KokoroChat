// Config Tauri Commands — アプリケーション設定管理

use tauri::State;

use crate::error::AppError;
use crate::llm::client::LLMClientConfig;
use crate::models::config::{AppConfig, ModelSettings};
use crate::state::AppState;

/// 現在のアプリケーション設定を取得
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, AppError> {
    Ok(state.config_manager.get_config())
}

/// アプリケーション設定を更新して永続化
#[tauri::command]
pub async fn set_config(config: AppConfig, state: State<'_, AppState>) -> Result<(), AppError> {
    state.config_manager.set_config(config)
}

/// LLM接続テスト
///
/// ModelSettingsからLLMClientConfigを構築し、接続テストを実行する。
/// 成功すればOk(())、失敗すればAppError::LlmApiを返す。
#[tauri::command]
pub async fn test_llm_connection(
    settings: ModelSettings,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let config = LLMClientConfig {
        base_url: settings.base_url,
        model: settings.model,
        api_key: settings.api_key,
        temperature: settings.temperature,
    };

    state.llm_client.test_connection(&config).await
}
