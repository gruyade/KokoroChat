// Plugin System tests

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use crate::error::AppError;
use crate::models::plugin::{ToolCall, ToolDefinition, ToolResult};
use crate::plugin::registry::{DefaultPluginRegistry, PluginRegistry};
use crate::plugin::system::{DefaultPluginSystem, PluginHandler, PluginSystem};

/// テスト用モックプラグイン
struct MockPlugin {
    name: String,
    description: String,
}

impl MockPlugin {
    fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl PluginHandler for MockPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: format!("{}_tool", self.name),
            description: format!("Tool provided by {}", self.name),
            parameters: json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
                },
                "required": ["input"]
            }),
        }]
    }

    async fn execute(&self, tool_call: &ToolCall, _app_handle: &tauri::AppHandle) -> Result<ToolResult, AppError> {
        let input = tool_call
            .arguments
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("no input");

        Ok(ToolResult {
            tool_call_id: tool_call.id.clone(),
            content: format!("[{}] processed: {}", self.name, input),
            is_error: false,
        })
    }
}

/// エラーを返すモックプラグイン
struct ErrorPlugin;

#[async_trait]
impl PluginHandler for ErrorPlugin {
    fn name(&self) -> &str {
        "error_plugin"
    }

    fn description(&self) -> &str {
        "A plugin that always errors"
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "error_tool".to_string(),
            description: "Always fails".to_string(),
            parameters: json!({"type": "object"}),
        }]
    }

    async fn execute(&self, tool_call: &ToolCall, _app_handle: &tauri::AppHandle) -> Result<ToolResult, AppError> {
        Ok(ToolResult {
            tool_call_id: tool_call.id.clone(),
            content: "execution failed".to_string(),
            is_error: true,
        })
    }
}

// --- Registry Tests ---

#[test]
fn test_register_plugin() {
    let registry = DefaultPluginRegistry::new();
    let plugin = MockPlugin::new("test_plugin", "A test plugin");

    let result = registry.register(Box::new(plugin));
    assert!(result.is_ok());

    let plugins = registry.list_plugins();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "test_plugin");
    assert_eq!(plugins[0].description, "A test plugin");
    assert!(plugins[0].enabled);
}

#[test]
fn test_register_duplicate_plugin_fails() {
    let registry = DefaultPluginRegistry::new();
    let plugin1 = MockPlugin::new("dup_plugin", "First");
    let plugin2 = MockPlugin::new("dup_plugin", "Second");

    assert!(registry.register(Box::new(plugin1)).is_ok());
    let result = registry.register(Box::new(plugin2));
    assert!(result.is_err());

    match result.unwrap_err() {
        AppError::Plugin(msg) => assert!(msg.contains("already registered")),
        _ => panic!("Expected AppError::Plugin"),
    }
}

#[test]
fn test_unregister_plugin() {
    let registry = DefaultPluginRegistry::new();
    let plugin = MockPlugin::new("removable", "To be removed");

    registry.register(Box::new(plugin)).unwrap();
    assert_eq!(registry.list_plugins().len(), 1);

    let result = registry.unregister("removable");
    assert!(result.is_ok());
    assert_eq!(registry.list_plugins().len(), 0);
}

#[test]
fn test_unregister_nonexistent_plugin_fails() {
    let registry = DefaultPluginRegistry::new();
    let result = registry.unregister("nonexistent");
    assert!(result.is_err());

    match result.unwrap_err() {
        AppError::NotFound(msg) => assert!(msg.contains("nonexistent")),
        _ => panic!("Expected AppError::NotFound"),
    }
}

#[test]
fn test_enable_disable_plugin() {
    let registry = DefaultPluginRegistry::new();
    let plugin = MockPlugin::new("toggle_plugin", "Toggleable");

    registry.register(Box::new(plugin)).unwrap();

    // 初期状態: 有効
    let plugins = registry.list_plugins();
    assert!(plugins[0].enabled);

    // 無効化
    registry.disable_plugin("toggle_plugin").unwrap();
    let plugins = registry.list_plugins();
    assert!(!plugins[0].enabled);

    // 再有効化
    registry.enable_plugin("toggle_plugin").unwrap();
    let plugins = registry.list_plugins();
    assert!(plugins[0].enabled);
}

#[test]
fn test_enable_nonexistent_plugin_fails() {
    let registry = DefaultPluginRegistry::new();
    let result = registry.enable_plugin("ghost");
    assert!(result.is_err());
}

#[test]
fn test_disable_nonexistent_plugin_fails() {
    let registry = DefaultPluginRegistry::new();
    let result = registry.disable_plugin("ghost");
    assert!(result.is_err());
}

#[test]
fn test_plugin_config_management() {
    let registry = DefaultPluginRegistry::new();
    let plugin = MockPlugin::new("configurable", "Has config");

    registry.register(Box::new(plugin)).unwrap();

    // 初期状態: config なし
    assert!(registry.get_plugin_config("configurable").is_none());

    // 設定を保存
    let config = json!({"api_key": "secret", "timeout": 30});
    registry
        .set_plugin_config("configurable", config.clone())
        .unwrap();

    // 設定を取得
    let retrieved = registry.get_plugin_config("configurable");
    assert_eq!(retrieved, Some(config));
}

#[test]
fn test_set_config_nonexistent_plugin_fails() {
    let registry = DefaultPluginRegistry::new();
    let result = registry.set_plugin_config("ghost", json!({}));
    assert!(result.is_err());
}

