// Plugin System - PluginHandler trait + PluginSystem trait + DefaultPluginSystem

use async_trait::async_trait;
use std::sync::Arc;

use crate::error::AppError;
use crate::models::plugin::{ToolCall, ToolDefinition, ToolResult};

use super::registry::PluginRegistry;

/// プラグインハンドラtrait — 第三者が実装可能
#[async_trait]
pub trait PluginHandler: Send + Sync {
    /// プラグイン名
    fn name(&self) -> &str;
    /// プラグインの説明
    fn description(&self) -> &str;
    /// このプラグインが提供するツール一覧
    fn tools(&self) -> Vec<ToolDefinition>;
    /// ツール実行
    async fn execute(&self, tool_call: &ToolCall) -> Result<ToolResult, AppError>;
}

/// プラグインシステムtrait — tool_callディスパッチと有効ツール一覧取得
#[async_trait]
pub trait PluginSystem: Send + Sync {
    /// tool_call群を対応するプラグインにディスパッチし、結果を収集
    async fn handle_tool_calls(&self, tool_calls: &[ToolCall])
        -> Result<Vec<ToolResult>, AppError>;
    /// 有効なプラグインが提供する全ツール定義を取得
    fn get_enabled_tools(&self) -> Vec<ToolDefinition>;
}

/// デフォルトのPluginSystem実装
pub struct DefaultPluginSystem {
    registry: Arc<dyn PluginRegistry>,
}

impl DefaultPluginSystem {
    pub fn new(registry: Arc<dyn PluginRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl PluginSystem for DefaultPluginSystem {
    async fn handle_tool_calls(
        &self,
        tool_calls: &[ToolCall],
    ) -> Result<Vec<ToolResult>, AppError> {
        let mut results = Vec::with_capacity(tool_calls.len());

        for tool_call in tool_calls {
            let result = self.registry.execute_tool(tool_call).await;
            match result {
                Ok(tool_result) => results.push(tool_result),
                Err(_) => {
                    // ツールが見つからない場合、is_error=true の結果を返す
                    results.push(ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content: format!("Tool '{}' not found or not available", tool_call.name),
                        is_error: true,
                    });
                }
            }
        }

        Ok(results)
    }

    fn get_enabled_tools(&self) -> Vec<ToolDefinition> {
        self.registry.get_enabled_tools()
    }
}
