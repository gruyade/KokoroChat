//! LLMクライアントのプロパティテスト
//! proptest を使用してOpenAI互換APIフォーマットの不変条件を検証する。

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::llm::client::{ChatMessage, LLMClientConfig, MessageRole, OpenAICompatibleClient};
    use crate::models::ToolDefinition;

    // ========================================
    // Arbitrary Strategies
    // ========================================

    /// MessageRole のストラテジー
    fn arb_message_role() -> impl Strategy<Value = MessageRole> {
        prop_oneof![
            Just(MessageRole::System),
            Just(MessageRole::User),
            Just(MessageRole::Assistant),
            Just(MessageRole::Tool),
        ]
    }

    /// ChatMessage のストラテジー
    fn arb_chat_message() -> impl Strategy<Value = ChatMessage> {
        (
            arb_message_role(),
            "[a-zA-Z0-9 ぁ-んァ-ヶ]{1,100}",
            proptest::option::of("[a-z0-9_]{4,20}"),
        )
            .prop_map(|(role, content, tool_call_id)| ChatMessage {
                role,
                content,
                tool_call_id,
                images: None,
            })
    }

    /// LLMClientConfig のストラテジー
    fn arb_llm_client_config() -> impl Strategy<Value = LLMClientConfig> {
        (
            "http://[a-z]{3,10}:[0-9]{4}/v[0-9]",
            "[a-z]{2,8}-[0-9]{1,2}",
            proptest::option::of("sk-[a-zA-Z0-9]{10,30}"),
            0.0f32..2.0,
        )
            .prop_map(|(base_url, model, api_key, temperature)| LLMClientConfig {
                base_url,
                model,
                api_key,
                temperature,
                provider: None,
            })
    }

    /// ToolDefinition のストラテジー
    fn arb_tool_definition() -> impl Strategy<Value = ToolDefinition> {
        (
            "[a-z_]{3,20}",
            "[a-zA-Z ]{5,50}",
        )
            .prop_map(|(name, description)| ToolDefinition {
                name,
                description,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "input": {"type": "string"}
                    },
                    "required": ["input"]
                }),
            })
    }

    // ========================================
    // Property 17: LLM request OpenAI format compliance
    // ========================================
    // **Validates: Requirements 7.3**
    //
    // For any valid set of ChatMessages and LLMClientConfig, the request body
    // built by `build_request_body` SHALL:
    // 1. Always contain "model" field matching config.model
    // 2. Always contain "messages" array with correct role/content mapping
    // 3. Always contain "temperature" field matching config.temperature
    // 4. Always contain "stream" field (boolean)
    // 5. When tools are provided, contain "tools" array with correct function format
    // 6. When tool_call_id is present on a message, include it in the serialized message
    // 7. Message roles are serialized as lowercase strings ("system", "user", "assistant", "tool")

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// Sub-property 1: "model" field matches config.model
        #[test]
        fn prop_request_body_contains_model(
            messages in proptest::collection::vec(arb_chat_message(), 1..5),
            config in arb_llm_client_config(),
            stream in proptest::bool::ANY,
        ) {
            let client = OpenAICompatibleClient::new();
            let body = client.build_request_body(&messages, &config, None, stream);

            let model_value = body.get("model").expect("body must contain 'model' field");
            prop_assert_eq!(model_value.as_str().unwrap(), config.model.as_str());
        }

        /// Sub-property 2: "messages" array with correct role/content mapping
        #[test]
        fn prop_request_body_messages_mapping(
            messages in proptest::collection::vec(arb_chat_message(), 1..8),
            config in arb_llm_client_config(),
        ) {
            let client = OpenAICompatibleClient::new();
            let body = client.build_request_body(&messages, &config, None, false);

            let msgs_arr = body["messages"]
                .as_array()
                .expect("body must contain 'messages' array");

            prop_assert_eq!(msgs_arr.len(), messages.len());

            for (i, (msg_json, msg_orig)) in msgs_arr.iter().zip(messages.iter()).enumerate() {
                // content一致
                prop_assert_eq!(
                    msg_json["content"].as_str().unwrap(),
                    msg_orig.content.as_str(),
                    "Message {} content mismatch", i
                );

                // role存在
                prop_assert!(
                    msg_json.get("role").is_some(),
                    "Message {} must have 'role' field", i
                );
            }
        }

        /// Sub-property 3: "temperature" field matches config.temperature
        #[test]
        fn prop_request_body_contains_temperature(
            messages in proptest::collection::vec(arb_chat_message(), 1..3),
            config in arb_llm_client_config(),
        ) {
            let client = OpenAICompatibleClient::new();
            let body = client.build_request_body(&messages, &config, None, false);

            let temp_value = body.get("temperature").expect("body must contain 'temperature' field");
            let temp_f64 = temp_value.as_f64().unwrap();
            // f32→f64変換の精度を考慮して近似比較
            prop_assert!(
                (temp_f64 - config.temperature as f64).abs() < 1e-5,
                "temperature mismatch: got {}, expected {}",
                temp_f64,
                config.temperature
            );
        }

        /// Sub-property 4: "stream" field is boolean
        #[test]
        fn prop_request_body_contains_stream(
            messages in proptest::collection::vec(arb_chat_message(), 1..3),
            config in arb_llm_client_config(),
            stream in proptest::bool::ANY,
        ) {
            let client = OpenAICompatibleClient::new();
            let body = client.build_request_body(&messages, &config, None, stream);

            let stream_value = body.get("stream").expect("body must contain 'stream' field");
            prop_assert!(stream_value.is_boolean(), "'stream' must be a boolean");
            prop_assert_eq!(stream_value.as_bool().unwrap(), stream);
        }

        /// Sub-property 5: When tools are provided, "tools" array with correct function format
        #[test]
        fn prop_request_body_tools_format(
            messages in proptest::collection::vec(arb_chat_message(), 1..3),
            config in arb_llm_client_config(),
            tools in proptest::collection::vec(arb_tool_definition(), 1..4),
        ) {
            let client = OpenAICompatibleClient::new();
            let body = client.build_request_body(&messages, &config, Some(&tools), false);

            let tools_arr = body["tools"]
                .as_array()
                .expect("body must contain 'tools' array when tools provided");

            prop_assert_eq!(tools_arr.len(), tools.len());

            for (i, (tool_json, tool_orig)) in tools_arr.iter().zip(tools.iter()).enumerate() {
                // type == "function"
                prop_assert_eq!(
                    tool_json["type"].as_str().unwrap(),
                    "function",
                    "Tool {} must have type 'function'", i
                );

                // function.name一致
                prop_assert_eq!(
                    tool_json["function"]["name"].as_str().unwrap(),
                    tool_orig.name.as_str(),
                    "Tool {} name mismatch", i
                );

                // function.description一致
                prop_assert_eq!(
                    tool_json["function"]["description"].as_str().unwrap(),
                    tool_orig.description.as_str(),
                    "Tool {} description mismatch", i
                );

                // function.parameters存在
                prop_assert!(
                    tool_json["function"].get("parameters").is_some(),
                    "Tool {} must have 'parameters' field", i
                );
            }
        }

        /// Sub-property 6: When tool_call_id is present, include it in serialized message
        #[test]
        fn prop_request_body_tool_call_id_inclusion(
            content in "[a-zA-Z0-9]{1,50}",
            tool_call_id in "[a-z0-9_]{4,20}",
            config in arb_llm_client_config(),
        ) {
            let client = OpenAICompatibleClient::new();
            let messages = vec![ChatMessage {
                role: MessageRole::Tool,
                content: content.clone(),
                tool_call_id: Some(tool_call_id.clone()),
                images: None,
            }];

            let body = client.build_request_body(&messages, &config, None, false);
            let msgs_arr = body["messages"].as_array().unwrap();

            prop_assert_eq!(
                msgs_arr[0]["tool_call_id"].as_str().unwrap(),
                tool_call_id.as_str(),
                "tool_call_id must be included when present"
            );
        }

        /// Sub-property 7: Message roles are serialized as lowercase strings
        #[test]
        fn prop_request_body_roles_lowercase(
            messages in proptest::collection::vec(arb_chat_message(), 1..8),
            config in arb_llm_client_config(),
        ) {
            let client = OpenAICompatibleClient::new();
            let body = client.build_request_body(&messages, &config, None, false);

            let msgs_arr = body["messages"].as_array().unwrap();
            let valid_roles = ["system", "user", "assistant", "tool"];

            for (i, (msg_json, msg_orig)) in msgs_arr.iter().zip(messages.iter()).enumerate() {
                let role_str = msg_json["role"]
                    .as_str()
                    .expect(&format!("Message {} role must be a string", i));

                prop_assert!(
                    valid_roles.contains(&role_str),
                    "Message {} role '{}' is not a valid lowercase role string",
                    i,
                    role_str
                );

                // 正しいロールにマッピングされている
                let expected_role = match msg_orig.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Tool => "tool",
                };
                prop_assert_eq!(
                    role_str,
                    expected_role,
                    "Message {} role mapping incorrect", i
                );
            }
        }
    }
}


