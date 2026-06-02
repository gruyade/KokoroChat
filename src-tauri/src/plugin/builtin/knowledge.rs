// ナレッジプラグイン — get_knowledge ツールを提供

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;

use crate::db::database::Database;
use crate::db::repositories::knowledge as knowledge_repo;
use crate::error::AppError;
use crate::models::plugin::{ToolCall, ToolDefinition, ToolResult};
use crate::plugin::system::PluginHandler;

/// ナレッジプラグイン — tool_reference モードのナレッジをget_knowledgeツールで提供
pub struct KnowledgePlugin {
    db: Arc<Mutex<Database>>,
}

impl KnowledgePlugin {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl<R: tauri::Runtime> PluginHandler<R> for KnowledgePlugin {
    fn name(&self) -> &str {
        "knowledge"
    }

    fn description(&self) -> &str {
        concat!(
            "[Purpose] A plugin that provides the AI with access to user-uploaded knowledge files. ",
            "Offers a get_knowledge tool that retrieves the content of a specific knowledge file by name.\n",
            "[When to use] Enable this plugin when knowledge files with injection_mode=tool_reference are registered in the session. ",
            "The AI can call get_knowledge to retrieve file content on demand without consuming prompt space every turn.\n",
            "[Out of scope] Cannot modify or delete knowledge entries. ",
            "Cannot access knowledge entries from other sessions.",
        )
    }

    fn tools(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "get_knowledge".to_string(),
            description: concat!(
                "[Purpose] Retrieve the full content of a knowledge file registered in the current session.\n",
                "[When to use] When you need to reference detailed information from a knowledge file. ",
                "Call this tool with the exact file_name of the knowledge entry you want to access.\n",
                "[Out of scope] Cannot modify knowledge entries. Only returns content for files registered with tool_reference mode.",
            ).to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_name": {
                        "type": "string",
                        "description": "The exact file name of the knowledge entry to retrieve. Must match case-sensitively."
                    }
                },
                "required": ["file_name"]
            }),
        }]
    }

    async fn execute(
        &self,
        tool_call: &ToolCall,
        _app_handle: &tauri::AppHandle<R>,
    ) -> Result<ToolResult, AppError> {
        match tool_call.name.as_str() {
            "get_knowledge" => self.execute_get_knowledge(tool_call),
            _ => Err(AppError::Plugin(format!(
                "Unknown tool: {}",
                tool_call.name
            ))),
        }
    }
}

impl KnowledgePlugin {
    /// get_knowledge ツール実行: file_name引数で指定されたエントリのcontentを返す
    fn execute_get_knowledge(&self, tool_call: &ToolCall) -> Result<ToolResult, AppError> {
        let file_name = tool_call
            .arguments
            .get("file_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Plugin("'file_name' パラメータが必要".to_string()))?;

        // コンテキストからsession_idを取得
        let session_id = tool_call
            .context
            .as_ref()
            .and_then(|ctx| ctx.session_id.as_ref())
            .ok_or_else(|| AppError::Plugin("session_id が取得できない".to_string()))?;

        let db_guard = self
            .db
            .lock()
            .map_err(|e| AppError::Plugin(format!("DB lock失敗: {}", e)))?;
        let conn = db_guard.connection();

        // tool_reference モードかつ enabled のエントリからコンテンツを検索
        let entries = knowledge_repo::get_tool_reference_entries(conn, session_id)?;

        // file_name が一致するエントリを探す
        if let Some(entry) = entries.iter().find(|e| e.file_name == file_name) {
            Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: entry.content.clone(),
                is_error: false,
            })
        } else {
            // 一致しない場合: 利用可能なファイル名一覧をエラーメッセージに含める
            let available: Vec<&str> = entries.iter().map(|e| e.file_name.as_str()).collect();
            let available_list = if available.is_empty() {
                "（なし）".to_string()
            } else {
                available.join(", ")
            };

            Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: format!(
                    "'{}' に一致するナレッジなし。利用可能: [{}]",
                    file_name, available_list
                ),
                is_error: true,
            })
        }
    }
}
