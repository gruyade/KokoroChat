//! プラグインシステムのプロパティテスト
//! proptest を使用して Plugin Registry / Plugin System の不変条件を検証する。
//!
//! **Validates: Requirements 11.1, 11.2, 11.3, 11.7, 11.8, 11.9**

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use proptest::prelude::*;
    use serde_json::{json, Value};

    use crate::error::AppError;
    use crate::models::plugin::{ToolCall, ToolDefinition, ToolResult};
    use crate::plugin::registry::{DefaultPluginRegistry, PluginRegistry};
    use crate::plugin::system::{DefaultPluginSystem, PluginHandler, PluginSystem};

    type TestRegistry = DefaultPluginRegistry<tauri::test::MockRuntime>;

    // ========================================
    // テスト用モックプラグイン
    // ========================================

    /// 設定可能なモックプラグイン
    struct MockPlugin {
        plugin_name: String,
        plugin_description: String,
        tool_defs: Vec<ToolDefinition>,
    }

    impl MockPlugin {
        fn new(name: &str) -> Self {
            Self {
                plugin_name: name.to_string(),
                plugin_description: format!("Mock plugin: {}", name),
                tool_defs: vec![ToolDefinition {
                    name: format!("{}_tool", name),
                    description: format!("Tool provided by {}", name),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "input": { "type": "string" }
                        },
                        "required": ["input"]
                    }),
                }],
            }
        }
    }

    #[async_trait]
    impl<R: tauri::Runtime> PluginHandler<R> for MockPlugin {
        fn name(&self) -> &str {
            &self.plugin_name
        }

        fn description(&self) -> &str {
            &self.plugin_description
        }

        fn tools(&self) -> Vec<ToolDefinition> {
            self.tool_defs.clone()
        }

        async fn execute(
            &self,
            tool_call: &ToolCall,
            _app_handle: &tauri::AppHandle<R>,
        ) -> Result<ToolResult, AppError> {
            let input = tool_call
                .arguments
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("no input");

            Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: format!("[{}] processed: {}", self.plugin_name, input),
                is_error: false,
            })
        }
    }

    fn make_mock_app() -> tauri::App<tauri::test::MockRuntime> {
        tauri::test::mock_builder()
            .build(tauri::generate_context!())
            .unwrap()
    }

    // ========================================
    // ストラテジー
    // ========================================

    /// 有効なプラグイン名（英数字+アンダースコア、1〜20文字）
    fn plugin_name_strategy() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,19}"
    }

    /// 複数のユニークなプラグイン名を生成
    fn unique_plugin_names(count: usize) -> impl Strategy<Value = Vec<String>> {
        proptest::collection::hash_set(plugin_name_strategy(), count)
            .prop_map(|set| set.into_iter().collect::<Vec<_>>())
    }

    /// JSON Value のストラテジー（プラグイン設定用）
    fn json_config_strategy() -> impl Strategy<Value = Value> {
        prop_oneof![
            Just(json!({"key": "value"})),
            Just(json!({"timeout": 30, "retries": 3})),
            Just(json!({"api_key": "test-key-123", "enabled": true})),
            Just(json!({"nested": {"a": 1, "b": "two"}})),
            Just(json!({"list": [1, 2, 3], "flag": false})),
            Just(json!(null)),
            Just(json!(42)),
            Just(json!("simple_string")),
        ]
    }

    /// ToolCall ID のストラテジー
    fn tool_call_id_strategy() -> impl Strategy<Value = String> {
        "call_[a-z0-9]{4,8}"
    }

    // ========================================
    // Property 21: Plugin registration idempotence
    // ========================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_duplicate_registration_fails(
            name in plugin_name_strategy(),
        ) {
            let registry = TestRegistry::new();

            let plugin1 = MockPlugin::new(&name);
            let result1 = registry.register(Box::new(plugin1));
            prop_assert!(result1.is_ok());

            let plugin2 = MockPlugin::new(&name);
            let result2 = registry.register(Box::new(plugin2));
            prop_assert!(result2.is_err());

            let plugins = registry.list_plugins();
            prop_assert_eq!(plugins.len(), 1);

            match result2.unwrap_err() {
                AppError::Plugin(msg) => {
                    prop_assert!(msg.contains("already registered"));
                }
                other => {
                    return Err(proptest::test_runner::TestCaseError::fail(
                        format!("Expected AppError::Plugin, got: {:?}", other)
                    ));
                }
            }
        }

        #[test]
        fn prop_unique_names_all_register_successfully(
            names in unique_plugin_names(3),
        ) {
            let registry = TestRegistry::new();

            for name in &names {
                let plugin = MockPlugin::new(name);
                let result = registry.register(Box::new(plugin));
                prop_assert!(result.is_ok());
            }

            let plugins = registry.list_plugins();
            prop_assert_eq!(plugins.len(), names.len());
        }
    }

    // ========================================
    // Property 22: Plugin enable/disable isolation
    // ========================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_disable_one_does_not_affect_others(
            names in unique_plugin_names(3),
            target_idx in 0usize..3,
        ) {
            let target_idx = target_idx % names.len();
            let registry = TestRegistry::new();

            for name in &names {
                let plugin = MockPlugin::new(name);
                registry.register(Box::new(plugin)).unwrap();
            }

            let target_name = &names[target_idx];
            registry.disable_plugin(target_name).unwrap();

            let plugins = registry.list_plugins();
            for plugin_info in &plugins {
                if &plugin_info.name == target_name {
                    prop_assert!(!plugin_info.enabled);
                } else {
                    prop_assert!(plugin_info.enabled);
                }
            }
        }

        #[test]
        fn prop_enable_one_does_not_affect_others(
            names in unique_plugin_names(3),
            target_idx in 0usize..3,
        ) {
            let target_idx = target_idx % names.len();
            let registry = TestRegistry::new();

            for name in &names {
                let plugin = MockPlugin::new(name);
                registry.register(Box::new(plugin)).unwrap();
            }

            for name in &names {
                registry.disable_plugin(name).unwrap();
            }

            let target_name = &names[target_idx];
            registry.enable_plugin(target_name).unwrap();

            let plugins = registry.list_plugins();
            for plugin_info in &plugins {
                if &plugin_info.name == target_name {
                    prop_assert!(plugin_info.enabled);
                } else {
                    prop_assert!(!plugin_info.enabled);
                }
            }
        }
    }

    // ========================================
    // Property 23: Tool definition format compliance
    // ========================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_tool_definitions_have_valid_format(
            name in plugin_name_strategy(),
        ) {
            let registry = TestRegistry::new();
            let plugin = MockPlugin::new(&name);
            registry.register(Box::new(plugin)).unwrap();

            let tools = registry.get_enabled_tools();

            for tool in &tools {
                prop_assert!(!tool.name.is_empty());
                prop_assert!(!tool.description.is_empty());
                prop_assert!(tool.parameters.is_object());

                let params_obj = tool.parameters.as_object().unwrap();
                prop_assert!(params_obj.contains_key("type"));
            }
        }

        #[test]
        fn prop_builtin_plugins_tool_format_compliance(
            _dummy in Just(()),
        ) {
            use crate::plugin::builtin::calculator::CalculatorPlugin;
            use crate::plugin::builtin::web_search::WebSearchPlugin;
            use crate::plugin::builtin::file_ops::FileOpsPlugin;
            use crate::config::model_config::ModelConfigManager;
            use crate::models::config::{
                AppConfig, AttachmentConfig, MemoryConfig, ModelPurpose, ModelSettings,
                PluginsConfig, SendKey, SpontaneousConfig, TTSGlobalConfig, Theme, ThoughtConfig,
                UIConfig,
            };
            use std::collections::HashMap;

            let mut models = HashMap::new();
            models.insert(ModelPurpose::Chat, ModelSettings {
                provider: None,
                base_url: String::new(),
                model: String::new(),
                api_key: None,
                temperature: 0.7,
            });
            let app_config = AppConfig {
                models,
                spontaneous: SpontaneousConfig { enabled: false, min_interval_seconds: 60, probability: 0.3 },
                thought: ThoughtConfig { enabled: false, interval_minutes: 5, auto_delete_threshold_minutes: 1440 },
                memory: MemoryConfig { compression_threshold: 50 },
                tts: TTSGlobalConfig { enabled: false, voicepeak_path: None, timeout_seconds: 60, max_chunk_size: 140, irodori_base_url: None, irodori_caption_base_url: None, irodori_reference_audio_base_url: None },
                ui: UIConfig { theme: Theme::Dark, language: "ja".to_string(), send_key: SendKey::default() },
                plugins: PluginsConfig { enabled_plugins: vec![], plugin_settings: HashMap::new() },
                attachment: AttachmentConfig { max_file_size_bytes: 10 * 1024 * 1024, allowed_extensions: vec!["txt".to_string()] },
            };
            let config_manager = Arc::new(ModelConfigManager::new_with_config(app_config));

            let plugins: Vec<Box<dyn PluginHandler>> = vec![
                Box::new(CalculatorPlugin::new()),
                Box::new(WebSearchPlugin::new(config_manager)),
                Box::new(FileOpsPlugin::new(
                    std::path::PathBuf::from("."),
                    Arc::new(std::sync::Mutex::new(
                        crate::db::database::Database::open_in_memory().unwrap()
                    )),
                )),
            ];

            for plugin in &plugins {
                let tools = plugin.tools();
                prop_assert!(!tools.is_empty());

                for tool in &tools {
                    prop_assert!(!tool.name.is_empty());
                    prop_assert!(!tool.description.is_empty());
                    prop_assert!(tool.parameters.is_object());
                    let params_obj = tool.parameters.as_object().unwrap();
                    prop_assert!(params_obj.contains_key("type"));
                }
            }
        }
    }

    // ========================================
    // Property 24: Tool call dispatch correctness
    // ========================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_tool_call_dispatches_to_correct_plugin(
            names in unique_plugin_names(3),
            target_idx in 0usize..3,
            call_id in tool_call_id_strategy(),
        ) {
            let target_idx = target_idx % names.len();

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = make_mock_app();
                let registry = Arc::new(TestRegistry::new());

                for name in &names {
                    let plugin = MockPlugin::new(name);
                    registry.register(Box::new(plugin)).unwrap();
                }

                let system = DefaultPluginSystem::new(registry);

                let target_name = &names[target_idx];
                let tool_name = format!("{}_tool", target_name);

                let tool_calls = vec![ToolCall {
                    id: call_id.clone(),
                    name: tool_name,
                    arguments: json!({"input": "test_data"}),
                    context: None,
                }];

                let results = system.handle_tool_calls(&tool_calls, app.handle()).await.unwrap();

                prop_assert_eq!(results.len(), 1);
                prop_assert!(!results[0].is_error);
                let expected_marker = format!("[{}]", target_name);
                prop_assert!(results[0].content.contains(&expected_marker));

                Ok(())
            })?;
        }

        #[test]
        fn prop_nonexistent_tool_returns_error(
            call_id in tool_call_id_strategy(),
            fake_tool in "[a-z]{5,10}_nonexistent",
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = make_mock_app();
                let registry = Arc::new(TestRegistry::new());
                let plugin = MockPlugin::new("existing");
                registry.register(Box::new(plugin)).unwrap();

                let system = DefaultPluginSystem::new(registry);

                let tool_calls = vec![ToolCall {
                    id: call_id,
                    name: fake_tool.clone(),
                    arguments: json!({}),
                    context: None,
                }];

                let results = system.handle_tool_calls(&tool_calls, app.handle()).await.unwrap();

                prop_assert_eq!(results.len(), 1);
                prop_assert!(results[0].is_error);

                Ok(())
            })?;
        }

        #[test]
        fn prop_disabled_plugin_tool_returns_error(
            name in plugin_name_strategy(),
            call_id in tool_call_id_strategy(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = make_mock_app();
                let registry = Arc::new(TestRegistry::new());
                let plugin = MockPlugin::new(&name);
                registry.register(Box::new(plugin)).unwrap();
                registry.disable_plugin(&name).unwrap();

                let system = DefaultPluginSystem::new(registry);

                let tool_name = format!("{}_tool", name);
                let tool_calls = vec![ToolCall {
                    id: call_id,
                    name: tool_name.clone(),
                    arguments: json!({"input": "test"}),
                    context: None,
                }];

                let results = system.handle_tool_calls(&tool_calls, app.handle()).await.unwrap();

                prop_assert_eq!(results.len(), 1);
                prop_assert!(results[0].is_error);

                Ok(())
            })?;
        }
    }

    // ========================================
    // Property 25: Tool result propagation to LLM
    // ========================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_tool_result_contains_original_call_id(
            name in plugin_name_strategy(),
            call_id in tool_call_id_strategy(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = make_mock_app();
                let registry = Arc::new(TestRegistry::new());
                let plugin = MockPlugin::new(&name);
                registry.register(Box::new(plugin)).unwrap();

                let system = DefaultPluginSystem::new(registry);

                let tool_name = format!("{}_tool", name);
                let tool_calls = vec![ToolCall {
                    id: call_id.clone(),
                    name: tool_name,
                    arguments: json!({"input": "propagation_test"}),
                    context: None,
                }];

                let results = system.handle_tool_calls(&tool_calls, app.handle()).await.unwrap();

                prop_assert_eq!(results.len(), 1);
                prop_assert_eq!(&results[0].tool_call_id, &call_id);

                Ok(())
            })?;
        }

        #[test]
        fn prop_multiple_tool_results_preserve_call_ids(
            names in unique_plugin_names(3),
            call_ids in proptest::collection::vec(tool_call_id_strategy(), 3),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = make_mock_app();
                let registry = Arc::new(TestRegistry::new());

                for name in &names {
                    let plugin = MockPlugin::new(name);
                    registry.register(Box::new(plugin)).unwrap();
                }

                let system = DefaultPluginSystem::new(registry);

                let tool_calls: Vec<ToolCall> = names
                    .iter()
                    .zip(call_ids.iter())
                    .map(|(name, id)| ToolCall {
                        id: id.clone(),
                        name: format!("{}_tool", name),
                        arguments: json!({"input": "multi_test"}),
                        context: None,
                    })
                    .collect();

                let results = system.handle_tool_calls(&tool_calls, app.handle()).await.unwrap();

                prop_assert_eq!(results.len(), tool_calls.len());

                for (result, expected_id) in results.iter().zip(call_ids.iter()) {
                    prop_assert_eq!(&result.tool_call_id, expected_id);
                }

                Ok(())
            })?;
        }

        #[test]
        fn prop_error_result_also_contains_call_id(
            call_id in tool_call_id_strategy(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = make_mock_app();
                let registry = Arc::new(TestRegistry::new());
                let system = DefaultPluginSystem::new(registry);

                let tool_calls = vec![ToolCall {
                    id: call_id.clone(),
                    name: "nonexistent_tool".to_string(),
                    arguments: json!({}),
                    context: None,
                }];

                let results = system.handle_tool_calls(&tool_calls, app.handle()).await.unwrap();

                prop_assert_eq!(results.len(), 1);
                prop_assert!(results[0].is_error);
                prop_assert_eq!(&results[0].tool_call_id, &call_id);

                Ok(())
            })?;
        }
    }

    // ========================================
    // Property 26: Plugin config persistence round-trip
    // ========================================

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_config_round_trip(
            name in plugin_name_strategy(),
            config in json_config_strategy(),
        ) {
            let registry = TestRegistry::new();
            let plugin = MockPlugin::new(&name);
            registry.register(Box::new(plugin)).unwrap();

            registry.set_plugin_config(&name, config.clone()).unwrap();

            let retrieved = registry.get_plugin_config(&name);
            prop_assert_eq!(retrieved, Some(config));
        }

        #[test]
        fn prop_config_update_overwrites_previous(
            name in plugin_name_strategy(),
            config1 in json_config_strategy(),
            config2 in json_config_strategy(),
        ) {
            let registry = TestRegistry::new();
            let plugin = MockPlugin::new(&name);
            registry.register(Box::new(plugin)).unwrap();

            registry.set_plugin_config(&name, config1).unwrap();
            registry.set_plugin_config(&name, config2.clone()).unwrap();

            let retrieved = registry.get_plugin_config(&name);
            prop_assert_eq!(retrieved, Some(config2));
        }

        #[test]
        fn prop_config_isolation_between_plugins(
            names in unique_plugin_names(2),
            config_a in json_config_strategy(),
            config_b in json_config_strategy(),
        ) {
            let registry = TestRegistry::new();

            for name in &names {
                let plugin = MockPlugin::new(name);
                registry.register(Box::new(plugin)).unwrap();
            }

            registry.set_plugin_config(&names[0], config_a.clone()).unwrap();
            registry.set_plugin_config(&names[1], config_b.clone()).unwrap();

            let retrieved_a = registry.get_plugin_config(&names[0]);
            let retrieved_b = registry.get_plugin_config(&names[1]);

            prop_assert_eq!(retrieved_a, Some(config_a));
            prop_assert_eq!(retrieved_b, Some(config_b));
        }
    }
}
