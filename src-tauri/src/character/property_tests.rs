//! キャラクター機能のプロパティテスト
//! proptest を使用してCharacter Creator の不変条件を検証する。

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use async_trait::async_trait;
    use proptest::prelude::*;
    use tokio::sync::Mutex;

    use crate::character::creator::{CharacterCreator, DefaultCharacterCreator};
    use crate::db::database::Database;
    use crate::error::AppError;
    use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse};
    use crate::models::{Character, ToolDefinition};

    // ========================================
    // テスト用MockLLMClient
    // ========================================

    struct MockLLMClient;

    #[async_trait]
    impl LLMClient for MockLLMClient {
        async fn chat(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _tools: Option<&[ToolDefinition]>,
        ) -> Result<LLMResponse, AppError> {
            Ok(LLMResponse::Text("mock response".to_string()))
        }

        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _config: &LLMClientConfig,
            _callback: Box<dyn Fn(String) + Send>,
        ) -> Result<String, AppError> {
            Ok("mock stream".to_string())
        }

        async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
            Ok(())
        }
    }

    // ========================================
    // ヘルパー
    // ========================================

    fn test_llm_config() -> Arc<crate::config::model_config::ModelConfigManager> {
        use std::collections::HashMap;
        use crate::models::config::*;

        let mut models = HashMap::new();
        let settings = ModelSettings {
            base_url: "http://localhost:8080/v1".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            temperature: 0.7,
        };
        models.insert(ModelPurpose::Chat, settings.clone());
        models.insert(ModelPurpose::Memory, settings.clone());
        models.insert(ModelPurpose::Thought, settings.clone());
        models.insert(ModelPurpose::CharacterGeneration, settings);

        let config = AppConfig {
            models,
            spontaneous: SpontaneousConfig { enabled: false, min_interval_seconds: 60, probability: 0.3 },
            thought: ThoughtConfig { enabled: false, interval_minutes: 5, auto_delete_threshold_minutes: 1440 },
            memory: MemoryConfig { compression_threshold: 50 },
            tts: TTSGlobalConfig { enabled: false },
            ui: UIConfig { theme: Theme::Dark, language: "ja".to_string() },
            plugins: PluginsConfig { enabled_plugins: vec![], plugin_settings: HashMap::new() },
            attachment: AttachmentConfig { max_file_size_bytes: 10 * 1024 * 1024, allowed_extensions: vec![] },
        };

        Arc::new(crate::config::model_config::ModelConfigManager::new_with_config(config))
    }

    /// 非空文字列を生成するストラテジー
    fn non_empty_string() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9ぁ-んァ-ヶ]{1,30}".prop_map(|s| s)
    }

    /// ISO 8601日時文字列を生成するストラテジー
    fn iso8601_datetime() -> impl Strategy<Value = String> {
        (2020u32..2030, 1u32..13, 1u32..29, 0u32..24, 0u32..60, 0u32..60).prop_map(
            |(y, m, d, h, min, s)| {
                format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, min, s)
            },
        )
    }

    /// ユニークなUUIDを生成するストラテジー
    fn uuid_string() -> impl Strategy<Value = String> {
        "[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}"
    }

    /// Character のストラテジー
    fn arb_character() -> impl Strategy<Value = Character> {
        (
            uuid_string(),
            non_empty_string(),
            non_empty_string(),
            non_empty_string(),
            proptest::option::of(non_empty_string()),
            iso8601_datetime(),
            iso8601_datetime(),
        )
            .prop_map(
                |(id, name, description, system_prompt, avatar_path, created_at, updated_at)| {
                    Character {
                        id,
                        name,
                        description,
                        system_prompt,
                        avatar_path,
                        tts_config: None,
                        created_at,
                        updated_at,
                    }
                },
            )
    }

    /// ユニークIDを持つキャラクターセットを生成するストラテジー
    fn arb_character_set(max_size: usize) -> impl Strategy<Value = Vec<Character>> {
        proptest::collection::vec(arb_character(), 1..=max_size).prop_map(|mut chars| {
            // IDの重複を排除（ユニークなIDを保証）
            let mut seen_ids = HashSet::new();
            chars.retain(|c| seen_ids.insert(c.id.clone()));
            chars
        })
    }

    // ========================================
    // Property 2: Character listing completeness
    // ========================================
    // **Validates: Requirements 1.4**
    //
    // For any set of created Characters, calling list_characters SHALL return
    // exactly those characters — no more, no fewer.

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_character_listing_completeness(
            characters in arb_character_set(8),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
                let llm_client: Arc<dyn LLMClient> = Arc::new(MockLLMClient);
                let config = test_llm_config();
                let creator = DefaultCharacterCreator::new(db, llm_client, config);

                // 全キャラクターを保存
                let mut saved_ids: HashSet<String> = HashSet::new();
                for character in &characters {
                    creator.save_character(character).await.unwrap();
                    saved_ids.insert(character.id.clone());
                }

                // list_characters呼び出し
                let listed = creator.list_characters().await.unwrap();

                // 件数一致
                prop_assert_eq!(
                    listed.len(),
                    characters.len(),
                    "Character count mismatch: expected {}, got {}",
                    characters.len(),
                    listed.len()
                );

                // 全IDが含まれている（no fewer）
                let listed_ids: HashSet<String> = listed.iter().map(|c| c.id.clone()).collect();
                for expected_id in &saved_ids {
                    prop_assert!(
                        listed_ids.contains(expected_id),
                        "Character {} not found in listing",
                        expected_id
                    );
                }

                // 余分なIDが含まれていない（no more）
                for listed_id in &listed_ids {
                    prop_assert!(
                        saved_ids.contains(listed_id),
                        "Unexpected character {} found in listing",
                        listed_id
                    );
                }

                Ok(())
            })?;
        }

        #[test]
        fn prop_character_listing_after_deletion(
            characters in arb_character_set(6),
            delete_ratio in 0.1f64..0.9,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
                let llm_client: Arc<dyn LLMClient> = Arc::new(MockLLMClient);
                let config = test_llm_config();
                let creator = DefaultCharacterCreator::new(db, llm_client, config);

                // 全キャラクターを保存
                for character in &characters {
                    creator.save_character(character).await.unwrap();
                }

                // 一部を削除
                let delete_count = ((characters.len() as f64) * delete_ratio).ceil() as usize;
                let delete_count = delete_count.min(characters.len() - 1).max(1);
                let to_delete: Vec<String> = characters.iter()
                    .take(delete_count)
                    .map(|c| c.id.clone())
                    .collect();

                for id in &to_delete {
                    creator.delete_character(id).await.unwrap();
                }

                // list_characters呼び出し
                let listed = creator.list_characters().await.unwrap();

                // 残存キャラクターのID集合
                let expected_remaining: HashSet<String> = characters.iter()
                    .filter(|c| !to_delete.contains(&c.id))
                    .map(|c| c.id.clone())
                    .collect();

                // 件数一致
                prop_assert_eq!(
                    listed.len(),
                    expected_remaining.len(),
                    "After deletion: expected {} characters, got {}",
                    expected_remaining.len(),
                    listed.len()
                );

                // 残存IDが全て含まれている
                let listed_ids: HashSet<String> = listed.iter().map(|c| c.id.clone()).collect();
                for expected_id in &expected_remaining {
                    prop_assert!(
                        listed_ids.contains(expected_id),
                        "Remaining character {} not found after deletion",
                        expected_id
                    );
                }

                // 削除済みIDが含まれていない
                for deleted_id in &to_delete {
                    prop_assert!(
                        !listed_ids.contains(deleted_id),
                        "Deleted character {} still found in listing",
                        deleted_id
                    );
                }

                Ok(())
            })?;
        }
    }
}
