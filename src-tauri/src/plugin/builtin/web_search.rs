// Web検索プラグイン

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;

use crate::config::model_config::ModelConfigManager;
use crate::error::AppError;
use crate::models::plugin::{ToolCall, ToolDefinition, ToolResult};
use crate::plugin::system::PluginHandler;

/// fetch_page で返すテキストの最大文字数（トークン数制限を考慮）
const FETCH_PAGE_MAX_CHARS: usize = 8000;

/// Web検索プロバイダ種別
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SearchProvider {
    #[default]
    Tavily,
    Brave,
}

/// Web検索プラグイン設定
///
/// `AppConfig.plugins.plugin_settings["web_search"]` にJSON形式で保存される。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebSearchConfig {
    /// 検索プロバイダ（Tavily or Brave）
    #[serde(default)]
    pub provider: SearchProvider,
    /// Tavily API用のAPIキー
    #[serde(default)]
    pub tavily_api_key: Option<String>,
    /// Brave Search API用のAPIキー
    #[serde(default)]
    pub brave_api_key: Option<String>,
    /// 旧フィールド互換用（読み込み専用、保存時は使わない）
    #[serde(default, skip_serializing)]
    api_key: Option<String>,
    /// fetch_page ツール用のドメインホワイトリスト
    #[serde(default)]
    pub allowed_domains: Vec<String>,
}

impl WebSearchConfig {
    /// plugin_settings の JSON Value から WebSearchConfig をデシリアライズする。
    /// 値が存在しない、またはパース失敗時はデフォルト値を返す。
    /// 旧 `api_key` フィールドが存在する場合、provider に応じて適切なフィールドにマイグレーションする。
    pub fn from_plugin_settings(value: Option<&Value>) -> Self {
        match value {
            Some(v) => {
                let mut config: Self = serde_json::from_value(v.clone()).unwrap_or_default();
                // 旧 api_key フィールドからのマイグレーション
                if let Some(old_key) = config.api_key.take() {
                    if !old_key.is_empty() {
                        match config.provider {
                            SearchProvider::Tavily => {
                                if config.tavily_api_key.is_none() {
                                    config.tavily_api_key = Some(old_key);
                                }
                            }
                            SearchProvider::Brave => {
                                if config.brave_api_key.is_none() {
                                    config.brave_api_key = Some(old_key);
                                }
                            }
                        }
                    }
                }
                config
            }
            None => Self::default(),
        }
    }

    /// 現在のプロバイダに対応するAPIキーを取得する
    pub fn active_api_key(&self) -> Option<&str> {
        match self.provider {
            SearchProvider::Tavily => self.tavily_api_key.as_deref(),
            SearchProvider::Brave => self.brave_api_key.as_deref(),
        }
    }

    /// WebSearchConfig を JSON Value にシリアライズする。
    pub fn to_plugin_settings(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| json!({}))
    }

    /// プラグインレジストリから設定を読み込むヘルパー
    pub fn load_from_registry(registry: &dyn crate::plugin::registry::PluginRegistry) -> Self {
        let value = registry.get_plugin_config("web_search");
        Self::from_plugin_settings(value.as_ref())
    }

    /// プラグインレジストリに設定を保存するヘルパー
    pub fn save_to_registry(
        &self,
        registry: &dyn crate::plugin::registry::PluginRegistry,
    ) -> Result<(), AppError> {
        let value = self.to_plugin_settings();
        registry.set_plugin_config("web_search", value)
    }
}

/// Web検索プラグイン — Web検索を行う
pub struct WebSearchPlugin {
    config_manager: Arc<ModelConfigManager>,
}

impl WebSearchPlugin {
    pub fn new(config_manager: Arc<ModelConfigManager>) -> Self {
        Self { config_manager }
    }

    /// 設定から WebSearchConfig を読み込む
    fn load_config(&self) -> WebSearchConfig {
        let app_config = self.config_manager.get_config();
        let value = app_config.plugins.plugin_settings.get("web_search");
        WebSearchConfig::from_plugin_settings(value)
    }

