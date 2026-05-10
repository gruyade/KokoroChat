// Plugin Tauri Commands — プラグイン管理操作

use tauri::State;

use crate::db::repositories::chat_plugin_config;
use crate::error::AppError;
use crate::models::chat::ChatPluginConfig;
use crate::models::plugin::PluginInfo;
#[allow(unused_imports)]
use crate::plugin::registry::PluginRegistry;
use crate::state::{AppState, FileOpsStateManager};

/// 登録済みプラグイン一覧取得
#[tauri::command]
pub async fn list_plugins(state: State<'_, AppState>) -> Result<Vec<PluginInfo>, AppError> {
    Ok(state.plugin_registry.list_plugins())
}

/// プラグイン有効化
#[tauri::command]
pub async fn enable_plugin(name: String, state: State<'_, AppState>) -> Result<(), AppError> {
    state.plugin_registry.enable_plugin(&name)?;

    // AppConfigに反映してディスクへ永続化
    let mut app_config = state.config_manager.get_config();
    if !app_config.plugins.enabled_plugins.contains(&name) {
        app_config.plugins.enabled_plugins.push(name);
    }
    state.config_manager.set_config(app_config)?;

    Ok(())
}

/// プラグイン無効化
#[tauri::command]
pub async fn disable_plugin(name: String, state: State<'_, AppState>) -> Result<(), AppError> {
    state.plugin_registry.disable_plugin(&name)?;

    // AppConfigに反映してディスクへ永続化
    let mut app_config = state.config_manager.get_config();
    app_config.plugins.enabled_plugins.retain(|p| p != &name);
    state.config_manager.set_config(app_config)?;

    Ok(())
}

/// プラグイン固有設定取得
#[tauri::command]
pub async fn get_plugin_config(
    name: String,
    state: State<'_, AppState>,
) -> Result<Option<serde_json::Value>, AppError> {
    Ok(state.plugin_registry.get_plugin_config(&name))
}

/// プラグイン固有設定更新
#[tauri::command]
pub async fn set_plugin_config(
    name: String,
    config: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    // インメモリのPluginRegistryを更新
    state
        .plugin_registry
        .set_plugin_config(&name, config.clone())?;

    // AppConfigに反映してディスクへ永続化
    let mut app_config = state.config_manager.get_config();
    app_config
        .plugins
        .plugin_settings
        .insert(name.clone(), config);
    state.config_manager.set_config(app_config)?;

    Ok(())
}

/// セッション固有プラグイン設定取得（chat_plugin_configs テーブル）
#[tauri::command]
pub async fn get_session_plugin_config(
    session_id: String,
    plugin_name: String,
    state: State<'_, AppState>,
) -> Result<Option<ChatPluginConfig>, AppError> {
    let db = state.db.lock().unwrap();
    let conn = db.connection();
    chat_plugin_config::get_config(conn, &session_id, &plugin_name)
}

/// セッション固有プラグイン設定更新（chat_plugin_configs テーブル）
#[tauri::command]
pub async fn update_session_plugin_config(
    session_id: String,
    plugin_name: String,
    config_json: String,
    state: State<'_, AppState>,
) -> Result<ChatPluginConfig, AppError> {
    let db = state.db.lock().unwrap();
    let conn = db.connection();
    chat_plugin_config::upsert_config(conn, &session_id, &plugin_name, &config_json)
}

/// ファイル操作アクセス許可リクエストを解決する
///
/// フロントエンドのダイアログからユーザーの許可/拒否結果を受け取り、
/// 待機中の file_ops プラグインを再開させる。
#[tauri::command]
pub async fn resolve_file_ops_access(
    request_id: String,
    granted: bool,
    state: State<'_, FileOpsStateManager>,
) -> Result<(), AppError> {
    let sender = {
        let mut pending = state.pending_requests.lock().await;
        pending.remove(&request_id)
    };

    match sender {
        Some(tx) => {
            // send が Err になるのは Receiver がドロップ済みの場合のみ
            let _ = tx.send(granted);
            Ok(())
        }
        None => Err(AppError::NotFound(format!(
            "file_ops access request not found: {}",
            request_id
        ))),
    }
}
