// Application State — Tauri managed state

use std::sync::Arc;

use crate::attachment::processor::AttachmentProcessor;
use crate::character::creator::CharacterCreator;
use crate::chat::engine::ChatEngine;
use crate::config::model_config::ModelConfigManager;
use crate::llm::client::LLMClient;
use crate::memory::manager::MemoryManager;
use crate::plugin::registry::PluginRegistry;
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
}