    /// Brave Search API を呼び出して検索結果を取得する
    async fn search_brave(&self, query: &str, api_key: &str) -> Result<String, AppError> {
        let client = reqwest::Client::new();

        let response = client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("Accept", "application/json")
            .header("X-Subscription-Token", api_key)
            .query(&[("q", query)])
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Brave Search API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Network(format!(
                "Brave Search API returned status {}: {}",
                status, body
            )));
        }

        let response_json: Value = response.json().await.map_err(|e| {
            AppError::Serialization(format!("Failed to parse Brave response: {}", e))
        })?;

        // web.results 配列から title, url, description を抽出
        let results = response_json
            .get("web")
            .and_then(|w| w.get("results"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .map(|item| {
                        json!({
                            "title": item.get("title").and_then(Value::as_str).unwrap_or(""),
                            "url": item.get("url").and_then(Value::as_str).unwrap_or(""),
                            "content": item.get("description").and_then(Value::as_str).unwrap_or("")
                        })
                    })
                    .collect::<Vec<Value>>()
            })
            .unwrap_or_default();

        let output = json!({
            "query": query,
            "results": results
        });

        serde_json::to_string_pretty(&output)
            .map_err(|e| AppError::Serialization(format!("Failed to serialize results: {}", e)))
    }

    /// Tavily API を呼び出して検索結果を取得する
    async fn search_tavily(&self, query: &str, api_key: &str) -> Result<String, AppError> {
        let client = reqwest::Client::new();

        let request_body = json!({
            "api_key": api_key,
            "query": query,
            "search_depth": "basic",
            "include_answer": false,
            "max_results": 5
        });

        let response = client
            .post("https://api.tavily.com/search")
            .json(&request_body)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Tavily API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Network(format!(
                "Tavily API returned status {}: {}",
                status, body
            )));
        }

        let response_json: Value = response.json().await.map_err(|e| {
            AppError::Serialization(format!("Failed to parse Tavily response: {}", e))
        })?;

        // results 配列から title, url, content を抽出
        let results = response_json
            .get("results")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .map(|item| {
                        json!({
                            "title": item.get("title").and_then(Value::as_str).unwrap_or(""),
                            "url": item.get("url").and_then(Value::as_str).unwrap_or(""),
                            "content": item.get("content").and_then(Value::as_str).unwrap_or("")
                        })
                    })
                    .collect::<Vec<Value>>()
            })
            .unwrap_or_default();

        let output = json!({
            "query": query,
            "results": results
        });

        serde_json::to_string_pretty(&output)
            .map_err(|e| AppError::Serialization(format!("Failed to serialize results: {}", e)))
    }

    /// ドメインがホワイトリストに含まれるか検証する。
    /// サブドメインマッチをサポート: allowed_domains に "example.com" があれば
    /// "sub.example.com" もマッチする。
    fn is_domain_allowed(host: &str, allowed_domains: &[String]) -> bool {
        if allowed_domains.is_empty() {
            return false;
        }
        for domain in allowed_domains {
            if host == domain || host.ends_with(&format!(".{}", domain)) {
                return true;
            }
        }
        false
    }

    /// HTMLからテキスト本文を抽出する。
    /// script/style ブロックを除去し、タグを取り除き、空白を正規化する。
    fn extract_text_from_html(html: &str) -> String {
        // <script>...</script> と <style>...</style> を除去
        let re_script = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
        let text = re_script.replace_all(html, "");
        let re_style = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
        let text = re_style.replace_all(&text, "");
        // 全HTMLタグを除去
        let re_tags = Regex::new(r"<[^>]+>").unwrap();
        let text = re_tags.replace_all(&text, " ");
        // HTMLエンティティの基本的なデコード
        let text = text
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ");
        // 連続する空白・改行を1つのスペースに正規化
        let re_whitespace = Regex::new(r"\s+").unwrap();
        let text = re_whitespace.replace_all(&text, " ");
        text.trim().to_string()
    }

    /// fetch_page ツールを実行する
    async fn execute_fetch_page(&self, tool_call: &ToolCall) -> Result<ToolResult, AppError> {
        let url_str = tool_call
            .arguments
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Plugin("'url' パラメータが必要".to_string()))?;

        // URL パース
        let parsed_url =
            Url::parse(url_str).map_err(|e| AppError::Plugin(format!("無効なURL: {}", e)))?;

        let host = parsed_url
            .host_str()
            .ok_or_else(|| AppError::Plugin("URLにホスト名がない".to_string()))?;

        // 設定を読み込み、ドメインホワイトリスト検証
        let config = self.load_config();
        if !Self::is_domain_allowed(host, &config.allowed_domains) {
            let blocked_msg = json!({
                "error": "このドメインへのアクセスはホワイトリストで許可されていません。設定画面の「ツール管理」タブで許可ドメインを追加してください。",
                "blocked_domain": host
            });
            return Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: serde_json::to_string_pretty(&blocked_msg)
                    .unwrap_or_else(|_| blocked_msg.to_string()),
                is_error: false,
            });
        }

        // HTTP GET でページ取得
        let client = reqwest::Client::new();
        let response = match client
            .get(url_str)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                return Ok(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    content: format!("ページ取得エラー: {}", e),
                    is_error: true,
                });
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            return Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: format!("ページ取得エラー: HTTP {}", status),
                is_error: true,
            });
        }

        let html = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                return Ok(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    content: format!("レスポンス読み取りエラー: {}", e),
                    is_error: true,
                });
            }
        };

        // テキスト抽出
        let text = Self::extract_text_from_html(&html);

        // 文字数制限で切り詰め
        let truncated = text.len() > FETCH_PAGE_MAX_CHARS;
        let content = if truncated {
            // 文字境界を考慮して切り詰め
            let mut end = FETCH_PAGE_MAX_CHARS;
            while !text.is_char_boundary(end) && end > 0 {
                end -= 1;
            }
            &text[..end]
        } else {
            &text
        };

        let output = json!({
            "url": url_str,
            "content": content,
            "truncated": truncated
        });

        Ok(ToolResult {
            tool_call_id: tool_call.id.clone(),
            content: serde_json::to_string_pretty(&output).unwrap_or_else(|_| output.to_string()),
            is_error: false,
        })
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
        vec![
            ToolDefinition {
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
            },
            ToolDefinition {
                name: "fetch_page".to_string(),
                description: "指定URLのWebページ本文を取得する".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "取得するWebページのURL"
                        }
                    },
                    "required": ["url"]
                }),
            },
        ]
    }

    async fn execute(&self, tool_call: &ToolCall) -> Result<ToolResult, AppError> {
        match tool_call.name.as_str() {
            "fetch_page" => return self.execute_fetch_page(tool_call).await,
            "search" => {}
            _ => {
                return Err(AppError::Plugin(format!(
                    "不明なツール名: {}",
                    tool_call.name
                )));
            }
        }

        // --- search ツールの処理 ---
        let query = tool_call
            .arguments
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Plugin("'query' パラメータが必要".to_string()))?;

        // 設定を読み込み
        let config = self.load_config();

        // 現在のプロバイダに対応するAPIキーを取得
        let api_key = config.active_api_key().unwrap_or("");
        if api_key.is_empty() {
            let guidance = json!({
                "note": "Web検索を利用するには、設定画面の「ツール管理」タブからAPIキーを設定してください。Tavily (https://tavily.com) またはBrave Search API (https://brave.com/search/api/) のキーが必要です。"
            });
            return Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: serde_json::to_string_pretty(&guidance)
                    .unwrap_or_else(|_| guidance.to_string()),
                is_error: false,
            });
        }

        // APIキー設定済み — プロバイダに応じてAPI呼び出し
        match config.provider {
            SearchProvider::Tavily => match self.search_tavily(query, api_key).await {
                Ok(content) => Ok(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    content,
                    is_error: false,
                }),
                Err(e) => Ok(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    content: format!("検索エラー: {}", e),
                    is_error: true,
                }),
            },
            SearchProvider::Brave => match self.search_brave(query, api_key).await {
                Ok(content) => Ok(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    content,
                    is_error: false,
                }),
                Err(e) => Ok(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    content: format!("検索エラー: {}", e),
                    is_error: true,
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::config::model_config::ModelConfigManager;
    use crate::models::config::{
        AppConfig, AttachmentConfig, MemoryConfig, ModelPurpose, ModelSettings, PluginsConfig,
        SendKey, SpontaneousConfig, TTSGlobalConfig, Theme, ThoughtConfig, UIConfig,
    };

    /// テスト用のデフォルト AppConfig を生成
    fn test_app_config() -> AppConfig {
        let mut models = HashMap::new();
        let default_settings = ModelSettings {
            provider: None,
            base_url: String::new(),
            model: String::new(),
            api_key: None,
            temperature: 0.7,
        };
        models.insert(ModelPurpose::Chat, default_settings);

        AppConfig {
            models,
            spontaneous: SpontaneousConfig {
                enabled: false,
                min_interval_seconds: 60,
                probability: 0.3,
            },
            thought: ThoughtConfig {
                enabled: false,
                interval_minutes: 5,
                auto_delete_threshold_minutes: 1440,
            },
            memory: MemoryConfig {
                compression_threshold: 50,
            },
            tts: TTSGlobalConfig {
                enabled: false,
                voicepeak_path: None,
                timeout_seconds: 60,
                max_chunk_size: 140,
                irodori_base_url: None,
                irodori_caption_base_url: None,
                irodori_reference_audio_base_url: None,
            },
            ui: UIConfig {
                theme: Theme::Dark,
                language: "ja".to_string(),
                send_key: SendKey::default(),
            },
            plugins: PluginsConfig {
                enabled_plugins: vec![],
                plugin_settings: HashMap::new(),
            },
            attachment: AttachmentConfig {
                max_file_size_bytes: 10 * 1024 * 1024,
                allowed_extensions: vec!["txt".to_string()],
            },
        }
    }

    /// テスト用の ModelConfigManager を生成（APIキーなし）
    fn test_config_manager_no_key() -> Arc<ModelConfigManager> {
        Arc::new(ModelConfigManager::new_with_config(test_app_config()))
    }

    /// テスト用の ModelConfigManager を生成（APIキーあり）
    fn test_config_manager_with_key(api_key: &str) -> Arc<ModelConfigManager> {
        let mut config = test_app_config();
        let web_search_settings = json!({
            "provider": "tavily",
            "tavily_api_key": api_key
        });
        config
            .plugins
            .plugin_settings
            .insert("web_search".to_string(), web_search_settings);
        Arc::new(ModelConfigManager::new_with_config(config))
    }

    /// テスト用の ModelConfigManager を生成（Brave プロバイダ、APIキーあり）
    fn test_config_manager_brave(api_key: &str) -> Arc<ModelConfigManager> {
        let mut config = test_app_config();
        let web_search_settings = json!({
            "provider": "brave",
            "brave_api_key": api_key
        });
        config
            .plugins
            .plugin_settings
            .insert("web_search".to_string(), web_search_settings);
        Arc::new(ModelConfigManager::new_with_config(config))
    }

    // --- WebSearchConfig テスト ---

    #[test]
    fn test_config_default() {
        let config = WebSearchConfig::default();
        assert_eq!(config.provider, SearchProvider::Tavily);
        assert_eq!(config.tavily_api_key, None);
        assert_eq!(config.brave_api_key, None);
        assert!(config.allowed_domains.is_empty());
    }

    #[test]
    fn test_config_serialize_deserialize() {
        let config = WebSearchConfig {
            provider: SearchProvider::Brave,
            tavily_api_key: Some("tvly-key".to_string()),
            brave_api_key: Some("brave-key-123".to_string()),
            api_key: None,
            allowed_domains: vec!["example.com".to_string(), "docs.rs".to_string()],
        };

        let value = config.to_plugin_settings();
        let restored = WebSearchConfig::from_plugin_settings(Some(&value));

        assert_eq!(restored.provider, SearchProvider::Brave);
        assert_eq!(restored.tavily_api_key, Some("tvly-key".to_string()));
        assert_eq!(restored.brave_api_key, Some("brave-key-123".to_string()));
        assert_eq!(restored.allowed_domains.len(), 2);
        assert_eq!(restored.allowed_domains[0], "example.com");
        assert_eq!(restored.allowed_domains[1], "docs.rs");
    }

    #[test]
    fn test_config_from_none_returns_default() {
        let config = WebSearchConfig::from_plugin_settings(None);
        assert_eq!(config.provider, SearchProvider::Tavily);
        assert_eq!(config.tavily_api_key, None);
        assert_eq!(config.brave_api_key, None);
    }

    #[test]
    fn test_config_from_invalid_json_returns_default() {
        let invalid = json!("not an object");
        let config = WebSearchConfig::from_plugin_settings(Some(&invalid));
        assert_eq!(config.provider, SearchProvider::Tavily);
        assert_eq!(config.tavily_api_key, None);
    }

    #[test]
    fn test_config_from_partial_json() {
        // provider のみ指定、他はデフォルト
        let partial = json!({ "provider": "brave" });
        let config = WebSearchConfig::from_plugin_settings(Some(&partial));
        assert_eq!(config.provider, SearchProvider::Brave);
        assert_eq!(config.brave_api_key, None);
        assert!(config.allowed_domains.is_empty());
    }

    #[test]
    fn test_config_migration_from_old_api_key() {
        // 旧フォーマット（api_key フィールド）からのマイグレーション
        let old_format = json!({
            "provider": "tavily",
            "api_key": "tvly-old-key"
        });
        let config = WebSearchConfig::from_plugin_settings(Some(&old_format));
        assert_eq!(config.tavily_api_key, Some("tvly-old-key".to_string()));
        assert_eq!(config.brave_api_key, None);

        let old_format_brave = json!({
            "provider": "brave",
            "api_key": "BSA-old-key"
        });
        let config = WebSearchConfig::from_plugin_settings(Some(&old_format_brave));
        assert_eq!(config.brave_api_key, Some("BSA-old-key".to_string()));
        assert_eq!(config.tavily_api_key, None);
    }

    #[test]
    fn test_config_registry_roundtrip() {
        use crate::plugin::registry::{DefaultPluginRegistry, PluginRegistry};

        let registry = DefaultPluginRegistry::new();
        // プラグインを登録（set_plugin_config にはプラグインが登録されている必要がある）
        registry
            .register(Box::new(WebSearchPlugin::new(test_config_manager_no_key())))
            .unwrap();

        let config = WebSearchConfig {
            provider: SearchProvider::Tavily,
            tavily_api_key: Some("tvly-abc".to_string()),
            brave_api_key: None,
            api_key: None,
            allowed_domains: vec!["wikipedia.org".to_string()],
        };

        config.save_to_registry(&registry).unwrap();
        let loaded = WebSearchConfig::load_from_registry(&registry);

        assert_eq!(loaded.provider, SearchProvider::Tavily);
        assert_eq!(loaded.tavily_api_key, Some("tvly-abc".to_string()));
        assert_eq!(loaded.allowed_domains, vec!["wikipedia.org".to_string()]);
    }

    // --- プラグイン動作テスト ---

    #[test]
    fn test_plugin_metadata() {
        let plugin = WebSearchPlugin::new(test_config_manager_no_key());
        assert_eq!(plugin.name(), "web_search");
        assert_eq!(plugin.description(), "Web検索を行う");
        assert_eq!(plugin.tools().len(), 2);
        assert_eq!(plugin.tools()[0].name, "search");
        assert_eq!(plugin.tools()[1].name, "fetch_page");
    }

    #[tokio::test]
    async fn test_execute_no_api_key_returns_guidance() {
        let plugin = WebSearchPlugin::new(test_config_manager_no_key());
        let tool_call = ToolCall {
            id: "call-1".to_string(),
            name: "search".to_string(),
            arguments: json!({ "query": "Rust programming" }),
        };

        let result = plugin.execute(&tool_call).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.tool_call_id, "call-1");
        // APIキー未設定時はガイダンスメッセージを返す
        assert!(result.content.contains("APIキーを設定してください"));
        assert!(result.content.contains("Tavily"));
    }

    #[tokio::test]
    async fn test_execute_with_tavily_key_calls_api() {
        // 無効なAPIキーでTavily APIを呼ぶとエラーが返る
        let plugin = WebSearchPlugin::new(test_config_manager_with_key("tvly-invalid-key"));
        let tool_call = ToolCall {
            id: "call-2".to_string(),
            name: "search".to_string(),
            arguments: json!({ "query": "Rust programming" }),
        };

        let result = plugin.execute(&tool_call).await.unwrap();
        assert_eq!(result.tool_call_id, "call-2");
        // 無効なキーなので is_error: true が返る（APIエラーまたはネットワークエラー）
        assert!(result.is_error);
        assert!(result.content.contains("検索エラー"));
    }

    #[tokio::test]
    async fn test_execute_missing_query() {
        let plugin = WebSearchPlugin::new(test_config_manager_no_key());
        let tool_call = ToolCall {
            id: "call-3".to_string(),
            name: "search".to_string(),
            arguments: json!({}),
        };

        let result = plugin.execute(&tool_call).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_with_japanese_query_no_key() {
        let plugin = WebSearchPlugin::new(test_config_manager_no_key());
        let tool_call = ToolCall {
            id: "call-4".to_string(),
            name: "search".to_string(),
            arguments: json!({ "query": "人工知能" }),
        };

        let result = plugin.execute(&tool_call).await.unwrap();
        assert!(!result.is_error);
        // APIキー未設定なのでガイダンスが返る
        assert!(result.content.contains("APIキーを設定してください"));
    }

    #[tokio::test]
    async fn test_execute_with_empty_api_key_returns_guidance() {
        let plugin = WebSearchPlugin::new(test_config_manager_with_key(""));
        let tool_call = ToolCall {
            id: "call-5".to_string(),
            name: "search".to_string(),
            arguments: json!({ "query": "test" }),
        };

        let result = plugin.execute(&tool_call).await.unwrap();
        assert!(!result.is_error);
        // 空文字列のAPIキーもガイダンスを返す
        assert!(result.content.contains("APIキーを設定してください"));
    }

    // --- fetch_page テスト ---

    /// テスト用の ModelConfigManager を生成（allowed_domains 付き）
    fn test_config_manager_with_domains(domains: Vec<&str>) -> Arc<ModelConfigManager> {
        let mut config = test_app_config();
        let web_search_settings = json!({
            "provider": "tavily",
            "tavily_api_key": "test-key",
            "allowed_domains": domains
        });
        config
            .plugins
            .plugin_settings
            .insert("web_search".to_string(), web_search_settings);
        Arc::new(ModelConfigManager::new_with_config(config))
    }

    #[tokio::test]
    async fn test_fetch_page_empty_allowed_domains_blocks() {
        // allowed_domains が空の場合、全ドメインをブロック
        let plugin = WebSearchPlugin::new(test_config_manager_no_key());
        let tool_call = ToolCall {
            id: "fetch-1".to_string(),
            name: "fetch_page".to_string(),
            arguments: json!({ "url": "https://example.com/page" }),
        };

        let result = plugin.execute(&tool_call).await.unwrap();
        assert!(!result.is_error);
        assert!(result
            .content
            .contains("ホワイトリストで許可されていません"));
        assert!(result.content.contains("example.com"));
    }

    #[tokio::test]
    async fn test_fetch_page_domain_not_in_whitelist_blocks() {
        // ホワイトリストに含まれないドメインはブロック
        let plugin = WebSearchPlugin::new(test_config_manager_with_domains(vec!["allowed.com"]));
        let tool_call = ToolCall {
            id: "fetch-2".to_string(),
            name: "fetch_page".to_string(),
            arguments: json!({ "url": "https://blocked.com/page" }),
        };

        let result = plugin.execute(&tool_call).await.unwrap();
        assert!(!result.is_error);
        assert!(result
            .content
            .contains("ホワイトリストで許可されていません"));
        assert!(result.content.contains("blocked.com"));
    }

    #[test]
    fn test_domain_matching_exact() {
        // 完全一致
        let domains = vec!["example.com".to_string()];
        assert!(WebSearchPlugin::is_domain_allowed("example.com", &domains));
        assert!(!WebSearchPlugin::is_domain_allowed("other.com", &domains));
    }

    #[test]
    fn test_domain_matching_subdomain() {
        // サブドメインマッチ
        let domains = vec!["example.com".to_string()];
        assert!(WebSearchPlugin::is_domain_allowed(
            "sub.example.com",
            &domains
        ));
        assert!(WebSearchPlugin::is_domain_allowed(
            "deep.sub.example.com",
            &domains
        ));
        // 部分一致ではマッチしない
        assert!(!WebSearchPlugin::is_domain_allowed(
            "notexample.com",
            &domains
        ));
    }

    #[test]
    fn test_domain_matching_empty_list() {
        // 空リストは全てブロック
        let domains: Vec<String> = vec![];
        assert!(!WebSearchPlugin::is_domain_allowed("example.com", &domains));
    }

    #[test]
    fn test_extract_text_from_html_basic() {
        let html = "<html><head><title>Test</title></head><body><p>Hello World</p></body></html>";
        let text = WebSearchPlugin::extract_text_from_html(html);
        assert!(text.contains("Hello World"));
        assert!(!text.contains("<p>"));
    }

    #[test]
    fn test_extract_text_removes_script_and_style() {
        let html = r#"<html><head><style>body{color:red}</style></head>
            <body><script>alert('x')</script><p>Content</p></body></html>"#;
        let text = WebSearchPlugin::extract_text_from_html(html);
        assert!(text.contains("Content"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("color:red"));
    }

    #[test]
    fn test_extract_text_collapses_whitespace() {
        let html = "<p>Hello</p>   \n\n   <p>World</p>";
        let text = WebSearchPlugin::extract_text_from_html(html);
        // 連続空白は1つのスペースに正規化される
        assert!(!text.contains("  "));
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[tokio::test]
    async fn test_fetch_page_invalid_url() {
        let plugin = WebSearchPlugin::new(test_config_manager_with_domains(vec!["example.com"]));
        let tool_call = ToolCall {
            id: "fetch-invalid".to_string(),
            name: "fetch_page".to_string(),
            arguments: json!({ "url": "not-a-valid-url" }),
        };

        let result = plugin.execute(&tool_call).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_page_missing_url_param() {
        let plugin = WebSearchPlugin::new(test_config_manager_with_domains(vec!["example.com"]));
        let tool_call = ToolCall {
            id: "fetch-no-url".to_string(),
            name: "fetch_page".to_string(),
            arguments: json!({}),
        };

        let result = plugin.execute(&tool_call).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_page_allowed_domain_attempts_fetch() {
        // 許可ドメインの場合、実際にHTTPリクエストを試みる（ネットワークエラーになる可能性あり）
        let plugin = WebSearchPlugin::new(test_config_manager_with_domains(vec!["example.com"]));
        let tool_call = ToolCall {
            id: "fetch-allowed".to_string(),
            name: "fetch_page".to_string(),
            arguments: json!({ "url": "https://example.com" }),
        };

        // ドメインチェックは通過するので、ネットワークエラーか成功のどちらか
        let result = plugin.execute(&tool_call).await;
        // エラーにならない（ToolResult が返る）
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        // ホワイトリストエラーではない
        assert!(!tool_result
            .content
            .contains("ホワイトリストで許可されていません"));
    }

    #[tokio::test]
    async fn test_execute_unknown_tool_name() {
        let plugin = WebSearchPlugin::new(test_config_manager_no_key());
        let tool_call = ToolCall {
            id: "call-unknown".to_string(),
            name: "unknown_tool".to_string(),
            arguments: json!({}),
        };

        let result = plugin.execute(&tool_call).await;
        assert!(result.is_err());
    }
}
