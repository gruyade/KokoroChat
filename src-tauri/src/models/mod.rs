pub mod attachment;
pub mod character;
pub mod chat;
pub mod config;
pub mod memory;
pub mod plugin;
pub mod thought;
pub mod tts;

// Re-export all public types
pub use attachment::{Attachment, AttachmentType, MAX_FILE_SIZE};
pub use character::{Character, CharacterUpdate};
pub use chat::{ChatMessageRecord, ChatRole, ChatSession, ChatToolPermission, MessageAttachment};
pub use config::{
    AppConfig, AttachmentConfig, LLMProvider, MemoryConfig, ModelPurpose, ModelSettings,
    PluginsConfig, SpontaneousConfig, TTSGlobalConfig, Theme, ThoughtConfig, UIConfig,
};
pub use memory::Memory;
pub use plugin::{
    CliToolConfig, CustomToolRecord, CustomToolType, HttpToolConfig, PluginInfo, ToolCall,
    ToolDefinition, ToolResult,
};
pub use thought::Thought;
pub use tts::{EmotionParams, TTSConfig, TTSProvider};
