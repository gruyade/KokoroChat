// Plugin Registry - PluginRegistry trait + DefaultPluginRegistry

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use serde_json::Value;

use crate::error::AppError;
use crate::models::plugin::{PluginInfo, ToolCall, ToolDefinition, ToolResult};

use super::system::PluginHandler;

/// プラグインエントリ（ハンドラ + 状態）
struct PluginEntry {
    handler: Arc<dyn PluginHandler>,
    enabled: bool,
    config: Option<Value>,
}

/// プラグインレジストリtrait — 登録・管理・ディスパッチ
#[async_trait]
pub trait PluginRegistry: Send + Sync {
    /// プラグインを登録
    fn register(&self, handler: Box<dyn PluginHandler>) -> Result<(), AppError>;
    /// プラグインを登録解除
    fn unregister(&self, name: &str) -> Result<(), AppError>;
    /// 登録済みプラグイン一覧を取得
    fn list_plugins(&self) -> Vec<PluginInfo>;
    /// プラグインを有効化
    fn enable_plugin(&self, name: &str) -> Result<(), AppError>;
    /// プラグインを無効化
    fn disable_plugin(&self, name: &str) -> Result<(), AppError>;
    /// プラグイン固有設定を取得
    fn get_plugin_config(&self, name: &str) -> Option<Value>;
    /// プラグイン固有設定を更新
    fn set_plugin_config(&self, name: &str, config: Value) -> Result<(), AppError>;
    /// 有効なプラグインの全ツール定義を取得
    fn get_enabled_tools(&self) -> Vec<ToolDefinition>;
    /// tool_callを対応するプラグインで実行
    async fn execute_tool(&self, tool_call: &ToolCall, app_handle: &tauri::AppHandle) -> Result<ToolResult, AppError>;
}

/// デフォルトのPluginRegistry実装（RwLockによるスレッドセーフ管理）
pub struct DefaultPluginRegistry {
    plugins: RwLock<HashMap<String, PluginEntry>>,
}

impl DefaultPluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for DefaultPluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PluginRegistry for DefaultPluginRegistry {
    fn register(&self, handler: Box<dyn PluginHandler>) -> Result<(), AppError> {
        let name = handler.name().to_string();
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| AppError::Plugin(format!("Failed to acquire write lock: {}", e)))?;

        if plugins.contains_key(&name) {
            return Err(AppError::Plugin(format!(
                "Plugin '{}' is already registered",
                name
            )));
        }

        plugins.insert(
            name,
            PluginEntry {
                handler: Arc::from(handler),
                enabled: true,
                config: None,
            },
        );

        Ok(())
    }

    fn unregister(&self, name: &str) -> Result<(), AppError> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| AppError::Plugin(format!("Failed to acquire write lock: {}", e)))?;

        if plugins.remove(name).is_none() {
            return Err(AppError::NotFound(format!("Plugin '{}' not found", name)));
        }

        Ok(())
    }

    fn list_plugins(&self) -> Vec<PluginInfo> {
        let plugins = self.plugins.read().unwrap_or_else(|e| e.into_inner());

        plugins
            .values()
            .map(|entry| PluginInfo {
                name: entry.handler.name().to_string(),
                description: entry.handler.description().to_string(),
                version: "1.0.0".to_string(),
                enabled: entry.enabled,
                tools: entry.handler.tools(),
                config: entry.config.clone(),
            })
            .collect()
    }

    fn enable_plugin(&self, name: &str) -> Result<(), AppError> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| AppError::Plugin(format!("Failed to acquire write lock: {}", e)))?;

        let entry = plugins
            .get_mut(name)
            .ok_or_else(|| AppError::NotFound(format!("Plugin '{}' not found", name)))?;

        entry.enabled = true;
        Ok(())
    }

    fn disable_plugin(&self, name: &str) -> Result<(), AppError> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| AppError::Plugin(format!("Failed to acquire write lock: {}", e)))?;

        let entry = plugins
            .get_mut(name)
            .ok_or_else(|| AppError::NotFound(format!("Plugin '{}' not found", name)))?;

        entry.enabled = false;
        Ok(())
    }

    fn get_plugin_config(&self, name: &str) -> Option<Value> {
        let plugins = self.plugins.read().unwrap_or_else(|e| e.into_inner());
        plugins.get(name).and_then(|entry| entry.config.clone())
    }

    fn set_plugin_config(&self, name: &str, config: Value) -> Result<(), AppError> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| AppError::Plugin(format!("Failed to acquire write lock: {}", e)))?;

        let entry = plugins
            .get_mut(name)
            .ok_or_else(|| AppError::NotFound(format!("Plugin '{}' not found", name)))?;

        entry.config = Some(config);
        Ok(())
    }

    fn get_enabled_tools(&self) -> Vec<ToolDefinition> {
        let plugins = self.plugins.read().unwrap_or_else(|e| e.into_inner());

        plugins
            .values()
            .filter(|entry| entry.enabled)
            .flat_map(|entry| entry.handler.tools())
            .collect()
    }

    async fn execute_tool(&self, tool_call: &ToolCall, app_handle: &tauri::AppHandle) -> Result<ToolResult, AppError> {
        // ツール名からプラグインを検索し、Arcクローンを取得してからロックを解放
        let handler: Option<Arc<dyn PluginHandler>> = {
            let plugins = self
                .plugins
                .read()
                .map_err(|e| AppError::Plugin(format!("Failed to acquire read lock: {}", e)))?;

            plugins
                .values()
                .find(|entry| {
                    entry.enabled
                        && entry
                            .handler
                            .tools()
                            .iter()
                            .any(|t| t.name == tool_call.name)
                })
                .map(|entry| Arc::clone(&entry.handler))
        };

        match handler {
            Some(h) => h.execute(tool_call, app_handle).await,
            None => Err(AppError::NotFound(format!(
                "No enabled plugin provides tool '{}'",
                tool_call.name
            ))),
        }
    }
}
