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
        ("[a-z_]{3,20}", "[a-zA-Z ]{5,50}").prop_map(|(name, description)| ToolDefinition {
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

// Feature: thinking-reasoning-support, Property 2: Think tag extraction across chunk boundaries
#[cfg(test)]
mod think_tag_chunk_boundary_tests {
    use proptest::prelude::*;

    use crate::llm::think_tag_buffer::ThinkTagBuffer;

    // ========================================
    // Arbitrary Strategies
    // ========================================

    /// テキストセグメント（<think>タグを含まない任意テキスト）
    fn arb_text_segment() -> impl Strategy<Value = String> {
        // タグ文字列を避けるため、< と > を含まないテキストを生成
        "[a-zA-Z0-9 ぁ-んァ-ヶ!?.,;:]{0,50}"
    }

    /// <think>...</think>ブロックを含む可能性のある文字列を生成
    /// interleaved text and think blocks
    fn arb_string_with_think_tags() -> impl Strategy<Value = String> {
        // 1〜4個のセグメントを生成し、一部をthinkブロックに包む
        proptest::collection::vec(
            prop_oneof![
                // 通常テキストセグメント
                arb_text_segment(),
                // <think>で囲まれたセグメント
                arb_text_segment()
                    .prop_map(|s| format!("<think>{}</think>", s)),
            ],
            1..=6,
        )
        .prop_map(|segments| segments.join(""))
    }

    /// 文字列を指定した分割点でチャンクに分割（UTF-8境界を考慮）
    fn split_at_points(s: &str, points: &[usize]) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut prev = 0;
        let bytes = s.as_bytes();

        for &point in points {
            // UTF-8境界でなければスキップ
            if point > prev && point <= bytes.len() && s.is_char_boundary(point) {
                chunks.push(s[prev..point].to_string());
                prev = point;
            }
        }
        // 残り
        if prev < s.len() {
            chunks.push(s[prev..].to_string());
        }
        // 空の場合は元文字列全体を1チャンクに
        if chunks.is_empty() && !s.is_empty() {
            chunks.push(s.to_string());
        }
        chunks
    }

    // ========================================
    // Property 2: Think tag extraction across chunk boundaries
    // ========================================
    // **Validates: Requirements 1.4**
    //
    // For any text containing <think>...</think> blocks, splitting the text
    // at arbitrary chunk boundaries and processing each chunk sequentially
    // through the ThinkTagBuffer SHALL produce the same extracted thinking
    // content as processing the entire text at once.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_chunk_split_produces_same_result_as_single_pass(
            input in arb_string_with_think_tags(),
            split_count in 0usize..=10,
        ) {
            let input_len = input.len();

            // Generate split points based on the input length
            let split_points: Vec<usize> = if input_len == 0 {
                vec![]
            } else {
                // Create deterministic split points from split_count
                (0..split_count)
                    .map(|i| (i * input_len) / (split_count + 1).max(1))
                    .filter(|&p| p > 0 && p < input_len && input.is_char_boundary(p))
                    .collect()
            };

            let chunks = split_at_points(&input, &split_points);

            // --- Single pass: process entire input as one chunk ---
            let mut single_buf = ThinkTagBuffer::new();
            let (single_text_parts, single_think_parts) = single_buf.process_chunk(&input);
            let (single_flush_text, single_flush_think) = single_buf.flush();

            let single_text: String = single_text_parts.into_iter()
                .chain(single_flush_text.into_iter())
                .collect::<Vec<_>>()
                .join("");
            let single_thinking: String = single_think_parts.into_iter()
                .chain(single_flush_think.into_iter())
                .collect::<Vec<_>>()
                .join("");

            // --- Multi pass: process each chunk sequentially ---
            let mut multi_buf = ThinkTagBuffer::new();
            let mut multi_text_parts: Vec<String> = Vec::new();
            let mut multi_think_parts: Vec<String> = Vec::new();

            for chunk in &chunks {
                let (text_parts, think_parts) = multi_buf.process_chunk(chunk);
                multi_text_parts.extend(text_parts);
                multi_think_parts.extend(think_parts);
            }
            let (flush_text, flush_think) = multi_buf.flush();
            multi_text_parts.extend(flush_text);
            multi_think_parts.extend(flush_think);

            let multi_text: String = multi_text_parts.join("");
            let multi_thinking: String = multi_think_parts.join("");

            // --- Assert both approaches produce the same result ---
            prop_assert_eq!(
                &multi_text, &single_text,
                "Text content mismatch.\nInput: {:?}\nChunks: {:?}\nSingle text: {:?}\nMulti text: {:?}",
                input, chunks, single_text, multi_text
            );
            prop_assert_eq!(
                &multi_thinking, &single_thinking,
                "Thinking content mismatch.\nInput: {:?}\nChunks: {:?}\nSingle thinking: {:?}\nMulti thinking: {:?}",
                input, chunks, single_thinking, multi_thinking
            );
        }
    }
}

