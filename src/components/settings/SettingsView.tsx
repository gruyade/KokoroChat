import { useEffect, useState } from 'react';
import { Settings, Save, Loader2 } from 'lucide-react';
import { useConfigStore } from '../../stores';
import { ModelConfigForm } from './ModelConfigForm';
import type { AppConfig, ModelPurpose, ModelSettings, SendKey } from '../../types';

type SettingsTab = 'models' | 'tts' | 'spontaneous' | 'thought' | 'general';

const TABS: { id: SettingsTab; label: string }[] = [
  { id: 'models', label: 'モデル設定' },
  { id: 'tts', label: 'TTS' },
  { id: 'spontaneous', label: '自発的発話' },
  { id: 'thought', label: '思考' },
  { id: 'general', label: '一般' },
];

const MODEL_PURPOSES: { purpose: ModelPurpose; label: string }[] = [
  { purpose: 'chat', label: 'チャット' },
  { purpose: 'memory', label: '記憶圧縮' },
  { purpose: 'thought', label: '思考生成' },
  { purpose: 'character_generation', label: 'キャラクター生成' },
];

export function SettingsView() {
  const { config, loading, error, fetchConfig, updateConfig } = useConfigStore();
  const [activeTab, setActiveTab] = useState<SettingsTab>('models');
  const [draft, setDraft] = useState<AppConfig | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  useEffect(() => {
    if (config) {
      setDraft(structuredClone(config));
    }
  }, [config]);

  const handleSave = async () => {
    if (!draft) return;
    setSaving(true);
    try {
      await updateConfig(draft);
    } finally {
      setSaving(false);
    }
  };

  if (loading && !config) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        <Loader2 className="w-5 h-5 animate-spin mr-2" />
        読み込み中...
      </div>
    );
  }

  if (!draft) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        設定の読み込みに失敗
      </div>
    );
  }

  const updateModelSettings = (purpose: ModelPurpose, settings: ModelSettings) => {
    setDraft({ ...draft, models: { ...draft.models, [purpose]: settings } });
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-2">
          <Settings className="w-5 h-5" />
          <h1 className="text-xl font-semibold">設定</h1>
        </div>
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-3 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 flex items-center gap-2 transition-colors"
        >
          {saving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}
          保存
        </button>
      </div>

      {/* Error */}
      {error && (
        <div className="mb-4 p-3 rounded-md bg-destructive/10 text-destructive text-sm">
          {error}
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-1 mb-6 border-b border-border">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
              activeTab === tab.id
                ? 'border-primary text-primary'
                : 'border-transparent text-muted-foreground hover:text-foreground'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-y-auto space-y-4">
        {activeTab === 'models' && (
          <div className="space-y-4">
            {MODEL_PURPOSES.map(({ purpose, label }) => (
              <ModelConfigForm
                key={purpose}
                purpose={purpose}
                label={label}
                settings={draft.models[purpose]}
                onChange={(s) => updateModelSettings(purpose, s)}
              />
            ))}
          </div>
        )}

        {activeTab === 'tts' && (
          <div className="space-y-4 p-4 rounded-lg border border-border">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-medium">TTS（音声合成）</h3>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={draft.tts.enabled}
                  onChange={(e) =>
                    setDraft({ ...draft, tts: { ...draft.tts, enabled: e.target.checked } })
                  }
                  className="rounded"
                />
                <span className="text-sm">有効</span>
              </label>
            </div>
            <p className="text-xs text-muted-foreground">
              TTS設定はキャラクター個別に設定可能。ここではグローバルの有効/無効を切り替え。
            </p>
          </div>
        )}

        {activeTab === 'spontaneous' && (
          <div className="space-y-4 p-4 rounded-lg border border-border">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-medium">自発的発話</h3>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={draft.spontaneous.enabled}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      spontaneous: { ...draft.spontaneous, enabled: e.target.checked },
                    })
                  }
                  className="rounded"
                />
                <span className="text-sm">有効</span>
              </label>
            </div>
            <div>
              <label
                htmlFor="spontaneous-interval"
                className="block text-xs text-muted-foreground mb-1"
              >
                最小間隔（秒）: {draft.spontaneous.min_interval_seconds}
              </label>
              <input
                id="spontaneous-interval"
                type="number"
                min={10}
                max={3600}
                value={draft.spontaneous.min_interval_seconds}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    spontaneous: {
                      ...draft.spontaneous,
                      min_interval_seconds: parseInt(e.target.value) || 60,
                    },
                  })
                }
                className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
              />
            </div>
          </div>
        )}

        {activeTab === 'thought' && (
          <div className="space-y-4 p-4 rounded-lg border border-border">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-medium">独自思考</h3>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={draft.thought.enabled}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      thought: { ...draft.thought, enabled: e.target.checked },
                    })
                  }
                  className="rounded"
                />
                <span className="text-sm">有効</span>
              </label>
            </div>
            <div>
              <label
                htmlFor="thought-interval"
                className="block text-xs text-muted-foreground mb-1"
              >
                生成間隔（分）: {draft.thought.interval_minutes}
              </label>
              <input
                id="thought-interval"
                type="number"
                min={1}
                max={1440}
                value={draft.thought.interval_minutes}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    thought: {
                      ...draft.thought,
                      interval_minutes: parseInt(e.target.value) || 30,
                    },
                  })
                }
                className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
              />
            </div>
            <div>
              <label
                htmlFor="thought-auto-delete"
                className="block text-xs text-muted-foreground mb-1"
              >
                自動削除閾値（分）: {draft.thought.auto_delete_threshold_minutes === 0 ? '無効（全保持）' : draft.thought.auto_delete_threshold_minutes}
              </label>
              <div className="flex gap-2 mb-2">
                {[
                  { label: '無効', value: 0 },
                  { label: '1時間', value: 60 },
                  { label: '6時間', value: 360 },
                  { label: '24時間', value: 1440 },
                  { label: '7日', value: 10080 },
                ].map((preset) => (
                  <button
                    key={preset.value}
                    type="button"
                    onClick={() =>
                      setDraft({
                        ...draft,
                        thought: {
                          ...draft.thought,
                          auto_delete_threshold_minutes: preset.value,
                        },
                      })
                    }
                    className={`px-2 py-1 text-xs rounded-md border transition-colors ${
                      draft.thought.auto_delete_threshold_minutes === preset.value
                        ? 'border-primary bg-primary/10 text-primary'
                        : 'border-border text-muted-foreground hover:text-foreground'
                    }`}
                  >
                    {preset.label}
                  </button>
                ))}
              </div>
              <input
                id="thought-auto-delete"
                type="number"
                min={0}
                max={43200}
                value={draft.thought.auto_delete_threshold_minutes}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    thought: {
                      ...draft.thought,
                      auto_delete_threshold_minutes: parseInt(e.target.value) || 0,
                    },
                  })
                }
                className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
              />
              <p className="mt-1 text-xs text-muted-foreground">
                0 を設定すると自動削除を無効化（全思考を保持）
              </p>
            </div>
          </div>
        )}

        {activeTab === 'general' && (
          <div className="space-y-4">
            {/* Memory Config */}
            <div className="p-4 rounded-lg border border-border space-y-3">
              <h3 className="text-sm font-medium">記憶管理</h3>
              <div>
                <label
                  htmlFor="memory-threshold"
                  className="block text-xs text-muted-foreground mb-1"
                >
                  圧縮閾値（メッセージ数）: {draft.memory.compression_threshold}
                </label>
                <input
                  id="memory-threshold"
                  type="number"
                  min={5}
                  max={200}
                  value={draft.memory.compression_threshold}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      memory: {
                        ...draft.memory,
                        compression_threshold: parseInt(e.target.value) || 50,
                      },
                    })
                  }
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                />
              </div>
            </div>

            {/* Attachment Config */}
            <div className="p-4 rounded-lg border border-border space-y-3">
              <h3 className="text-sm font-medium">添付ファイル</h3>
              <div>
                <label
                  htmlFor="attachment-size"
                  className="block text-xs text-muted-foreground mb-1"
                >
                  最大ファイルサイズ（MB）
                </label>
                <input
                  id="attachment-size"
                  type="number"
                  min={1}
                  max={100}
                  value={Math.round(draft.attachment.max_file_size_bytes / (1024 * 1024))}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      attachment: {
                        ...draft.attachment,
                        max_file_size_bytes: (parseInt(e.target.value) || 10) * 1024 * 1024,
                      },
                    })
                  }
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                />
              </div>
            </div>

            {/* Send Key Config */}
            <div className="p-4 rounded-lg border border-border space-y-3">
              <h3 className="text-sm font-medium">送信キー</h3>
              <div>
                <label
                  htmlFor="send-key"
                  className="block text-xs text-muted-foreground mb-1"
                >
                  メッセージ送信に使用するキー
                </label>
                <select
                  id="send-key"
                  value={draft.ui.send_key}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      ui: {
                        ...draft.ui,
                        send_key: e.target.value as SendKey,
                      },
                    })
                  }
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                >
                  <option value="enter">Enter</option>
                  <option value="ctrl_enter">Ctrl+Enter</option>
                  <option value="shift_enter">Shift+Enter</option>
                </select>
                <p className="mt-1 text-xs text-muted-foreground">
                  選択したキー以外のEnter系コンビネーションは改行挿入になる
                </p>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