// Feature: app-enhancements-v3, Property 2: プロバイダー×エンドポイントによるAPI形式決定
#[cfg(test)]
mod api_strategy_tests {
    use proptest::prelude::*;

    use crate::llm::client::{ApiStrategy, LLMClientConfig, is_default_endpoint, resolve_api_strategy};
    use crate::models::LLMProvider;

    // ========================================
    // Arbitrary Strategies
    // ========================================

    /// LLMProvider のストラテジー（将来の拡張用に残す）
    #[allow(dead_code)]
    fn arb_provider() -> impl Strategy<Value = LLMProvider> {
        prop_oneof![
            Just(LLMProvider::Openai),
            Just(LLMProvider::Anthropic),
            Just(LLMProvider::Google),
            Just(LLMProvider::OpenaiCompatible),
        ]
    }

    /// Google デフォルトエンドポイントのストラテジー
    fn arb_google_default_endpoint() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(String::new()),
            Just("https://generativelanguage.googleapis.com/v1beta".to_string()),
            Just("https://generativelanguage.googleapis.com".to_string()),
        ]
    }

    /// Anthropic デフォルトエンドポイントのストラテジー
    fn arb_anthropic_default_endpoint() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(String::new()),
            Just("https://api.anthropic.com/v1".to_string()),
            Just("https://api.anthropic.com".to_string()),
        ]
    }

    /// カスタムエンドポイント（デフォルトではない）のストラテジー
    fn arb_custom_endpoint() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("http://localhost:8080/v1".to_string()),
            Just("https://my-proxy.example.com/api".to_string()),
            Just("https://custom-server.local:3000".to_string()),
            "[a-z]{3,8}\\.[a-z]{2,5}".prop_map(|s| format!("https://{}/v1", s)),
        ]
    }

    /// 任意のbase_url（デフォルトまたはカスタム）のストラテジー
    fn arb_base_url() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(String::new()),
            arb_google_default_endpoint(),
            arb_anthropic_default_endpoint(),
            arb_custom_endpoint(),
        ]
    }



    // ========================================
    // Property 2: プロバイダー×エンドポイントによるAPI形式決定
    // ========================================
    // **Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        /// provider=Google + デフォルトエンドポイント → OpenAI（応急措置: 常にOpenAI互換）
        #[test]
        fn prop_google_default_endpoint_returns_openai(
            base_url in arb_google_default_endpoint(),
            model in "[a-z]{2,8}-[0-9]{1,2}",
            api_key in proptest::option::of("sk-[a-zA-Z0-9]{10,20}"),
            temperature in 0.0f32..2.0,
        ) {
            let config = LLMClientConfig {
                base_url,
                model,
                api_key,
                temperature,
                provider: Some(LLMProvider::Google),
            };
            let strategy = resolve_api_strategy(&config);
            prop_assert_eq!(strategy, ApiStrategy::OpenAI,
                "Google + default endpoint should resolve to OpenAI (workaround)");
        }

        /// provider=Anthropic + デフォルトエンドポイント → Anthropic（ネイティブAPI使用）
        #[test]
        fn prop_anthropic_default_endpoint_returns_anthropic(
            base_url in arb_anthropic_default_endpoint(),
            model in "[a-z]{2,8}-[0-9]{1,2}",
            api_key in proptest::option::of("sk-[a-zA-Z0-9]{10,20}"),
            temperature in 0.0f32..2.0,
        ) {
            let config = LLMClientConfig {
                base_url,
                model,
                api_key,
                temperature,
                provider: Some(LLMProvider::Anthropic),
            };
            let strategy = resolve_api_strategy(&config);
            prop_assert_eq!(strategy, ApiStrategy::Anthropic,
                "Anthropic should always resolve to Anthropic strategy");
        }

        /// provider=Google + カスタムエンドポイント → OpenAI
        #[test]
        fn prop_google_custom_endpoint_returns_openai(
            base_url in arb_custom_endpoint(),
            model in "[a-z]{2,8}-[0-9]{1,2}",
            api_key in proptest::option::of("sk-[a-zA-Z0-9]{10,20}"),
            temperature in 0.0f32..2.0,
        ) {
            let config = LLMClientConfig {
                base_url,
                model,
                api_key,
                temperature,
                provider: Some(LLMProvider::Google),
            };
            let strategy = resolve_api_strategy(&config);
            prop_assert_eq!(strategy, ApiStrategy::OpenAI,
                "Google + custom endpoint should resolve to OpenAI");
        }

        /// provider=Anthropic + カスタムエンドポイント → Anthropic（常にネイティブAPI）
        #[test]
        fn prop_anthropic_custom_endpoint_returns_anthropic(
            base_url in arb_custom_endpoint(),
            model in "[a-z]{2,8}-[0-9]{1,2}",
            api_key in proptest::option::of("sk-[a-zA-Z0-9]{10,20}"),
            temperature in 0.0f32..2.0,
        ) {
            let config = LLMClientConfig {
                base_url,
                model,
                api_key,
                temperature,
                provider: Some(LLMProvider::Anthropic),
            };
            let strategy = resolve_api_strategy(&config);
            prop_assert_eq!(strategy, ApiStrategy::Anthropic,
                "Anthropic should always resolve to Anthropic strategy");
        }

        /// provider=Openai → OpenAI（エンドポイント問わず）
        #[test]
        fn prop_openai_provider_always_returns_openai(
            base_url in arb_base_url(),
            model in "[a-z]{2,8}-[0-9]{1,2}",
            api_key in proptest::option::of("sk-[a-zA-Z0-9]{10,20}"),
            temperature in 0.0f32..2.0,
        ) {
            let config = LLMClientConfig {
                base_url,
                model,
                api_key,
                temperature,
                provider: Some(LLMProvider::Openai),
            };
            let strategy = resolve_api_strategy(&config);
            prop_assert_eq!(strategy, ApiStrategy::OpenAI,
                "Openai provider should always resolve to OpenAI");
        }

        /// provider=OpenaiCompatible → OpenAI（エンドポイント問わず）
        #[test]
        fn prop_openai_compatible_always_returns_openai(
            base_url in arb_base_url(),
            model in "[a-z]{2,8}-[0-9]{1,2}",
            api_key in proptest::option::of("sk-[a-zA-Z0-9]{10,20}"),
            temperature in 0.0f32..2.0,
        ) {
            let config = LLMClientConfig {
                base_url,
                model,
                api_key,
                temperature,
                provider: Some(LLMProvider::OpenaiCompatible),
            };
            let strategy = resolve_api_strategy(&config);
            prop_assert_eq!(strategy, ApiStrategy::OpenAI,
                "OpenaiCompatible provider should always resolve to OpenAI");
        }

        /// provider=None → OpenAI（エンドポイント問わず）
        #[test]
        fn prop_none_provider_always_returns_openai(
            base_url in arb_base_url(),
            model in "[a-z]{2,8}-[0-9]{1,2}",
            api_key in proptest::option::of("sk-[a-zA-Z0-9]{10,20}"),
            temperature in 0.0f32..2.0,
        ) {
            let config = LLMClientConfig {
                base_url,
                model,
                api_key,
                temperature,
                provider: None,
            };
            let strategy = resolve_api_strategy(&config);
            prop_assert_eq!(strategy, ApiStrategy::OpenAI,
                "None provider should always resolve to OpenAI");
        }

        /// is_default_endpoint: Googleデフォルトエンドポイント判定
        #[test]
        fn prop_is_default_endpoint_google(
            base_url in arb_google_default_endpoint(),
        ) {
            prop_assert!(is_default_endpoint(&base_url, LLMProvider::Google),
                "Google default endpoints should be recognized as default");
        }

        /// is_default_endpoint: Anthropicデフォルトエンドポイント判定
        #[test]
        fn prop_is_default_endpoint_anthropic(
            base_url in arb_anthropic_default_endpoint(),
        ) {
            prop_assert!(is_default_endpoint(&base_url, LLMProvider::Anthropic),
                "Anthropic default endpoints should be recognized as default");
        }

        /// is_default_endpoint: カスタムエンドポイントはデフォルトではない
        #[test]
        fn prop_custom_endpoint_not_default_for_google(
            base_url in arb_custom_endpoint(),
        ) {
            prop_assert!(!is_default_endpoint(&base_url, LLMProvider::Google),
                "Custom endpoints should not be recognized as Google default");
        }

        /// is_default_endpoint: カスタムエンドポイントはAnthropicデフォルトではない
        #[test]
        fn prop_custom_endpoint_not_default_for_anthropic(
            base_url in arb_custom_endpoint(),
        ) {
            prop_assert!(!is_default_endpoint(&base_url, LLMProvider::Anthropic),
                "Custom endpoints should not be recognized as Anthropic default");
        }
    }
}


