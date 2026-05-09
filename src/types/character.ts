import type { TTSConfig } from './tts';

/** AIキャラクター */
export interface Character {
  id: string;
  name: string;
  /** ユーザーが入力した概要説明 */
  description: string;
  /** LLM生成 or 手動編集されたシステムプロンプト */
  system_prompt: string;
  avatar_path?: string;
  tts_config?: TTSConfig;
  /** ISO 8601 */
  created_at: string;
  /** ISO 8601 */
  updated_at: string;
}

/** キャラクター更新用（部分更新対応） */
export interface CharacterUpdate {
  name?: string;
  description?: string;
  system_prompt?: string;
  avatar_path?: string;
  tts_config?: TTSConfig;
  /** trueの場合、avatar_pathをNULLに更新する */
  clear_avatar?: boolean;
  /** trueの場合、tts_configをNULLに更新する */
  clear_tts?: boolean;
}