// Feature: thinking-reasoning-support, Property 1: Thinking content separation from text content
#[cfg(test)]
mod thinking_content_separation_tests {
    use proptest::prelude::*;

    use crate::llm::think_tag_buffer::ThinkTagBuffer;

    // ========================================
    // Arbitrary Strategies
    // ========================================

    /// テキストセグメント（<think>タグを含まない任意テキスト）
    fn arb_text_segment() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ぁ-んァ-ヶ!?.,]{1,80}"
    }

    /// thinkingセグメント（<think>タグを含まない任意テキスト）
    fn arb_thinking_segment() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ぁ-んァ-ヶ!?.,]{1,80}"
    }

    /// interleaved text/thinking segments を生成する戦略
    /// (全体文字列, 期待されるtext連結, 期待されるthinking連結) を返す
    fn arb_interleaved_content() -> impl Strategy<Value = (String, String, String)> {
        // 1〜4個のtext/thinkingペアを生成
        (
            proptest::collection::vec(arb_text_segment(), 1..=4),
            proptest::collection::vec(arb_thinking_segment(), 1..=3),
        )
            .prop_map(|(texts, thinkings)| {
                let mut full_string = String::new();
                let mut expected_text = String::new();
                let mut expected_thinking = String::new();

                // interleave: text0 <think>thinking0</think> text1 <think>thinking1</think> ...
                for i in 0..texts.len().max(thinkings.len()) {
                    if i < texts.len() {
                        full_string.push_str(&texts[i]);
                        expected_text.push_str(&texts[i]);
                    }
                    if i < thinkings.len() {
                        full_string.push_str("<think>");
                        full_string.push_str(&thinkings[i]);
                        full_string.push_str("</think>");
                        expected_thinking.push_str(&thinkings[i]);
                    }
                }

                (full_string, expected_text, expected_thinking)
            })
    }

    /// 文字列をランダムな位置でチャンクに分割
    fn split_into_chunks(s: &str, split_positions: &[usize]) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut prev = 0;

        for &pos in split_positions {
            if pos > prev && pos < s.len() && s.is_char_boundary(pos) {
                chunks.push(s[prev..pos].to_string());
                prev = pos;
            }
        }
        if prev < s.len() {
            chunks.push(s[prev..].to_string());
        }
        if chunks.is_empty() && !s.is_empty() {
            chunks.push(s.to_string());
        }
        chunks
    }

    // ========================================
    // Property 1: Thinking content separation from text content
    // ========================================
    // **Validates: Requirements 1.1, 1.2, 1.3, 1.5**
    //
    // For any LLM provider stream containing both thinking and text content,
    // the thinking content SHALL be accumulated exclusively in the thinking
    // buffer and SHALL NOT appear in the text callback output, and vice versa.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// メインプロパティ: thinking contentとtext contentが相互に混入しないこと
        /// ランダムなチャンク分割でThinkTagBufferに処理させ、
        /// textパーツの連結が期待テキストと一致し、thinkingパーツの連結が期待thinkingと一致することを検証
        #[test]
        fn prop_thinking_text_separation_no_cross_contamination(
            (full_string, expected_text, expected_thinking) in arb_interleaved_content(),
            split_count in 1usize..=12,
        ) {
            let input_len = full_string.len();

            // 分割ポイントを生成
            let split_positions: Vec<usize> = if input_len == 0 {
                vec![]
            } else {
                (0..split_count)
                    .map(|i| ((i + 1) * input_len) / (split_count + 1))
                    .filter(|&p| p > 0 && p < input_len && full_string.is_char_boundary(p))
                    .collect()
            };

            let chunks = split_into_chunks(&full_string, &split_positions);

            // ThinkTagBufferで各チャンクを順次処理
            let mut buf = ThinkTagBuffer::new();
            let mut all_text_parts: Vec<String> = Vec::new();
            let mut all_thinking_parts: Vec<String> = Vec::new();

            for chunk in &chunks {
                let (text_parts, thinking_parts) = buf.process_chunk(chunk);
                all_text_parts.extend(text_parts);
                all_thinking_parts.extend(thinking_parts);
            }
            let (flush_text, flush_thinking) = buf.flush();
            all_text_parts.extend(flush_text);
            all_thinking_parts.extend(flush_thinking);

            let actual_text: String = all_text_parts.join("");
            let actual_thinking: String = all_thinking_parts.join("");

            // Property assertion 1: text出力は期待されるtext内容と一致
            // （thinkingの内容がtextに混入していないことの証明）
            prop_assert_eq!(
                &actual_text, &expected_text,
                "Text output mismatch - thinking content leaked into text.\nInput: {:?}\nChunks: {:?}\nExpected text: {:?}\nActual text: {:?}",
                full_string, chunks, expected_text, actual_text
            );

            // Property assertion 2: thinking出力は期待されるthinking内容と一致
            // （textの内容がthinkingに混入していないことの証明）
            prop_assert_eq!(
                &actual_thinking, &expected_thinking,
                "Thinking output mismatch - text content leaked into thinking.\nInput: {:?}\nChunks: {:?}\nExpected thinking: {:?}\nActual thinking: {:?}",
                full_string, chunks, expected_thinking, actual_thinking
            );
        }

        /// 追加検証: thinking部分の各パーツが、元のtext segments内に含まれないこと
        #[test]
        fn prop_thinking_parts_not_in_text_segments(
            (full_string, expected_text, _expected_thinking) in arb_interleaved_content(),
            split_count in 1usize..=8,
        ) {
            let input_len = full_string.len();
            let split_positions: Vec<usize> = if input_len == 0 {
                vec![]
            } else {
                (0..split_count)
                    .map(|i| ((i + 1) * input_len) / (split_count + 1))
                    .filter(|&p| p > 0 && p < input_len && full_string.is_char_boundary(p))
                    .collect()
            };

            let chunks = split_into_chunks(&full_string, &split_positions);

            let mut buf = ThinkTagBuffer::new();
            let mut all_text_parts: Vec<String> = Vec::new();
            let mut all_thinking_parts: Vec<String> = Vec::new();

            for chunk in &chunks {
                let (text_parts, thinking_parts) = buf.process_chunk(chunk);
                all_text_parts.extend(text_parts);
                all_thinking_parts.extend(thinking_parts);
            }
            let (flush_text, flush_thinking) = buf.flush();
            all_text_parts.extend(flush_text);
            all_thinking_parts.extend(flush_thinking);

            // 各thinking partが空でない場合、textの連結結果に含まれていないことを確認
            let text_concat: String = all_text_parts.join("");
            for thinking_part in &all_thinking_parts {
                if !thinking_part.is_empty() && thinking_part.len() > 2 {
                    prop_assert!(
                        !text_concat.contains(thinking_part.as_str()),
                        "Thinking part {:?} found in text output {:?}",
                        thinking_part, text_concat
                    );
                }
            }

            // text連結が期待通りであることも再確認
            prop_assert_eq!(&text_concat, &expected_text);
        }
    }
}

