import { useState, useEffect, useCallback } from 'react';
import { Save, X, Wand2, Sparkles, Loader2, Camera } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import type { Character, TTSConfig, TTSProvider, EmotionParams, IrodoriMode } from '../../types';
import { AvatarCropDialog } from './AvatarCropDialog';
import { AvatarImage } from '../common/AvatarImage';

/** TTS設定セクションのローカルステート */
interface TTSFormState {
  enabled: boolean;
  provider: TTSProvider;
  irodori: {
    base_url: string;
    reference_audio_path: string;
    caption: string;
    irodori_mode: IrodoriMode;
  };
  voicepeak: {
    narrator: string;
    emotion: EmotionParams;
    speed: string;
    pitch: string;
  };
}

const DEFAULT_TTS_STATE: TTSFormState = {
  enabled: false,
  provider: 'voicepeak',
  irodori: {
    base_url: '',
    reference_audio_path: '',
    caption: '',
    irodori_mode: 'caption',
  },
  voicepeak: {
    narrator: '',
    emotion: {},
    speed: '100',
    pitch: '100',
  },
};

interface CharacterFormProps {
  character?: Character | null;
  onSave: (data: {
    name: string;
    description: string;
    system_prompt: string;
    avatar_path?: string;
    tts_config?: TTSConfig;
  }) => void;
  onCancel: () => void;
  loading?: boolean;
}