// Feature: app-enhancements-v3, Property 3: プロバイダー別レスポンスパースの正当性
#[cfg(test)]
mod response_parse_tests {
    use proptest::prelude::*;
    use serde_json::json;

    use crate::llm::client::{parse_anthropic_response, parse_gemini_response, LLMResponse};

    // ========================================
    // Arbitrary Strategies
    // ========================================

    /// 非空テキストのストラテジー
    fn arb_non_empty_text() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ぁ-んァ-ヶ]{1,200}"
    }

    /// 有効なGeminiレスポンスJSON（非空テキスト）のストラテジー
    fn arb_valid_gemini_response() -> impl Strategy<Value = (serde_json::Value, String)> {
        arb_non_empty_text().prop_map(|text| {
            let body = json!({
                "candidates": [{
                    "content": {
                        "parts": [{"text": text.clone()}]
                    }
                }]
            });
            (body, text)
        })
    }

    /// 有効なAnthropicレスポンスJSON（1〜4個のテキストブロック）のストラテジー
    fn arb_valid_anthropic_response(
    ) -> impl Strategy<Value = (serde_json::Value, String)> {
        proptest::collection::vec(arb_non_empty_text(), 1..=4).prop_map(|texts| {
            let expected = texts.join("");
            let content: Vec<serde_json::Value> = texts
                .into_iter()
                .map(|t| json!({"type": "text", "text": t}))
                .collect();
            let body = json!({ "content": content });
            (body, expected)
        })
    }

    // ========================================
    // Property 3: プロバイダー別レスポンスパースの正当性
    // ========================================
    // **Validates: Requirements 5.7**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        /// Sub-property 1: 有効なGeminiレスポンスJSONに対し、parse_gemini_responseはOk(LLMResponse::Text(text))を返し、textが入力と一致する
        #[test]
        fn prop_parse_gemini_response_valid(
            (body, expected_text) in arb_valid_gemini_response(),
        ) {
            let result = parse_gemini_response(&body);
            prop_assert!(result.is_ok(), "parse_gemini_response should return Ok for valid input");
            match result.unwrap() {
                LLMResponse::Text(text) => {
                    prop_assert_eq!(text, expected_text,
                        "parsed text should match input text");
                }
                other => {
                    prop_assert!(false, "Expected LLMResponse::Text, got {:?}", other);
                }
            }
        }

        /// Sub-property 2: 有効なAnthropicレスポンスJSONに対し、parse_anthropic_responseはOk(LLMResponse::Text(text))を返し、textが全テキストブロックの結合と一致する
        #[test]
        fn prop_parse_anthropic_response_valid(
            (body, expected_text) in arb_valid_anthropic_response(),
        ) {
            let result = parse_anthropic_response(&body);
            prop_assert!(result.is_ok(), "parse_anthropic_response should return Ok for valid input");
            match result.unwrap() {
                LLMResponse::Text(text) => {
                    prop_assert_eq!(text, expected_text,
                        "parsed text should be concatenation of all text blocks");
                }
                other => {
                    prop_assert!(false, "Expected LLMResponse::Text, got {:?}", other);
                }
            }
        }

        /// Sub-property 3: Geminiレスポンスのcandidatesが空配列の場合、parse_gemini_responseはErrを返す
        #[test]
        fn prop_parse_gemini_response_empty_candidates(
            _dummy in 0..20u32,
        ) {
            let body = json!({
                "candidates": []
            });
            let result = parse_gemini_response(&body);
            prop_assert!(result.is_err(),
                "parse_gemini_response should return Err for empty candidates");
        }
    }
}
