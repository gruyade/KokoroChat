/** OpenAI Function Calling互換のツール定義 */
export interface ToolDefinition {
  name: string;
  description: string;
  /** JSON Schema形式のパラメータ定義 */
  parameters: Record<string, unknown>;
}

/** LLMからのtool_callリクエスト */
export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}

/** ツール実行結果 */
export interface ToolResult {
  tool_call_id: string;
  content: string;
  is_error: boolean;
}

/** プラグインメタデータ */
export interface PluginInfo {
  name: string;
  description: string;
  version: string;
  enabled: boolean;
  tools: ToolDefinition[];
  /** プラグイン固有設定 */
  config?: Record<string, unknown>;
}
