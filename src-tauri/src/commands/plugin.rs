// Plugin Tauri Commands — プラグイン管理操作

use tauri::State;

use crate::error::AppError;
use crate::models::plugin::PluginInfo;
#[allow(unused_imports)]
use crate::plugin::registry::PluginRegistry;
use crate::state::AppState;

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
