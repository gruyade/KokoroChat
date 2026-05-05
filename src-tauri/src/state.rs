// Application State — Tauri managed state

use std::sync::Arc;

use crate::character::creator::CharacterCreator;
use crate::chat::engine::ChatEngine;

/// Tauriアプリケーション全体で共有される状態
pub struct AppState {
    pub character_creator: Arc<dyn CharacterCreator>,
    pub chat_engine: Arc<dyn ChatEngine>,
}