#[test]
fn test_get_enabled_tools() {
    let registry = DefaultPluginRegistry::new();
    let plugin1 = MockPlugin::new("plugin_a", "Plugin A");
    let plugin2 = MockPlugin::new("plugin_b", "Plugin B");

    registry.register(Box::new(plugin1)).unwrap();
    registry.register(Box::new(plugin2)).unwrap();

    // 両方有効 → 2つのツール
    let tools = registry.get_enabled_tools();
    assert_eq!(tools.len(), 2);

    // 1つ無効化 → 1つのツール
    registry.disable_plugin("plugin_a").unwrap();
    let tools = registry.get_enabled_tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "plugin_b_tool");
}

#[test]
fn test_list_plugins_shows_all() {
    let registry = DefaultPluginRegistry::new();
    let plugin1 = MockPlugin::new("p1", "Plugin 1");
    let plugin2 = MockPlugin::new("p2", "Plugin 2");
    let plugin3 = MockPlugin::new("p3", "Plugin 3");

    registry.register(Box::new(plugin1)).unwrap();
    registry.register(Box::new(plugin2)).unwrap();
    registry.register(Box::new(plugin3)).unwrap();

    registry.disable_plugin("p2").unwrap();

    let plugins = registry.list_plugins();
    assert_eq!(plugins.len(), 3);
}

// --- PluginSystem Tests ---

#[tokio::test]
async fn test_handle_tool_calls_success() {
    let registry = Arc::new(DefaultPluginRegistry::new());
    let plugin = MockPlugin::new("calc", "Calculator");
    registry.register(Box::new(plugin)).unwrap();

    let system = DefaultPluginSystem::new(registry);

    let tool_calls = vec![ToolCall {
        id: "call_1".to_string(),
        name: "calc_tool".to_string(),
        arguments: json!({"input": "2+2"}),
        context: None,
    }];

    let results = system.handle_tool_calls(&tool_calls).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_call_id, "call_1");
    assert!(!results[0].is_error);
    assert!(results[0].content.contains("processed: 2+2"));
}

#[tokio::test]
async fn test_handle_tool_calls_not_found() {
    let registry = Arc::new(DefaultPluginRegistry::new());
    let system = DefaultPluginSystem::new(registry);

    let tool_calls = vec![ToolCall {
        id: "call_x".to_string(),
        name: "nonexistent_tool".to_string(),
        arguments: json!({}),
        context: None,
    }];

    let results = system.handle_tool_calls(&tool_calls).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].is_error);
    assert!(results[0].content.contains("not found"));
}

#[tokio::test]
async fn test_handle_multiple_tool_calls() {
    let registry = Arc::new(DefaultPluginRegistry::new());
    let plugin_a = MockPlugin::new("alpha", "Alpha plugin");
    let plugin_b = MockPlugin::new("beta", "Beta plugin");
    registry.register(Box::new(plugin_a)).unwrap();
    registry.register(Box::new(plugin_b)).unwrap();

    let system = DefaultPluginSystem::new(registry);

    let tool_calls = vec![
        ToolCall {
            id: "c1".to_string(),
            name: "alpha_tool".to_string(),
            arguments: json!({"input": "hello"}),
            context: None,
        },
        ToolCall {
            id: "c2".to_string(),
            name: "beta_tool".to_string(),
            arguments: json!({"input": "world"}),
            context: None,
        },
    ];

    let results = system.handle_tool_calls(&tool_calls).await.unwrap();
    assert_eq!(results.len(), 2);
    assert!(!results[0].is_error);
    assert!(!results[1].is_error);
    assert!(results[0].content.contains("[alpha]"));
    assert!(results[1].content.contains("[beta]"));
}

#[tokio::test]
async fn test_handle_tool_calls_disabled_plugin() {
    let registry = Arc::new(DefaultPluginRegistry::new());
    let plugin = MockPlugin::new("disabled_one", "Will be disabled");
    registry.register(Box::new(plugin)).unwrap();
    registry.disable_plugin("disabled_one").unwrap();

    let system = DefaultPluginSystem::new(registry);

    let tool_calls = vec![ToolCall {
        id: "c_dis".to_string(),
        name: "disabled_one_tool".to_string(),
        arguments: json!({"input": "test"}),
        context: None,
    }];

    let results = system.handle_tool_calls(&tool_calls).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].is_error);
    assert!(results[0].content.contains("not found"));
}

#[tokio::test]
async fn test_get_enabled_tools_via_system() {
    let registry = Arc::new(DefaultPluginRegistry::new());
    let plugin = MockPlugin::new("sys_plugin", "System plugin");
    registry.register(Box::new(plugin)).unwrap();

    let system = DefaultPluginSystem::new(registry);

    let tools = system.get_enabled_tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "sys_plugin_tool");
}

#[tokio::test]
async fn test_execute_tool_with_error_plugin() {
    let registry = Arc::new(DefaultPluginRegistry::new());
    registry.register(Box::new(ErrorPlugin)).unwrap();

    let system = DefaultPluginSystem::new(registry);

    let tool_calls = vec![ToolCall {
        id: "err_call".to_string(),
        name: "error_tool".to_string(),
        arguments: json!({}),
        context: None,
    }];

    let results = system.handle_tool_calls(&tool_calls).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].is_error);
    assert_eq!(results[0].content, "execution failed");
}

#[tokio::test]
async fn test_handle_empty_tool_calls() {
    let registry = Arc::new(DefaultPluginRegistry::new());
    let system = DefaultPluginSystem::new(registry);

    let results = system.handle_tool_calls(&[]).await.unwrap();
    assert!(results.is_empty());
}
