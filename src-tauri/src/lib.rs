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
use chat::abort::StreamAbortManager;
use chat::engine::DefaultChatEngine;
use config::model_config::ModelConfigManager;
use db::database::Database;
use llm::client::OpenAICompatibleClient;
use memory::manager::DefaultMemoryManager;
use plugin::builtin::{CalculatorPlugin, FileOpsPlugin, WebSearchPlugin};
use plugin::registry::{DefaultPluginRegistry, PluginRegistry};
use state::AppState;
use thought::engine::DefaultThoughtEngine;
use tts::connector::DefaultTTSConnector;
use tts::flow_controller::TTSFlowController;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;

            // .envファイルから環境変数をロード（存在しなくてもエラーにしない）
            // cargo tauri dev時はsrc-tauri/から実行されるため、親ディレクトリも探索
            dotenvy::dotenv().ok();
            dotenvy::from_filename("../.env").ok();

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

            // コンポーネント初期化
            let llm_lock = Arc::new(tokio::sync::Mutex::new(()));

            let character_creator: Arc<dyn character::creator::CharacterCreator> =
                Arc::new(DefaultCharacterCreator::new(
                    db_for_character,
                    llm_client.clone(),
                    config_manager.clone(),
                ));

            let tts_connector: Arc<dyn tts::connector::TTSConnector> =
                Arc::new(DefaultTTSConnector::new());

            let tts_flow_controller = Arc::new(TTSFlowController::new(
                tts_connector.clone(),
                llm_client.clone(),
                config_manager.clone(),
            ));

            let chat_engine: Arc<dyn chat::engine::ChatEngine> =
                Arc::new(DefaultChatEngine::new(
                    db_for_chat.clone(),
                    llm_client.clone(),
                    config_manager.clone(),
                    llm_lock.clone(),
                    tts_connector.clone(),
                    Some(tts_flow_controller),
                ));

            let memory_manager: Arc<dyn memory::manager::MemoryManager> =
                Arc::new(DefaultMemoryManager::new(
                    db_for_memory,
                    llm_client.clone(),
                    config_manager.clone(),
                    config_manager
                        .get_config()
                        .memory
                        .compression_threshold,
                    llm_lock.clone(),
                ));

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
            let db_for_thought = Arc::new(std::sync::Mutex::new(
                Database::open(&db_path).expect("Failed to open database for thought"),
            ));
            let thought_engine: Arc<dyn thought::engine::ThoughtEngine> =
                Arc::new(DefaultThoughtEngine::new(
                    db_for_thought,
                    llm_client.clone(),
                    config_manager.clone(),
                    llm_lock.clone(),
                ));

            let app_state = AppState {
                character_creator,
                chat_engine,
                memory_manager,
                tts_connector,
                config_manager,
                llm_client,
                attachment_processor,
                plugin_registry,
                thought_engine,
                llm_lock,
                db: db_for_chat.clone(),
                stream_abort_manager: Arc::new(StreamAbortManager::new()),
                spontaneous_paused: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
            commands::character::generate_system_prompt,
            commands::character::improve_system_prompt,
            commands::character::export_character,
            commands::character::import_character,
            commands::chat::create_session,
            commands::chat::send_message,
            commands::chat::get_history,
            commands::chat::list_sessions,
            commands::chat::delete_session,
            commands::chat::delete_message,
            commands::chat::regenerate_message,
            commands::chat::stop_generation,
            commands::chat::trigger_spontaneous_check,
            commands::chat::edit_and_resend,
            commands::config::get_config,
            commands::config::set_config,
            commands::config::test_llm_connection,
            commands::config::fetch_available_models,
            commands::memory::list_memories,
            commands::memory::update_memory,
            commands::memory::delete_memory,
            commands::memory::generate_memory_manual,
            commands::tts::synthesize_speech,
            commands::tts::test_tts_connection,
            commands::tts::list_voicepeak_emotions,
            commands::tts::generate_speech_for_message,
            commands::attachment::process_attachment,
            commands::attachment::get_supported_extensions,
            commands::plugin::list_plugins,
            commands::plugin::enable_plugin,
            commands::plugin::disable_plugin,
            commands::plugin::get_plugin_config,
            commands::plugin::set_plugin_config,
            commands::thought::get_thoughts,
            commands::thought::start_thought_engine,
            commands::thought::stop_thought_engine,
            commands::thought::delete_thought,
            commands::thought::pause_thought_engine,
            commands::thought::resume_thought_engine,
            commands::thought::pause_spontaneous,
            commands::thought::resume_spontaneous,
            commands::debug::debug_compress_memory,
            commands::debug::debug_generate_thought,
            commands::debug::debug_trigger_spontaneous,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