// Feature: app-enhancements-v3, Property 2: プロバイダー×エンドポイントによるAPI形式決定
#[cfg(test)]
mod api_strategy_tests {
    use proptest::prelude::*;

    use crate::llm::client::{
        is_default_endpoint, resolve_api_strategy, ApiStrategy, LLMClientConfig,
    };
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
    fn arb_valid_anthropic_response() -> impl Strategy<Value = (serde_json::Value, String)> {
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

        /// Sub-property 1: 有効なGeminiレスポンスJSONに対し、parse_gemini_responseはOk(LLMResponse::Text { content: text, thinking: None })を返し、textが入力と一致する
        #[test]
        fn prop_parse_gemini_response_valid(
            (body, expected_text) in arb_valid_gemini_response(),
        ) {
            let result = parse_gemini_response(&body);
            prop_assert!(result.is_ok(), "parse_gemini_response should return Ok for valid input");
            match result.unwrap() {
                LLMResponse::Text { content: text, thinking: None } => {
                    prop_assert_eq!(text, expected_text,
                        "parsed text should match input text");
                }
                other => {
                    prop_assert!(false, "Expected LLMResponse::Text, got {:?}", other);
                }
            }
        }

        /// Sub-property 2: 有効なAnthropicレスポンスJSONに対し、parse_anthropic_responseはOk(LLMResponse::Text { content: text, thinking: None })を返し、textが全テキストブロックの結合と一致する
        #[test]
        fn prop_parse_anthropic_response_valid(
            (body, expected_text) in arb_valid_anthropic_response(),
        ) {
            let result = parse_anthropic_response(&body);
            prop_assert!(result.is_ok(), "parse_anthropic_response should return Ok for valid input");
            match result.unwrap() {
                LLMResponse::Text { content: text, thinking: None } => {
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

// Feature: thinking-reasoning-support, Property 7: Thinking block type and order preservation
#[cfg(test)]
mod thinking_block_order_preservation_tests {
    use proptest::prelude::*;

    use crate::llm::client::REDACTED_THINKING_MARKER;

    // ========================================
    // Arbitrary Strategies
    // ========================================

    /// Anthropic thinking ブロックの種別
    #[derive(Debug, Clone)]
    enum ThinkingBlock {
        /// 通常の thinking ブロック（テキスト内容あり）
        Normal(String),
        /// redacted_thinking ブロック（マーカーで置換）
        Redacted,
    }

    /// thinking ブロック（ランダムテキスト）のストラテジー
    fn arb_thinking_block() -> impl Strategy<Value = ThinkingBlock> {
        prop_oneof![
            // 通常 thinking ブロック: 非空テキスト
            "[a-zA-Z0-9 ]{1,100}".prop_map(ThinkingBlock::Normal),
            // redacted_thinking ブロック
            Just(ThinkingBlock::Redacted),
        ]
    }

    /// thinking ブロック列（1〜10個）のストラテジー
    fn arb_thinking_block_sequence() -> impl Strategy<Value = Vec<ThinkingBlock>> {
        proptest::collection::vec(arb_thinking_block(), 1..=10)
    }

    /// ThinkingBlock列から期待されるaccumulated文字列を構築
    fn build_expected_accumulated(blocks: &[ThinkingBlock]) -> String {
        blocks
            .iter()
            .map(|block| match block {
                ThinkingBlock::Normal(text) => text.clone(),
                ThinkingBlock::Redacted => REDACTED_THINKING_MARKER.to_string(),
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// ThinkingBlock列をAnthropic SSEイベント処理をシミュレートして蓄積
    /// (parse_anthropic_responseの内部ロジックと同等の蓄積処理)
    fn simulate_anthropic_accumulation(blocks: &[ThinkingBlock]) -> String {
        let mut thinking_parts: Vec<String> = Vec::new();
        for block in blocks {
            match block {
                ThinkingBlock::Normal(text) => {
                    if !text.is_empty() {
                        thinking_parts.push(text.clone());
                    }
                }
                ThinkingBlock::Redacted => {
                    thinking_parts.push(REDACTED_THINKING_MARKER.to_string());
                }
            }
        }
        thinking_parts.join("")
    }

    /// ThinkingBlock列からAnthropic形式のJSONレスポンスを構築し、parse_anthropic_responseで検証
    fn build_anthropic_json_and_parse(
        blocks: &[ThinkingBlock],
    ) -> Result<Option<String>, crate::error::AppError> {
        use crate::llm::client::parse_anthropic_response;
        use serde_json::json;

        let content: Vec<serde_json::Value> = blocks
            .iter()
            .map(|block| match block {
                ThinkingBlock::Normal(text) => json!({
                    "type": "thinking",
                    "thinking": text,
                }),
                ThinkingBlock::Redacted => json!({
                    "type": "redacted_thinking",
                    "data": "base64encodeddata"
                }),
            })
            .collect();

        // text ブロックも1つ追加（Anthropicレスポンスには通常テキストが含まれる）
        let mut full_content = content;
        full_content.push(json!({
            "type": "text",
            "text": "response text"
        }));

        let body = json!({ "content": full_content });
        let response = parse_anthropic_response(&body)?;

        match response {
            crate::llm::client::LLMResponse::Text { thinking, .. } => Ok(thinking),
            _ => Ok(None),
        }
    }

    // ========================================
    // Property 7: Thinking block type and order preservation
    // ========================================
    // **Validates: Requirements 6.1, 6.4**
    //
    // For any Anthropic response containing a sequence of `thinking` and
    // `redacted_thinking` blocks, the accumulated thinking content SHALL
    // preserve the type annotation (normal vs redacted marker) and the
    // original ordering of all blocks.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// 蓄積ロジックが型と順序を保持することを検証（シミュレーション）
        #[test]
        fn prop_thinking_block_accumulation_preserves_order(
            blocks in arb_thinking_block_sequence(),
        ) {
            let expected = build_expected_accumulated(&blocks);
            let actual = simulate_anthropic_accumulation(&blocks);

            prop_assert_eq!(
                &actual, &expected,
                "Accumulation should preserve type and order.\nBlocks: {:?}\nExpected: {:?}\nActual: {:?}",
                blocks, expected, actual
            );
        }

        /// parse_anthropic_response による実際のパースが型と順序を保持することを検証
        #[test]
        fn prop_parse_anthropic_response_preserves_thinking_block_order(
            blocks in arb_thinking_block_sequence(),
        ) {
            let expected = build_expected_accumulated(&blocks);
            let result = build_anthropic_json_and_parse(&blocks);

            prop_assert!(result.is_ok(), "parse_anthropic_response should not fail: {:?}", result);

            let thinking = result.unwrap();
            prop_assert!(thinking.is_some(),
                "thinking should be Some when blocks are present.\nBlocks: {:?}",
                blocks
            );

            let actual = thinking.unwrap();
            prop_assert_eq!(
                &actual, &expected,
                "parse_anthropic_response should preserve type annotation and order.\nBlocks: {:?}\nExpected: {:?}\nActual: {:?}",
                blocks, expected, actual
            );
        }

        /// redacted_thinking ブロックが REDACTED_THINKING_MARKER に置換されることを検証
        #[test]
        fn prop_redacted_blocks_produce_marker(
            normal_texts in proptest::collection::vec("[a-zA-Z0-9 ]{1,50}", 0..=3),
            redacted_positions in proptest::collection::vec(proptest::bool::ANY, 1..=5),
        ) {
            // redacted_positions に基づいてブロック列を構築
            let blocks: Vec<ThinkingBlock> = redacted_positions.iter().enumerate().map(|(i, &is_redacted)| {
                if is_redacted {
                    ThinkingBlock::Redacted
                } else {
                    let text = normal_texts.get(i % normal_texts.len().max(1))
                        .cloned()
                        .unwrap_or_else(|| "fallback".to_string());
                    ThinkingBlock::Normal(text)
                }
            }).collect();

            let result = build_anthropic_json_and_parse(&blocks);
            prop_assert!(result.is_ok());

            let thinking = result.unwrap().unwrap_or_default();

            // redacted ブロックの数だけマーカーが含まれる
            let expected_marker_count = blocks.iter().filter(|b| matches!(b, ThinkingBlock::Redacted)).count();
            let actual_marker_count = thinking.matches(REDACTED_THINKING_MARKER).count();

            prop_assert_eq!(
                actual_marker_count, expected_marker_count,
                "Number of REDACTED_THINKING_MARKER occurrences should match redacted block count.\nBlocks: {:?}\nThinking: {:?}",
                blocks, thinking
            );
        }
    }
}