export function CharacterForm({ character, onSave, onCancel, loading }: CharacterFormProps) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [systemPrompt, setSystemPrompt] = useState('');
  const [avatarPath, setAvatarPath] = useState<string | null>(null);
  const [cropImage, setCropImage] = useState<string | null>(null);
  const [generating, setGenerating] = useState(false);
  const [improving, setImproving] = useState(false);
  const [improveDirection, setImproveDirection] = useState('');
  const [ttsState, setTtsState] = useState<TTSFormState>(DEFAULT_TTS_STATE);
  const [emotionKeys, setEmotionKeys] = useState<string[]>([]);
  const [emotionLoading, setEmotionLoading] = useState(false);

  useEffect(() => {
    if (character) {
      setName(character.name);
      setDescription(character.description);
      setSystemPrompt(character.system_prompt);
      setAvatarPath(character.avatar_path ?? null);
    } else {
      setName('');
      setDescription('');
      setSystemPrompt('');
      setAvatarPath(null);
    }
  }, [character]);

  // TTS設定の初期値ロード
  useEffect(() => {
    if (character?.tts_config) {
      setTtsState({
        enabled: true,
        provider: character.tts_config.provider,
        irodori: {
          base_url: character.tts_config.base_url ?? '',
          reference_audio_path: character.tts_config.reference_audio_path ?? '',
          caption: character.tts_config.caption ?? '',
          irodori_mode: character.tts_config.irodori_mode ?? 'caption',
        },
        voicepeak: {
          narrator: character.tts_config.narrator ?? '',
          emotion: character.tts_config.emotion ?? {},
          speed: character.tts_config.speed?.toString() ?? '',
          pitch: character.tts_config.pitch?.toString() ?? '',
        },
      });
    } else {
      setTtsState(DEFAULT_TTS_STATE);
    }
  }, [character]);

  // ナレーター変更時に感情リストを取得
  const fetchEmotionKeys = useCallback(async (narrator: string) => {
    if (!narrator.trim()) {
      setEmotionKeys([]);
      return;
    }
    setEmotionLoading(true);
    try {
      const keys = await invoke<string[]>('list_voicepeak_emotions', { narrator: narrator.trim() });
      setEmotionKeys(keys);
      // 新しいキーに合わせてemotionを初期化（既存値は保持）
      setTtsState((prev) => {
        const newEmotion: Record<string, number> = {};
        for (const key of keys) {
          newEmotion[key] = prev.voicepeak.emotion[key] ?? 0;
        }
        return {
          ...prev,
          voicepeak: { ...prev.voicepeak, emotion: newEmotion },
        };
      });
    } catch {
      // 取得失敗時は既存のemotionキーをそのまま使用
      setEmotionKeys(Object.keys(ttsState.voicepeak.emotion));
    } finally {
      setEmotionLoading(false);
    }
  }, [ttsState.voicepeak.emotion]);

  // 初期ロード時にナレーターが設定済みなら感情リスト取得
  useEffect(() => {
    if (ttsState.enabled && ttsState.provider === 'voicepeak' && ttsState.voicepeak.narrator) {
      fetchEmotionKeys(ttsState.voicepeak.narrator);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [character]);

  const handleAvatarSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => setCropImage(reader.result as string);
    reader.readAsDataURL(file);
    // Reset input so same file can be re-selected
    e.target.value = '';
  };

  const handleAvatarCrop = async (base64: string) => {
    setCropImage(null);
    try {
      const path = await invoke<string>('save_avatar', { base64Data: base64 });
      setAvatarPath(path);
    } catch (e) {
      console.error('Avatar save failed:', e);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;

    let tts_config: TTSConfig | undefined = undefined;
    if (ttsState.enabled) {
      if (ttsState.provider === 'voicepeak') {
        tts_config = {
          provider: 'voicepeak',
          narrator: ttsState.voicepeak.narrator || undefined,
          // 感情キーが存在する場合は値が0でも保存（キー情報がEmotionGeneratorに必要）
          emotion: Object.keys(ttsState.voicepeak.emotion).length > 0
            ? ttsState.voicepeak.emotion
            : undefined,
          speed: ttsState.voicepeak.speed ? Number(ttsState.voicepeak.speed) : undefined,
          pitch: ttsState.voicepeak.pitch ? Number(ttsState.voicepeak.pitch) : undefined,
        };
      } else {
        tts_config = {
          provider: 'irodori-tts',
          base_url: ttsState.irodori.base_url || undefined,
          irodori_mode: ttsState.irodori.irodori_mode,
          reference_audio_path: ttsState.irodori.irodori_mode === 'reference_audio'
            ? ttsState.irodori.reference_audio_path || undefined
            : undefined,
          caption: ttsState.irodori.irodori_mode === 'caption'
            ? ttsState.irodori.caption || undefined
            : undefined,
        };
      }
    }

    onSave({
      name: name.trim(),
      description: description.trim(),
      system_prompt: systemPrompt,
      avatar_path: avatarPath ?? undefined,
      tts_config,
    });
  };

  const handleGenerate = async () => {
    if (!name.trim() || !description.trim()) return;
    setGenerating(true);
    try {
      const prompt = await invoke<string>('generate_system_prompt', {
        name: name.trim(),
        description: description.trim(),
      });
      setSystemPrompt(prompt);
    } catch (e) {
      alert(`生成エラー: ${e}`);
    } finally {
      setGenerating(false);
    }
  };

  const handleImprove = async () => {
    if (!name.trim() || !systemPrompt.trim()) return;
    setImproving(true);
    try {
      const improved = await invoke<string>('improve_system_prompt', {
        name: name.trim(),
        description: description.trim(),
        currentPrompt: systemPrompt,
        direction: improveDirection.trim() || null,
      });
      setSystemPrompt(improved);
    } catch (e) {
      alert(`改善エラー: ${e}`);
    } finally {
      setImproving(false);
    }
  };

  const isEditing = !!character;
  const isProcessing = generating || improving || loading;

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">
          {isEditing ? 'キャラクター編集' : '新規キャラクター作成'}
        </h2>
        <button
          type="button"
          onClick={onCancel}
          className="p-1.5 rounded hover:bg-muted text-muted-foreground"
          aria-label="キャンセル"
        >
          <X className="w-5 h-5" />
        </button>
      </div>

      {/* Avatar */}
      <div className="flex items-center gap-4">
        <label className="relative cursor-pointer group">
          <input
            type="file"
            accept="image/*"
            onChange={handleAvatarSelect}
            className="hidden"
          />
          <div className="w-16 h-16 rounded-full bg-muted border border-border flex items-center justify-center overflow-hidden group-hover:ring-2 group-hover:ring-primary transition-all">
            {avatarPath ? (
              <AvatarImage
                avatarPath={avatarPath}
                alt="アバター"
                className="w-full h-full object-cover"
              />
            ) : (
              <Camera className="w-6 h-6 text-muted-foreground" />
            )}
          </div>
          <span className="absolute -bottom-1 -right-1 p-0.5 rounded-full bg-primary text-primary-foreground opacity-0 group-hover:opacity-100 transition-opacity">
            <Camera className="w-3 h-3" />
          </span>
        </label>
        <p className="text-xs text-muted-foreground">クリックしてアバター画像を選択</p>
      </div>

      {/* Crop Dialog */}
      {cropImage && (
        <AvatarCropDialog
          imageSrc={cropImage}
          onConfirm={handleAvatarCrop}
          onCancel={() => setCropImage(null)}
        />
      )}

      {/* Name */}
      <div>
        <label htmlFor="char-name" className="block text-sm font-medium mb-1">
          名前
        </label>
        <input
          id="char-name"
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="キャラクター名"
          className="w-full px-3 py-2 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
          required
        />
      </div>

      {/* Description */}
      <div>
        <label htmlFor="char-desc" className="block text-sm font-medium mb-1">
          概要
        </label>
        <textarea
          id="char-desc"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="キャラクターの概要説明（性格、背景など）"
          rows={3}
          className="w-full px-3 py-2 rounded-md border border-border bg-background text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary"
        />
      </div>

      {/* System Prompt */}
      <div>
        <div className="flex items-center justify-between mb-1">
          <label htmlFor="char-prompt" className="text-sm font-medium">
            System Prompt
          </label>
          <div className="flex gap-1">
            {/* 生成ボタン */}
            <button
              type="button"
              onClick={handleGenerate}
              disabled={isProcessing || !name.trim() || !description.trim()}
              className="px-2 py-1 text-xs rounded-md bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1 transition-colors"
              title="説明内容からSystem Promptを生成"
            >
              {generating ? <Loader2 className="w-3 h-3 animate-spin" /> : <Wand2 className="w-3 h-3" />}
              生成
            </button>
            {/* 改善ボタン */}
            <button
              type="button"
              onClick={handleImprove}
              disabled={isProcessing || !name.trim() || !systemPrompt.trim()}
              className="px-2 py-1 text-xs rounded-md bg-accent text-accent-foreground hover:bg-accent/80 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1 transition-colors"
              title="現在のSystem Promptを改善"
            >
              {improving ? <Loader2 className="w-3 h-3 animate-spin" /> : <Sparkles className="w-3 h-3" />}
              改善
            </button>
          </div>
        </div>
        <textarea
          id="char-prompt"
          value={systemPrompt}
          onChange={(e) => setSystemPrompt(e.target.value)}
          placeholder="キャラクターのSystem Prompt（生成ボタンで自動作成可能）"
          rows={10}
          className="w-full px-3 py-2 rounded-md border border-border bg-background text-sm font-mono resize-y focus:outline-none focus:ring-2 focus:ring-primary"
        />
        <p className="text-xs text-muted-foreground mt-1">
          「生成」: 概要から新規作成 / 「改善」: 現在の内容をLLMで改良
        </p>
        {/* 改善方向性入力欄 */}
        <div className="mt-2">
          <label htmlFor="improve-direction" className="block text-xs text-muted-foreground mb-1">
            改善の方向性（任意）
          </label>
          <input
            id="improve-direction"
            type="text"
            value={improveDirection}
            onChange={(e) => setImproveDirection(e.target.value)}
            placeholder="例: もっとツンデレ感を出して、語尾に「〜だし」を付けて"
            className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </div>
      </div>

      {/* TTS Settings */}
      <fieldset className="border border-border rounded-md p-4 space-y-3">
        <legend className="text-sm font-medium px-1">🔊 TTS設定</legend>

        {/* TTS Toggle */}
        <div className="flex items-center gap-2">
          <input
            id="tts-enabled"
            type="checkbox"
            role="switch"
            aria-checked={ttsState.enabled}
            checked={ttsState.enabled}
            onChange={(e) =>
              setTtsState((prev) => ({ ...prev, enabled: e.target.checked }))
            }
            className="w-4 h-4 rounded border-border text-primary focus:ring-primary"
          />
          <label htmlFor="tts-enabled" className="text-sm">
            TTSを有効にする
          </label>
        </div>

        {/* Provider Selection */}
        <div role="radiogroup" aria-label="TTSプロバイダー選択">
          <p className="text-sm font-medium mb-1">プロバイダー:</p>
          <div className="flex flex-col gap-1">
            <label className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                name="tts-provider"
                value="irodori-tts"
                checked={ttsState.provider === 'irodori-tts'}
                disabled={!ttsState.enabled}
                aria-disabled={!ttsState.enabled}
                onChange={() =>
                  setTtsState((prev) => ({ ...prev, provider: 'irodori-tts' }))
                }
                className="text-primary focus:ring-primary"
              />
              TTSサーバー（Irodori-TTS）
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                name="tts-provider"
                value="voicepeak"
                checked={ttsState.provider === 'voicepeak'}
                disabled={!ttsState.enabled}
                aria-disabled={!ttsState.enabled}
                onChange={() =>
                  setTtsState((prev) => ({ ...prev, provider: 'voicepeak' }))
                }
                className="text-primary focus:ring-primary"
              />
              VoicePeak（CLI方式）
            </label>
          </div>
        </div>

        {/* Provider-specific fields (hidden when TTS disabled) */}
        {ttsState.enabled && ttsState.provider === 'irodori-tts' && (
          <div className="space-y-2 border-t border-border pt-3">
            <p className="text-xs font-medium text-muted-foreground">Irodori-TTS設定</p>

            {/* Mode selector */}
            <div>
              <p className="text-xs mb-1">動作モード:</p>
              <div className="flex gap-4">
                <label className="flex items-center gap-2 text-xs">
                  <input
                    type="radio"
                    name="irodori-mode"
                    value="caption"
                    checked={ttsState.irodori.irodori_mode === 'caption'}
                    onChange={() =>
                      setTtsState((prev) => ({
                        ...prev,
                        irodori: { ...prev.irodori, irodori_mode: 'caption' },
                      }))
                    }
                    className="text-primary focus:ring-primary"
                  />
                  キャプションモード
                </label>
                <label className="flex items-center gap-2 text-xs">
                  <input
                    type="radio"
                    name="irodori-mode"
                    value="reference_audio"
                    checked={ttsState.irodori.irodori_mode === 'reference_audio'}
                    onChange={() =>
                      setTtsState((prev) => ({
                        ...prev,
                        irodori: { ...prev.irodori, irodori_mode: 'reference_audio' },
                      }))
                    }
                    className="text-primary focus:ring-primary"
                  />
                  参照音声モード
                </label>
              </div>
            </div>

            {/* Caption mode: show caption field */}
            {ttsState.irodori.irodori_mode === 'caption' && (
              <div>
                <label htmlFor="tts-caption" className="block text-xs mb-0.5">
                  ベース声キャプション <span className="text-destructive">*</span>
                </label>
                <input
                  id="tts-caption"
                  type="text"
                  value={ttsState.irodori.caption}
                  onChange={(e) =>
                    setTtsState((prev) => ({
                      ...prev,
                      irodori: { ...prev.irodori, caption: e.target.value },
                    }))
                  }
                  placeholder="参照音声の説明テキスト（例: 明るく元気な女性の声）"
                  required
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                />
              </div>
            )}

            {/* Reference audio mode: show reference audio path field */}
            {ttsState.irodori.irodori_mode === 'reference_audio' && (
              <div>
                <label htmlFor="tts-ref-audio" className="block text-xs mb-0.5">
                  参照音声ファイルパス <span className="text-destructive">*</span>
                </label>
                <input
                  id="tts-ref-audio"
                  type="text"
                  value={ttsState.irodori.reference_audio_path}
                  onChange={(e) =>
                    setTtsState((prev) => ({
                      ...prev,
                      irodori: { ...prev.irodori, reference_audio_path: e.target.value },
                    }))
                  }
                  placeholder="/path/to/reference.wav"
                  required
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                />
              </div>
            )}
          </div>
        )}

        {ttsState.enabled && ttsState.provider === 'voicepeak' && (
          <div className="space-y-2 border-t border-border pt-3">
            <p className="text-xs font-medium text-muted-foreground">VoicePeak設定</p>
            <div>
              <label htmlFor="tts-narrator" className="block text-xs mb-0.5">
                ナレーター
              </label>
              <input
                id="tts-narrator"
                type="text"
                value={ttsState.voicepeak.narrator}
                onChange={(e) =>
                  setTtsState((prev) => ({
                    ...prev,
                    voicepeak: { ...prev.voicepeak, narrator: e.target.value },
                  }))
                }
                onBlur={(e) => fetchEmotionKeys(e.target.value)}
                placeholder="Japanese Female 1"
                className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
              />
            </div>
            <div className="grid grid-cols-2 gap-2">
              <div>
                <label htmlFor="tts-speed" className="block text-xs mb-0.5">
                  速度 (%)
                </label>
                <input
                  id="tts-speed"
                  type="number"
                  value={ttsState.voicepeak.speed}
                  onChange={(e) =>
                    setTtsState((prev) => ({
                      ...prev,
                      voicepeak: { ...prev.voicepeak, speed: e.target.value },
                    }))
                  }
                  placeholder="100"
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                />
              </div>
              <div>
                <label htmlFor="tts-pitch" className="block text-xs mb-0.5">
                  ピッチ
                </label>
                <input
                  id="tts-pitch"
                  type="number"
                  value={ttsState.voicepeak.pitch}
                  onChange={(e) =>
                    setTtsState((prev) => ({
                      ...prev,
                      voicepeak: { ...prev.voicepeak, pitch: e.target.value },
                    }))
                  }
                  placeholder="0"
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                />
              </div>
            </div>
            {/* Emotion Sliders */}
            <div className="space-y-1.5">
              <p className="text-xs font-medium">
                感情パラメータ（上限値）
                {emotionLoading && <Loader2 className="inline w-3 h-3 ml-1 animate-spin" />}
              </p>
              {emotionKeys.length > 0 ? (
                emotionKeys.map((key) => (
                  <div key={key} className="flex items-center gap-2">
                    <label htmlFor={`tts-emotion-${key}`} className="text-xs w-16 truncate">
                      {key}
                    </label>
                    <input
                      id={`tts-emotion-${key}`}
                      type="range"
                      min={0}
                      max={100}
                      value={ttsState.voicepeak.emotion[key] ?? 0}
                      onChange={(e) =>
                        setTtsState((prev) => ({
                          ...prev,
                          voicepeak: {
                            ...prev.voicepeak,
                            emotion: {
                              ...prev.voicepeak.emotion,
                              [key]: Number(e.target.value),
                            },
                          },
                        }))
                      }
                      className="flex-1 h-2 rounded-lg appearance-none bg-muted"
                    />
                    <span className="text-xs w-8 text-right tabular-nums">
                      {ttsState.voicepeak.emotion[key] ?? 0}
                    </span>
                  </div>
                ))
              ) : (
                <p className="text-xs text-muted-foreground">
                  ナレーターを入力して感情リストを取得
                </p>
              )}
            </div>
          </div>
        )}
      </fieldset>

      {/* Actions */}
      <div className="flex gap-2 justify-end">
        <button
          type="button"
          onClick={onCancel}
          className="px-4 py-2 text-sm rounded-md border border-border hover:bg-muted transition-colors"
        >
          キャンセル
        </button>
        <button
          type="submit"
          disabled={!name.trim() || isProcessing}
          className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2 transition-colors"
        >
          <Save className="w-4 h-4" />
          {loading ? '保存中...' : '保存'}
        </button>
      </div>
    </form>
  );
}
