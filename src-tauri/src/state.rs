// Application State — Tauri managed state

use std::sync::{Arc, Mutex};

use crate::attachment::processor::AttachmentProcessor;
use crate::character::creator::CharacterCreator;
use crate::chat::engine::ChatEngine;
use crate::config::model_config::ModelConfigManager;
use crate::db::database::Database;
use crate::llm::client::LLMClient;
use crate::memory::manager::MemoryManager;
use crate::plugin::registry::PluginRegistry;
use crate::thought::engine::ThoughtEngine;
use crate::tts::connector::TTSConnector;

/// Tauriアプリケーション全体で共有される状態
pub struct AppState {
    pub character_creator: Arc<dyn CharacterCreator>,
    pub chat_engine: Arc<dyn ChatEngine>,
    pub memory_manager: Arc<dyn MemoryManager>,
    pub tts_connector: Arc<dyn TTSConnector>,
    pub config_manager: Arc<ModelConfigManager>,
    pub llm_client: Arc<dyn LLMClient>,
    pub attachment_processor: Arc<dyn AttachmentProcessor>,
    pub plugin_registry: Arc<dyn PluginRegistry>,
    pub thought_engine: Arc<dyn ThoughtEngine>,
    /// LLMリクエスト直列化用グローバルロック
    pub llm_lock: Arc<tokio::sync::Mutex<()>>,
    /// デバッグ用DB参照
    pub db: Arc<Mutex<Database>>,
}

impl AppState {
    /// DB接続を取得（デバッグコマンド用）
    pub fn chat_engine_db(&self) -> Result<std::sync::MutexGuard<'_, Database>, String> {
        self.db.lock().map_err(|e| format!("DB lock failed: {}", e))
    }
}
