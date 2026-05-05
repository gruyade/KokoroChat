/** TTS音声合成プロバイダー */
export type TTSProvider = 'irodori-tts' | 'voicepeak';

/** VoicePeak感情パラメータ */
export interface EmotionParams {
  happy?: number;
  fun?: number;
  angry?: number;
  sad?: number;
}

/** TTS設定（キャラクター個別） */
export interface TTSConfig {
  provider: TTSProvider;
  base_url: string;
  /** Irodori-TTS: 参照音声ファイルパス */
  reference_audio_path?: string;
  /** Irodori-TTS: キャプション */
  caption?: string;
  /** VoicePeak: ナレーター名 */
  narrator?: string;
  /** VoicePeak: 感情パラメータ */
  emotion?: EmotionParams;
  /** 読み上げ速度 */
  speed?: number;
  /** ピッチ */
  pitch?: number;
}
