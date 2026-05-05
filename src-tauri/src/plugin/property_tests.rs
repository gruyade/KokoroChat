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
    impl PluginHandler for MockPlugin {
        fn name(&self) -> &str {
            &self.plugin_name
        }

        fn description(&self) -> &str {
            &self.plugin_description
        }

        fn tools(&self) -> Vec<ToolDefinition> {
            self.tool_defs.clone()
        }

        async fn execute(&self, tool_call: &ToolCall) -> Result<ToolResult, AppError> {
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
    //
    // **Validates: Requirements 11.2**
    //
    // For any PluginHandler registered in the Plugin Registry, registering the
    // same plugin (by name) again SHALL fail with an error. The registry SHALL
    // contain exactly one entry per unique plugin name.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_duplicate_registration_fails(
            name in plugin_name_strategy(),
        ) {
            let registry = DefaultPluginRegistry::new();

            // 1回目の登録: 成功
            let plugin1 = MockPlugin::new(&name);
            let result1 = registry.register(Box::new(plugin1));
            prop_assert!(
                result1.is_ok(),
                "First registration of '{}' should succeed",
                name
            );

            // 2回目の登録: 失敗
            let plugin2 = MockPlugin::new(&name);
            let result2 = registry.register(Box::new(plugin2));
            prop_assert!(
                result2.is_err(),
                "Second registration of '{}' should fail",
                name
            );

            // レジストリには1エントリのみ
            let plugins = registry.list_plugins();
            prop_assert_eq!(
                plugins.len(),
                1,
                "Registry should contain exactly 1 entry for '{}', got {}",
                name,
                plugins.len()
            );

            // エラーがPlugin種別であること
            match result2.unwrap_err() {
                AppError::Plugin(msg) => {
                    prop_assert!(
                        msg.contains("already registered"),
                        "Error should mention 'already registered', got: {}",
                        msg
                    );
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
            let registry = DefaultPluginRegistry::new();

            for name in &names {
                let plugin = MockPlugin::new(name);
                let result = registry.register(Box::new(plugin));
                prop_assert!(
                    result.is_ok(),
                    "Registration of unique name '{}' should succeed",
                    name
                );
            }

            let plugins = registry.list_plugins();
            prop_assert_eq!(
                plugins.len(),
                names.len(),
                "Registry should contain exactly {} entries",
                names.len()
            );
        }
    }

    // ========================================
    // Property 22: Plugin enable/disable isolation
    // ========================================
    //
    // **Validates: Requirements 11.2**
    //
    // For any set of registered Plugins, enabling or disabling one Plugin SHALL
    // NOT affect the enabled state of other Plugins.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_disable_one_does_not_affect_others(
            names in unique_plugin_names(3),
            target_idx in 0usize..3,
        ) {
            let target_idx = target_idx % names.len();
            let registry = DefaultPluginRegistry::new();

            // 全プラグイン登録（初期状態: 全て有効）
            for name in &names {
                let plugin = MockPlugin::new(name);
                registry.register(Box::new(plugin)).unwrap();
            }

            // ターゲットを無効化
            let target_name = &names[target_idx];
            registry.disable_plugin(target_name).unwrap();

            // 他のプラグインの状態を確認
            let plugins = registry.list_plugins();
            for plugin_info in &plugins {
                if &plugin_info.name == target_name {
                    prop_assert!(
                        !plugin_info.enabled,
                        "Target plugin '{}' should be disabled",
                        target_name
                    );
                } else {
                    prop_assert!(
                        plugin_info.enabled,
                        "Non-target plugin '{}' should remain enabled after disabling '{}'",
                        plugin_info.name,
                        target_name
                    );
                }
            }
        }

        #[test]
        fn prop_enable_one_does_not_affect_others(
            names in unique_plugin_names(3),
            target_idx in 0usize..3,
        ) {
            let target_idx = target_idx % names.len();
            let registry = DefaultPluginRegistry::new();

            // 全プラグイン登録
            for name in &names {
                let plugin = MockPlugin::new(name);
                registry.register(Box::new(plugin)).unwrap();
            }

            // 全て無効化
            for name in &names {
                registry.disable_plugin(name).unwrap();
            }

            // ターゲットのみ有効化
            let target_name = &names[target_idx];
            registry.enable_plugin(target_name).unwrap();

            // 他のプラグインの状態を確認
            let plugins = registry.list_plugins();
            for plugin_info in &plugins {
                if &plugin_info.name == target_name {
                    prop_assert!(
                        plugin_info.enabled,
                        "Target plugin '{}' should be enabled",
                        target_name
                    );
                } else {
                    prop_assert!(
                        !plugin_info.enabled,
                        "Non-target plugin '{}' should remain disabled after enabling '{}'",
                        plugin_info.name,
                        target_name
                    );
                }
            }
        }
    }

    // ========================================
    // Property 23: Tool definition format compliance
    // ========================================
    //
    // **Validates: Requirements 11.1**
    //
    // For any registered plugin, its tool definitions SHALL have non-empty name,
    // description, and valid JSON Schema parameters.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_tool_definitions_have_valid_format(
            name in plugin_name_strategy(),
        ) {
            let registry = DefaultPluginRegistry::new();
            let plugin = MockPlugin::new(&name);
            registry.register(Box::new(plugin)).unwrap();

            let tools = registry.get_enabled_tools();

            for tool in &tools {
                // name が非空
                prop_assert!(
                    !tool.name.is_empty(),
                    "Tool name should not be empty"
                );

                // description が非空
                prop_assert!(
                    !tool.description.is_empty(),
                    "Tool description should not be empty"
                );

                // parameters が JSON Object であること（JSON Schema準拠）
                prop_assert!(
                    tool.parameters.is_object(),
                    "Tool parameters should be a JSON object, got: {:?}",
                    tool.parameters
                );

                // parameters に "type" フィールドが存在すること
                let params_obj = tool.parameters.as_object().unwrap();
                prop_assert!(
                    params_obj.contains_key("type"),
                    "Tool parameters JSON Schema should have 'type' field"
                );
            }
        }

        #[test]
        fn prop_builtin_plugins_tool_format_compliance(
            _dummy in Just(()),
        ) {
            // 組み込みプラグインのツール定義もフォーマット準拠を確認
            use crate::plugin::builtin::calculator::CalculatorPlugin;
            use crate::plugin::builtin::web_search::WebSearchPlugin;
            use crate::plugin::builtin::file_ops::FileOpsPlugin;

            let plugins: Vec<Box<dyn PluginHandler>> = vec![
                Box::new(CalculatorPlugin::new()),
                Box::new(WebSearchPlugin::new()),
                Box::new(FileOpsPlugin::new(std::path::PathBuf::from("."))),
            ];

            for plugin in &plugins {
                let tools = plugin.tools();
                prop_assert!(
                    !tools.is_empty(),
                    "Plugin '{}' should provide at least one tool",
                    plugin.name()
                );

                for tool in &tools {
                    prop_assert!(
                        !tool.name.is_empty(),
                        "Tool name in plugin '{}' should not be empty",
                        plugin.name()
                    );
                    prop_assert!(
                        !tool.description.is_empty(),
                        "Tool description in plugin '{}' should not be empty",
                        plugin.name()
                    );
                    prop_assert!(
                        tool.parameters.is_object(),
                        "Tool '{}' parameters should be a JSON object",
                        tool.name
                    );
                    let params_obj = tool.parameters.as_object().unwrap();
                    prop_assert!(
                        params_obj.contains_key("type"),
                        "Tool '{}' parameters should have 'type' field",
                        tool.name
                    );
                }
            }
        }
    }

    // ========================================
    // Property 24: Tool call dispatch correctness
    // ========================================
    //
    // **Validates: Requirements 11.3, 11.7**
    //
    // For any tool_call with a name matching an enabled plugin's tool,
    // execute_tool SHALL dispatch to that plugin. For tool names not matching
    // any enabled plugin, the system SHALL return an error ToolResult.

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
                let registry = Arc::new(DefaultPluginRegistry::new());

                for name in &names {
                    let plugin = MockPlugin::new(name);
                    registry.register(Box::new(plugin)).unwrap();
                }

                let system = DefaultPluginSystem::new(registry);

                // ターゲットプラグインのツールを呼び出し
                let target_name = &names[target_idx];
                let tool_name = format!("{}_tool", target_name);

                let tool_calls = vec![ToolCall {
                    id: call_id.clone(),
                    name: tool_name,
                    arguments: json!({"input": "test_data"}),
                }];

                let results = system.handle_tool_calls(&tool_calls).await.unwrap();

                prop_assert_eq!(results.len(), 1);
                prop_assert!(!results[0].is_error, "Tool call should succeed");

                // 正しいプラグインにディスパッチされたことを確認
                prop_assert!(
                    results[0].content.contains(&format!("[{}]", target_name)),
                    "Result should come from plugin '{}', got: {}",
                    target_name,
                    results[0].content
                );

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
                let registry = Arc::new(DefaultPluginRegistry::new());
                let plugin = MockPlugin::new("existing");
                registry.register(Box::new(plugin)).unwrap();

                let system = DefaultPluginSystem::new(registry);

                let tool_calls = vec![ToolCall {
                    id: call_id,
                    name: fake_tool.clone(),
                    arguments: json!({}),
                }];

                let results = system.handle_tool_calls(&tool_calls).await.unwrap();

                prop_assert_eq!(results.len(), 1);
                prop_assert!(
                    results[0].is_error,
                    "Non-existent tool '{}' should return error result",
                    fake_tool
                );

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
                let registry = Arc::new(DefaultPluginRegistry::new());
                let plugin = MockPlugin::new(&name);
                registry.register(Box::new(plugin)).unwrap();
                registry.disable_plugin(&name).unwrap();

                let system = DefaultPluginSystem::new(registry);

                let tool_name = format!("{}_tool", name);
                let tool_calls = vec![ToolCall {
                    id: call_id,
                    name: tool_name.clone(),
                    arguments: json!({"input": "test"}),
                }];

                let results = system.handle_tool_calls(&tool_calls).await.unwrap();

                prop_assert_eq!(results.len(), 1);
                prop_assert!(
                    results[0].is_error,
                    "Disabled plugin's tool '{}' should return error result",
                    tool_name
                );

                Ok(())
            })?;
        }
    }

    // ========================================
    // Property 25: Tool result propagation to LLM
    // ========================================
    //
    // **Validates: Requirements 11.3, 11.9**
    //
    // For any tool execution result, the ToolResult SHALL contain the original
    // tool_call_id, enabling the Chat Engine to include it in subsequent LLM
    // requests with the correct reference.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_tool_result_contains_original_call_id(
            name in plugin_name_strategy(),
            call_id in tool_call_id_strategy(),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let registry = Arc::new(DefaultPluginRegistry::new());
                let plugin = MockPlugin::new(&name);
                registry.register(Box::new(plugin)).unwrap();

                let system = DefaultPluginSystem::new(registry);

                let tool_name = format!("{}_tool", name);
                let tool_calls = vec![ToolCall {
                    id: call_id.clone(),
                    name: tool_name,
                    arguments: json!({"input": "propagation_test"}),
                }];

                let results = system.handle_tool_calls(&tool_calls).await.unwrap();

                prop_assert_eq!(results.len(), 1);
                prop_assert_eq!(
                    &results[0].tool_call_id,
                    &call_id,
                    "ToolResult.tool_call_id should match original ToolCall.id"
                );

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
                let registry = Arc::new(DefaultPluginRegistry::new());

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
                    })
                    .collect();

                let results = system.handle_tool_calls(&tool_calls).await.unwrap();

                prop_assert_eq!(results.len(), tool_calls.len());

                // 各結果のtool_call_idが対応するToolCallのidと一致
                for (result, expected_id) in results.iter().zip(call_ids.iter()) {
                    prop_assert_eq!(
                        &result.tool_call_id,
                        expected_id,
                        "Each ToolResult should preserve its original tool_call_id"
                    );
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
                let registry = Arc::new(DefaultPluginRegistry::new());
                let system = DefaultPluginSystem::new(registry);

                // 存在しないツールを呼び出し → エラー結果
                let tool_calls = vec![ToolCall {
                    id: call_id.clone(),
                    name: "nonexistent_tool".to_string(),
                    arguments: json!({}),
                }];

                let results = system.handle_tool_calls(&tool_calls).await.unwrap();

                prop_assert_eq!(results.len(), 1);
                prop_assert!(results[0].is_error);
                prop_assert_eq!(
                    &results[0].tool_call_id,
                    &call_id,
                    "Error ToolResult should also preserve original tool_call_id"
                );

                Ok(())
            })?;
        }
    }

    // ========================================
    // Property 26: Plugin config persistence round-trip
    // ========================================
    //
    // **Validates: Requirements 11.8**
    //
    // For any plugin config set via set_plugin_config, get_plugin_config SHALL
    // return the same value.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_config_round_trip(
            name in plugin_name_strategy(),
            config in json_config_strategy(),
        ) {
            let registry = DefaultPluginRegistry::new();
            let plugin = MockPlugin::new(&name);
            registry.register(Box::new(plugin)).unwrap();

            // 設定を保存
            registry.set_plugin_config(&name, config.clone()).unwrap();

            // 設定を取得して一致確認
            let retrieved = registry.get_plugin_config(&name);
            prop_assert_eq!(
                retrieved,
                Some(config),
                "get_plugin_config should return the same value that was set"
            );
        }

        #[test]
        fn prop_config_update_overwrites_previous(
            name in plugin_name_strategy(),
            config1 in json_config_strategy(),
            config2 in json_config_strategy(),
        ) {
            let registry = DefaultPluginRegistry::new();
            let plugin = MockPlugin::new(&name);
            registry.register(Box::new(plugin)).unwrap();

            // 1回目の設定
            registry.set_plugin_config(&name, config1).unwrap();

            // 2回目の設定（上書き）
            registry.set_plugin_config(&name, config2.clone()).unwrap();

            // 最新の設定が返ること
            let retrieved = registry.get_plugin_config(&name);
            prop_assert_eq!(
                retrieved,
                Some(config2),
                "get_plugin_config should return the latest config"
            );
        }

        #[test]
        fn prop_config_isolation_between_plugins(
            names in unique_plugin_names(2),
            config_a in json_config_strategy(),
            config_b in json_config_strategy(),
        ) {
            let registry = DefaultPluginRegistry::new();

            for name in &names {
                let plugin = MockPlugin::new(name);
                registry.register(Box::new(plugin)).unwrap();
            }

            // 各プラグインに異なる設定を保存
            registry.set_plugin_config(&names[0], config_a.clone()).unwrap();
            registry.set_plugin_config(&names[1], config_b.clone()).unwrap();

            // 各プラグインの設定が独立していること
            let retrieved_a = registry.get_plugin_config(&names[0]);
            let retrieved_b = registry.get_plugin_config(&names[1]);

            prop_assert_eq!(
                retrieved_a,
                Some(config_a),
                "Plugin A's config should be independent"
            );
            prop_assert_eq!(
                retrieved_b,
                Some(config_b),
                "Plugin B's config should be independent"
            );
        }
    }
}
