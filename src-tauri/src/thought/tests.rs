// Thought Engine tests

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::db::database::Database;
use crate::error::AppError;
use crate::llm::client::{ChatMessage, LLMClient, LLMClientConfig, LLMResponse, MessageRole};
use crate::models::{ChatMessageRecord, ChatRole, Memory, Thought, ToolDefinition};
use crate::thought::engine::{DefaultThoughtEngine, ThoughtEngine};

/// テスト用MockLLMClient
struct MockLLMClient {
    response: Mutex<String>,
}

impl MockLLMClient {
    fn new(response: &str) -> Self {
        Self {
            response: Mutex::new(response.to_string()),
        }
    }
}

#[async_trait]
impl LLMClient for MockLLMClient {
    async fn chat(
        &self,
        _messages: &[ChatMessage],
        _config: &LLMClientConfig,
        _tools: Option<&[ToolDefinition]>,
    ) -> Result<LLMResponse, AppError> {
        let resp = self.response.lock().unwrap().clone();
        Ok(LLMResponse::Text(resp))
    }

    async fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        _config: &LLMClientConfig,
        _callback: Box<dyn Fn(String) + Send>,
    ) -> Result<String, AppError> {
        let resp = self.response.lock().unwrap().clone();
        Ok(resp)
    }

    async fn test_connection(&self, _config: &LLMClientConfig) -> Result<(), AppError> {
        Ok(())
    }
}

fn setup_db() -> Database {
    let db = Database::open_in_memory().unwrap();
    let conn = db.connection();

    conn.execute(
        "INSERT INTO characters (id, name, description, system_prompt, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            "char-001",
            "テストキャラ",
            "テスト用",
            "あなたは猫のキャラクターです。",
            "2024-01-01T00:00:00Z",
            "2024-01-01T00:00:00Z"
        ],
    )
    .unwrap();

    db
}

fn setup_db_with_session(db: &Database) {
    let conn = db.connection();

    conn.execute(
        "INSERT INTO chat_sessions (id, character_id, title, last_message_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            "sess-001",
            "char-001",
            "テストセッション",
            "2024-01-01T12:00:00Z",
            "2024-01-01T00:00:00Z"
        ],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO chat_messages (id, session_id, role, content, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            "msg-001",
            "sess-001",
            "user",
            "今日の天気はどう？",
            "2024-01-01T10:00:00Z"
        ],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO chat_messages (id, session_id, role, content, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            "msg-002",
            "sess-001",
            "assistant",
            "にゃー、今日は晴れだよ！",
            "2024-01-01T10:01:00Z"
        ],
    )
    .unwrap();
}

fn setup_db_with_memories(db: &Database) {
    let conn = db.connection();

    conn.execute(
        "INSERT INTO memories (id, character_id, content, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            "mem-001",
            "char-001",
            "ユーザーは猫が好き",
            "2024-01-01T00:00:00Z",
            "2024-01-01T00:00:00Z"
        ],
    )
    .unwrap();
}

fn default_llm_config() -> Arc<crate::config::model_config::ModelConfigManager> {
    use std::collections::HashMap;
    use crate::models::config::*;

    let mut models = HashMap::new();
    let settings = ModelSettings {
        base_url: "http://localhost:8080/v1".to_string(),
        model: "test-model".to_string(),
        api_key: None,
        temperature: 0.7,
        provider: None,
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
        tts: TTSGlobalConfig { enabled: false, voicepeak_path: None, timeout_seconds: 60, max_chunk_size: 140, irodori_base_url: None, irodori_caption_base_url: None, irodori_reference_audio_base_url: None },
        ui: UIConfig { theme: Theme::Dark, language: "ja".to_string(), send_key: SendKey::default() },
        plugins: PluginsConfig { enabled_plugins: vec![], plugin_settings: HashMap::new() },
        attachment: AttachmentConfig { max_file_size_bytes: 10 * 1024 * 1024, allowed_extensions: vec![] },
    };

    Arc::new(crate::config::model_config::ModelConfigManager::new_with_config(config))
}

