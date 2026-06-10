import type { ToolCall } from './plugin';

/** チャットメッセージのロール */
export type ChatRole = 'user' | 'assistant' | 'spontaneous' | 'tool';

/** チャットセッション */
export interface ChatSession {
  id: string;
  character_id: string;
  title?: string;
  last_message_at?: string;
  last_message_preview?: string;
  created_at: string;
}

/** チャットメッセージレコード（DB保存用） */
export interface ChatMessageRecord {
  id: string;
  session_id: string;
  role: ChatRole;
  content: string;
  /** 添付ファイル情報 */
  attachments?: MessageAttachment[];
  /** tool_callリクエスト（role=assistant時） */
  tool_calls?: ToolCall[];
  /** tool結果のtool_call参照ID（role=tool時） */
  tool_call_id?: string;
  /** LLMが返したthinking/reasoning content */
  thinking_content?: string | null;
  created_at: string;
}

/** メッセージに添付されたファイル情報 */
export interface MessageAttachment {
  file_name: string;
  attachment_type: string;
  extracted_text?: string;
  base64_data?: string;
}
