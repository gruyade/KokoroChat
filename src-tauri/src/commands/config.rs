// Config Tauri Commands — アプリケーション設定管理

use tauri::State;

use crate::error::AppError;
use crate::llm::client::LLMClientConfig;
use crate::models::config::{AppConfig, LLMProvider, ModelSettings};
use crate::state::AppState;

/// OpenAI互換 /models エンドポイントのレスポンス構造
#[derive(serde::Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(serde::Deserialize)]
struct ModelEntry {
    id: String,
}

/// 現在のアプリケーション設定を取得
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, AppError> {
    Ok(state.config_manager.get_config())
}

/// アプリケーション設定を更新して永続化
/// plugins セクションは set_plugin_config / enable_plugin / disable_plugin で管理するため、
/// ここでは現在の保存済み plugins を維持する（フロントの古い draft で上書きされるのを防止）。
#[tauri::command]
pub async fn set_config(config: AppConfig, state: State<'_, AppState>) -> Result<(), AppError> {
    let mut merged = config;
    let current = state.config_manager.get_config();
    merged.plugins = current.plugins;
    state.config_manager.set_config(merged)
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
        provider: settings.provider,
    };

    state.llm_client.test_connection(&config).await
}

/// 利用可能なモデル一覧を取得
///
/// プロバイダーに応じたモデル一覧エンドポイントにGETリクエストを送信し、
/// モデルIDの一覧を返却する。
/// - OpenAI/Google/OpenAI互換: {base_url}/models (Authorization: Bearer)
/// - Anthropic: https://api.anthropic.com/v1/models (x-api-key + anthropic-version)
#[tauri::command]
pub async fn fetch_available_models(
    base_url: String,
    api_key: Option<String>,
    provider: Option<LLMProvider>,
) -> Result<Vec<String>, AppError> {
    let client = reqwest::Client::new();

    let is_anthropic = matches!(provider, Some(LLMProvider::Anthropic));

    // Anthropicの場合: base_urlに関係なく公式エンドポイントを使用
    let url = if is_anthropic {
        "https://api.anthropic.com/v1/models".to_string()
    } else {
        format!("{}/models", base_url.trim_end_matches('/'))
    };

    let mut request = client.get(&url);

    if let Some(key) = &api_key {
        if !key.is_empty() {
            if is_anthropic {
                request = request
                    .header("x-api-key", key.as_str())
                    .header("anthropic-version", "2023-06-01");
            } else {
                request = request.header("Authorization", format!("Bearer {}", key));
            }
        }
    }

    let response = request
        .send()
        .await
        .map_err(|e| AppError::Network(format!("モデル一覧の取得に失敗: {}", e)))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(AppError::LlmApi("API Keyが無効".to_string()));
    }

    if !status.is_success() {
        return Err(AppError::LlmApi(format!(
            "モデル一覧の取得に失敗 (HTTP {})",
            status.as_u16()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| AppError::Network(format!("レスポンスの読み取りに失敗: {}", e)))?;

    let models_response: ModelsResponse = serde_json::from_str(&body)
        .map_err(|e| AppError::Serialization(format!("モデル一覧のJSON解析に失敗: {}", e)))?;

    let model_ids: Vec<String> = models_response.data.into_iter().map(|m| m.id).collect();

    Ok(model_ids)
}
