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

/** カスタムツールの実行方式 */
export type CustomToolType = 'http' | 'cli';

/** カスタムツール登録リクエスト */
export interface CustomToolRequest {
  name: string;
  description: string;
  type: CustomToolType;
  /** HTTP方式の場合のエンドポイントURL */
  endpoint?: string;
  /** CLI方式の場合のコマンド */
  command?: string;
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
  /** 組み込みプラグインかどうか（trueの場合削除不可） */
  builtin?: boolean;
}
