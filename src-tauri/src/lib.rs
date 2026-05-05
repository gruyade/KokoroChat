pub mod attachment;
pub mod character;
pub mod chat;
pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod llm;
pub mod memory;
pub mod models;
pub mod plugin;
pub mod spontaneous;
pub mod state;
pub mod thought;
pub mod tts;

pub use error::AppError;

use std::sync::Arc;

use attachment::processor::DefaultAttachmentProcessor;
use character::creator::DefaultCharacterCreator;
use chat::engine::DefaultChatEngine;
use config::model_config::ModelConfigManager;
use db::database::Database;
use llm::client::{LLMClientConfig, OpenAICompatibleClient};
use memory::manager::DefaultMemoryManager;
use models::config::ModelPurpose;
use plugin::builtin::{CalculatorPlugin, FileOpsPlugin, WebSearchPlugin};
use plugin::registry::{DefaultPluginRegistry, PluginRegistry};
use state::AppState;
use tts::connector::DefaultTTSConnector;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;

            // アプリデータディレクトリ取得
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");

            // Database初期化
            let db_path = app_data_dir.join("data.sqlite");
            let db = Database::open(&db_path).expect("Failed to open database");

            // CharacterCreator用（tokio::sync::Mutex）
            let db_for_character =
                Arc::new(tokio::sync::Mutex::new(db));

            // ChatEngine/MemoryManager用に別DBインスタンスを作成（std::sync::Mutex）
            let db_for_chat = Arc::new(std::sync::Mutex::new(
                Database::open(&db_path).expect("Failed to open database for chat"),
            ));
            let db_for_memory = Arc::new(std::sync::Mutex::new(
                Database::open(&db_path).expect("Failed to open database for memory"),
            ));

            // LLMクライアント初期化
            let llm_client: Arc<dyn llm::client::LLMClient> =
                Arc::new(OpenAICompatibleClient::new());

            // 設定ロード
            let config_path = app_data_dir.join("config.json");
            let config_manager = Arc::new(
                ModelConfigManager::new(config_path).expect("Failed to load config"),
            );

            // キャラクター生成用LLM設定
            let chargen_llm_config = config_manager
                .get_model_settings(&ModelPurpose::CharacterGeneration)
                .map(|s| LLMClientConfig {
                    base_url: s.base_url,
                    model: s.model,
                    api_key: s.api_key,
                    temperature: s.temperature,
                })
                .unwrap_or_else(|| LLMClientConfig {
                    base_url: String::new(),
                    model: String::new(),
                    api_key: None,
                    temperature: 0.7,
                });

            // チャット/メモリ/思考用LLM設定
            let chat_llm_config = config_manager
                .get_model_settings(&ModelPurpose::Chat)
                .map(|s| LLMClientConfig {
                    base_url: s.base_url,
                    model: s.model,
                    api_key: s.api_key,
                    temperature: s.temperature,
                })
                .unwrap_or_else(|| LLMClientConfig {
                    base_url: String::new(),
                    model: String::new(),
                    api_key: None,
                    temperature: 0.7,
                });

            // コンポーネント初期化
            let character_creator: Arc<dyn character::creator::CharacterCreator> =
                Arc::new(DefaultCharacterCreator::new(
                    db_for_character,
                    llm_client.clone(),
                    chargen_llm_config,
                ));

            let chat_engine: Arc<dyn chat::engine::ChatEngine> =
                Arc::new(DefaultChatEngine::new(
                    db_for_chat.clone(),
                    llm_client.clone(),
                    chat_llm_config.clone(),
                ));

            let memory_manager: Arc<dyn memory::manager::MemoryManager> =
                Arc::new(DefaultMemoryManager::new(
                    db_for_memory,
                    llm_client.clone(),
                    chat_llm_config,
                    config_manager
                        .get_config()
                        .memory
                        .compression_threshold,
                ));

            let tts_connector: Arc<dyn tts::connector::TTSConnector> =
                Arc::new(DefaultTTSConnector::new());

            let attachment_processor: Arc<dyn attachment::processor::AttachmentProcessor> =
                Arc::new(DefaultAttachmentProcessor::new());

            // プラグインレジストリ初期化・組み込みプラグイン登録
            let plugin_registry = Arc::new(DefaultPluginRegistry::new());
            plugin_registry
                .register(Box::new(CalculatorPlugin::new()))
                .ok();
            plugin_registry
                .register(Box::new(WebSearchPlugin::new()))
                .ok();
            plugin_registry
                .register(Box::new(FileOpsPlugin::new(
                    app_data_dir.join("plugin_files"),
                )))
                .ok();

            // AppState構築
            let app_state = AppState {
                character_creator,
                chat_engine,
                memory_manager,
                tts_connector,
                config_manager,
                llm_client,
                attachment_processor,
                plugin_registry,
            };

            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::character::create_character,
            commands::character::list_characters,
            commands::character::get_character,
            commands::character::update_character,
            commands::character::delete_character,
            commands::chat::create_session,
            commands::chat::send_message,
            commands::chat::get_history,
            commands::chat::list_sessions,
            commands::chat::delete_session,
            commands::config::get_config,
            commands::config::set_config,
            commands::config::test_llm_connection,
            commands::memory::list_memories,
            commands::memory::update_memory,
            commands::memory::delete_memory,
            commands::tts::synthesize_speech,
            commands::tts::test_tts_connection,
            commands::attachment::process_attachment,
            commands::attachment::get_supported_extensions,
            commands::plugin::list_plugins,
            commands::plugin::enable_plugin,
            commands::plugin::disable_plugin,
            commands::plugin::get_plugin_config,
            commands::plugin::set_plugin_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
