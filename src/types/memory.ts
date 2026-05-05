/** 記憶（LLMによる会話要約） */
export interface Memory {
  id: string;
  character_id: string;
  /** LLMによる要約テキスト */
  content: string;
  source_session_id?: string;
  source_message_from?: string;
  source_message_to?: string;
  created_at: string;
  updated_at: string;
}
