/** ナレッジエントリの注入モード */
export type InjectionMode = 'system_prompt' | 'tool_reference';

/** ナレッジエントリのメタデータ（content除外の軽量表現） */
export interface KnowledgeEntryMeta {
  id: string;
  file_name: string;
  size_bytes: number;
  enabled: boolean;
  injection_mode: InjectionMode;
  created_at: string;
}
