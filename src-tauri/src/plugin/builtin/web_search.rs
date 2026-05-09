// Web検索プラグイン（スタブ実装）

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::models::plugin::{ToolCall, ToolDefinition, ToolResult};
use crate::plugin::system::PluginHandler;

/// Web検索プラグイン — Web検索を行う（スタブ実装）
///
/// 実際のAPI連携は将来実装。現在はプレースホルダーレスポンスを返す。
pub struct WebSearchPlugin;

impl WebSearchPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PluginHandler for WebSearchPlugin {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Web検索を行う"
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "search".to_string(),
            description: "キーワードでWeb検索する".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "検索キーワード"
                    }
                },
                "required": ["query"]
            }),
        }]
    }

    async fn execute(&self, tool_call: &ToolCall) -> Result<ToolResult, AppError> {
        let query = tool_call
            .arguments
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Plugin("'query' パラメータが必要".to_string()))?;

        // スタブ実装: プレースホルダーレスポンスを返す
        let stub_response = json!({
            "query": query,
            "results": [
                {
                    "title": format!("「{}」の検索結果1", query),
                    "url": format!("https://example.com/search?q={}", query),
                    "snippet": format!("「{}」に関する情報のスニペット（スタブ）", query)
                },
                {
                    "title": format!("「{}」の検索結果2", query),
                    "url": format!("https://example.org/article/{}", query),
                    "snippet": format!("「{}」についての詳細記事（スタブ）", query)
                }
            ],
            "note": "これはスタブ実装。実際のWeb検索APIは未接続。"
        });

        Ok(ToolResult {
            tool_call_id: tool_call.id.clone(),
            content: serde_json::to_string_pretty(&stub_response)
                .unwrap_or_else(|_| stub_response.to_string()),
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_metadata() {
        let plugin = WebSearchPlugin::new();
        assert_eq!(plugin.name(), "web_search");
        assert_eq!(plugin.description(), "Web検索を行う");
        assert_eq!(plugin.tools().len(), 1);
        assert_eq!(plugin.tools()[0].name, "search");
    }

    #[tokio::test]
    async fn test_execute_returns_stub_response() {
        let plugin = WebSearchPlugin::new();
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: "search".to_string(),
            arguments: json!({ "query": "Rust programming" }),
        };

        let result = plugin.execute(&tool_call).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.tool_call_id, "call-1");
        assert!(result.content.contains("Rust programming"));
        assert!(result.content.contains("スタブ"));
    }

    #[tokio::test]
    async fn test_execute_missing_query() {
        let plugin = WebSearchPlugin::new();
        let tool_call = ToolCall {
            id: "call-2".to_string(),
            name: "search".to_string(),
            arguments: json!({}),
        };

        let result = plugin.execute(&tool_call).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_japanese_query() {
        let plugin = WebSearchPlugin::new();
        let tool_call = ToolCall {
            id: "call-3".to_string(),
            name: "search".to_string(),
            arguments: json!({ "query": "人工知能" }),
        };

        let result = plugin.execute(&tool_call).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("人工知能"));
    }
}
