/** モデルの用途 */
export type ModelPurpose = 'chat' | 'memory' | 'thought' | 'character_generation';

/** テーマ */
export type Theme = 'light' | 'dark';

/** LLMプロバイダー種別 */
export type LLMProvider = 'openai' | 'anthropic' | 'google' | 'openai_compatible';

/** LLMモデル接続設定 */
export interface ModelSettings {
  provider?: LLMProvider;
  base_url: string;
  model: string;
  api_key?: string;
  temperature: number;
}

/** 自発的発話設定 */
export interface SpontaneousConfig {
  enabled: boolean;
  min_interval_seconds: number;
  probability: number;
}

/** 独自思考設定 */
export interface ThoughtConfig {
  enabled: boolean;
  interval_minutes: number;
  auto_delete_threshold_minutes: number;
}

/** 記憶管理設定 */
export interface MemoryConfig {
  /** 圧縮トリガーとなるメッセージ数閾値 */
  compression_threshold: number;
}

/** TTS全体設定 */
export interface TTSGlobalConfig {
  enabled: boolean;
  /** VoicePeak CLI実行ファイルパス */
  voicepeak_path?: string;
  /** TTS生成タイムアウト（秒）。デフォルト: 60 */
  timeout_seconds?: number;
  /** テキスト分割の最大チャンクサイズ（文字数）。デフォルト: 140 */
  max_chunk_size?: number;
}

/** 送信キー設定 */
export type SendKey = 'enter' | 'ctrl_enter' | 'shift_enter';

/** UI設定 */
export interface UIConfig {
  theme: Theme;
  language: string;
  send_key: SendKey;
}

/** プラグイン設定 */
export interface PluginsConfig {
  enabled_plugins: string[];
  /** プラグイン名 → 固有設定 */
  plugin_settings: Record<string, unknown>;
}

/** 添付ファイル設定 */
export interface AttachmentConfig {
  /** デフォルト10MB */
  max_file_size_bytes: number;
  allowed_extensions: string[];
}

/** アプリケーション全体設定 */
export interface AppConfig {
  models: Record<ModelPurpose, ModelSettings>;
  spontaneous: SpontaneousConfig;
  thought: ThoughtConfig;
  memory: MemoryConfig;
  tts: TTSGlobalConfig;
  ui: UIConfig;
  plugins: PluginsConfig;
  attachment: AttachmentConfig;
}