/// テスト用llm_lockを生成
fn test_llm_lock() -> Arc<tokio::sync::Mutex<()>> {
    Arc::new(tokio::sync::Mutex::new(()))
}

#[tokio::test]
async fn test_generate_thought_basic() {
    let db = setup_db();
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("今日は穏やかな一日だにゃ"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    let thought = engine.generate_thought("char-001").await.unwrap();

    assert_eq!(thought.character_id, "char-001");
    assert_eq!(thought.content, "今日は穏やかな一日だにゃ");
    assert!(!thought.id.is_empty());
    assert!(!thought.created_at.is_empty());
}

#[tokio::test]
async fn test_generate_thought_with_context() {
    let db = setup_db();
    setup_db_with_session(&db);
    setup_db_with_memories(&db);
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("天気の話をしたな…外に出たいにゃ"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    let thought = engine.generate_thought("char-001").await.unwrap();

    assert_eq!(thought.content, "天気の話をしたな…外に出たいにゃ");
    // コンテキスト概要が設定されている
    let ctx = thought.context.unwrap();
    assert!(ctx.contains("直近会話"));
    assert!(ctx.contains("記憶"));
}

#[tokio::test]
async fn test_generate_thought_saved_to_db() {
    let db = setup_db();
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("保存テスト思考"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    let thought = engine.generate_thought("char-001").await.unwrap();

    // DBから取得して確認
    let thoughts = engine.get_thoughts("char-001", None).await.unwrap();
    assert_eq!(thoughts.len(), 1);
    assert_eq!(thoughts[0].id, thought.id);
    assert_eq!(thoughts[0].content, "保存テスト思考");
}

#[tokio::test]
async fn test_generate_thought_character_not_found() {
    let db = setup_db();
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("should not reach"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    let result = engine.generate_thought("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_generate_thought_empty_response() {
    let db = setup_db();
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("   "));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    let result = engine.generate_thought("char-001").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_thoughts_with_limit() {
    let db = setup_db();
    {
        let conn = db.connection();
        for i in 0..5 {
            conn.execute(
                "INSERT INTO thoughts (id, character_id, content, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    format!("thought-{:03}", i),
                    "char-001",
                    format!("思考{}", i),
                    format!("2024-01-01T{:02}:00:00Z", 10 + i),
                ],
            )
            .unwrap();
        }
    }
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("unused"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    let thoughts = engine.get_thoughts("char-001", Some(3)).await.unwrap();
    assert_eq!(thoughts.len(), 3);
    // DESC順で最新から
    assert_eq!(thoughts[0].id, "thought-004");
}

#[tokio::test]
async fn test_get_thoughts_empty() {
    let db = setup_db();
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("unused"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    let thoughts = engine.get_thoughts("char-001", None).await.unwrap();
    assert!(thoughts.is_empty());
}

#[test]
fn test_set_frequency() {
    let db = setup_db();
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("unused"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    engine.set_frequency("char-001", 10);

    let state = engine.state.lock().unwrap();
    assert_eq!(state.interval_minutes, 10);
    assert_eq!(state.character_id, Some("char-001".to_string()));
}

#[test]
fn test_stop_sets_running_false() {
    let db = setup_db();
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("unused"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    engine.running.store(true, std::sync::atomic::Ordering::SeqCst);
    engine.stop();

    assert!(!engine.running.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn test_build_thought_prompt_empty_context() {
    let messages: Vec<ChatMessageRecord> = vec![];
    let memories: Vec<Memory> = vec![];

    let prompt = DefaultThoughtEngine::build_thought_prompt(
        "You are a cat.",
        &messages,
        &memories,
    );

    // system + meta-prompt = 2メッセージ
    assert_eq!(prompt.len(), 2);
    assert_eq!(prompt[0].role, MessageRole::System);
    assert_eq!(prompt[0].content, "You are a cat.");
    assert_eq!(prompt[1].role, MessageRole::User);
    assert!(prompt[1].content.contains("internal thought"));
}

#[test]
fn test_build_thought_prompt_with_messages_and_memories() {
    let messages = vec![
        ChatMessageRecord {
            id: "msg-001".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::User,
            content: "こんにちは".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:00:00Z".to_string(),
        },
        ChatMessageRecord {
            id: "msg-002".to_string(),
            session_id: "sess-001".to_string(),
            role: ChatRole::Assistant,
            content: "にゃー".to_string(),
            attachments: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: "2024-01-01T10:01:00Z".to_string(),
        },
    ];

    let memories = vec![Memory {
        id: "mem-001".to_string(),
        character_id: "char-001".to_string(),
        content: "ユーザーは猫が好き".to_string(),
        source_session_id: None,
        source_message_from: None,
        source_message_to: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    }];

    let prompt = DefaultThoughtEngine::build_thought_prompt(
        "You are a cat.",
        &messages,
        &memories,
    );

    // system + memory_system + 2 conversation + meta-prompt = 5
    assert_eq!(prompt.len(), 5);
    assert_eq!(prompt[0].role, MessageRole::System);
    assert_eq!(prompt[0].content, "You are a cat.");
    // 記憶
    assert_eq!(prompt[1].role, MessageRole::System);
    assert!(prompt[1].content.contains("ユーザーは猫が好き"));
    // 会話
    assert_eq!(prompt[2].role, MessageRole::User);
    assert_eq!(prompt[2].content, "こんにちは");
    assert_eq!(prompt[3].role, MessageRole::Assistant);
    assert_eq!(prompt[3].content, "にゃー");
    // メタプロンプト
    assert_eq!(prompt[4].role, MessageRole::User);
    assert!(prompt[4].content.contains("internal thought"));
}

#[test]
fn test_build_thought_prompt_spontaneous_role_mapped_to_assistant() {
    let messages = vec![ChatMessageRecord {
        id: "msg-001".to_string(),
        session_id: "sess-001".to_string(),
        role: ChatRole::Spontaneous,
        content: "自発的発話".to_string(),
        attachments: None,
        tool_calls: None,
        tool_call_id: None,
        created_at: "2024-01-01T10:00:00Z".to_string(),
    }];

    let prompt = DefaultThoughtEngine::build_thought_prompt(
        "System prompt",
        &messages,
        &[],
    );

    // Spontaneous roleはAssistantにマッピング
    assert_eq!(prompt[1].role, MessageRole::Assistant);
}

#[test]
fn test_thought_event_serialization() {
    use crate::thought::engine::ThoughtEvent;

    let event = ThoughtEvent {
        character_id: "char-001".to_string(),
        thought: Thought {
            id: "thought-001".to_string(),
            character_id: "char-001".to_string(),
            content: "テスト思考".to_string(),
            context: Some("直近会話2件".to_string()),
            created_at: "2024-01-01T10:00:00Z".to_string(),
        },
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("char-001"));
    assert!(json.contains("テスト思考"));
    assert!(json.contains("thought-001"));
}

#[test]
fn test_pause_resume() {
    let db = setup_db();
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("unused"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    // 初期状態: paused = false
    assert!(!engine.is_paused());

    // pause
    engine.pause();
    assert!(engine.is_paused());

    // resume
    engine.resume();
    assert!(!engine.is_paused());
}

#[tokio::test]
async fn test_delete_thought_removes_from_db() {
    let db = setup_db();
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("削除テスト思考"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    // 思考を生成
    let thought = engine.generate_thought("char-001").await.unwrap();

    // 削除前: 存在する
    let thoughts = engine.get_thoughts("char-001", None).await.unwrap();
    assert_eq!(thoughts.len(), 1);

    // 削除
    engine.delete_thought(&thought.id).await.unwrap();

    // 削除後: 存在しない
    let thoughts = engine.get_thoughts("char-001", None).await.unwrap();
    assert!(thoughts.is_empty());
}

#[tokio::test]
async fn test_delete_thought_not_found() {
    let db = setup_db();
    let db = Arc::new(Mutex::new(db));
    let mock_llm = Arc::new(MockLLMClient::new("unused"));
    let config = default_llm_config();

    let engine = DefaultThoughtEngine::new(db.clone(), mock_llm, config, test_llm_lock());

    // 存在しないIDで削除 → NotFoundエラー
    let result = engine.delete_thought("nonexistent-id").await;
    assert!(result.is_err());
}
