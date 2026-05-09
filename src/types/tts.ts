/** TTS音声合成プロバイダー */
export type TTSProvider = 'irodori-tts' | 'voicepeak';

/** Irodori-TTSモード */
export type IrodoriMode = 'caption' | 'reference_audio';

/** VoicePeak感情パラメータ（ナレーターごとに異なるキー） */
export type EmotionParams = Record<string, number>;

/** TTS設定（キャラクター個別） */
export interface TTSConfig {
  provider: TTSProvider;
  /** Irodori-TTS用ベースURL */
  base_url?: string;
  /** Irodori-TTS: 参照音声ファイルパス */
  reference_audio_path?: string;
  /** Irodori-TTS: キャプション */
  caption?: string;
  /** VoicePeak: ナレーター名 */
  narrator?: string;
  /** VoicePeak: 感情パラメータ */
  emotion?: EmotionParams;
  /** 読み上げ速度 (50-200) */
  speed?: number;
  /** ピッチ (-300〜300) */
  pitch?: number;
  /** Irodori-TTS動作モード */
  irodori_mode?: IrodoriMode;
}

/** TTS完了イベントペイロード */
export interface TTSCompleteEvent {
  session_id: string;
  text: string;
  audio: string;
}

/** TTS生成中イベントペイロード */
export interface TTSGeneratingEvent {
  session_id: string;
}

/** TTSエラーイベントペイロード */
export interface TTSErrorEvent {
  session_id: string;
  text: string;
  error: string;
}
