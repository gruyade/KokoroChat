/** キャラクターの独自思考 */
export interface Thought {
  id: string;
  character_id: string;
  content: string;
  /** 思考生成時の参照コンテキスト概要 */
  context?: string;
  created_at: string;
}
