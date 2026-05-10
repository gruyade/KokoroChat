import { useEffect, useState } from 'react';
import { Settings, Save, Loader2, Plus, Trash2, Puzzle, Globe, ChevronRight } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useConfigStore, usePluginStore, useUIStore } from '../../stores';
import { ModelConfigForm } from './ModelConfigForm';
import type { AppConfig, ModelPurpose, ModelSettings, SendKey } from '../../types';
import type { CustomToolType } from '../../types/plugin';

/** Web検索プロバイダ種別 */
type SearchProvider = 'tavily' | 'brave';

/** Web検索プラグイン設定 */
interface WebSearchConfig {
  provider: SearchProvider;
  tavily_api_key: string | null;
  brave_api_key: string | null;
  allowed_domains: string[];
}

type SettingsTab = 'models' | 'tts' | 'spontaneous' | 'thought' | 'general' | 'tools';

const TABS: { id: SettingsTab; label: string; badge?: string }[] = [
  { id: 'models', label: 'モデル設定' },
  { id: 'tts', label: 'TTS', badge: 'WIP' },
  { id: 'spontaneous', label: '自発的発話' },
  { id: 'thought', label: '思考' },
  { id: 'general', label: '一般' },
  { id: 'tools', label: 'ツール管理' },
];

const MODEL_PURPOSES: { purpose: ModelPurpose; label: string }[] = [
  { purpose: 'chat', label: 'チャット' },
  { purpose: 'memory', label: '記憶圧縮' },
  { purpose: 'thought', label: '思考生成' },
  { purpose: 'character_generation', label: 'キャラクター生成' },
];

export function SettingsView() {
  const { config, loading, error, fetchConfig, updateConfig } = useConfigStore();
  const {
    plugins,
    loading: pluginsLoading,
    error: pluginsError,
    fetchPlugins,
    enablePlugin,
    disablePlugin,
    registerCustomPlugin,
    removePlugin,
    isBuiltinPlugin,
  } = usePluginStore();
  const { showToast } = useUIStore();
  const [activeTab, setActiveTab] = useState<SettingsTab>('models');
  const [draft, setDraft] = useState<AppConfig | null>(null);
  const [saving, setSaving] = useState(false);
  const [showAddToolForm, setShowAddToolForm] = useState(false);
  const [newToolName, setNewToolName] = useState('');
  const [newToolDescription, setNewToolDescription] = useState('');
  const [newToolType, setNewToolType] = useState<CustomToolType>('http');
  const [newToolEndpoint, setNewToolEndpoint] = useState('');
  const [newToolCommand, setNewToolCommand] = useState('');

  // Web検索設定
  const [webSearchProvider, setWebSearchProvider] = useState<SearchProvider>('tavily');
  const [webSearchTavilyApiKey, setWebSearchTavilyApiKey] = useState('');
  const [webSearchBraveApiKey, setWebSearchBraveApiKey] = useState('');
  const [webSearchAllowedDomains, setWebSearchAllowedDomains] = useState('');
  const [webSearchSaving, setWebSearchSaving] = useState(false);

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  useEffect(() => {
    fetchPlugins();
  }, [fetchPlugins]);

  useEffect(() => {
    if (config) {
      setDraft(structuredClone(config));
    }
  }, [config]);

  // Web検索設定の読み込み
  useEffect(() => {
    const loadWebSearchConfig = async () => {
      try {
        const cfg = await invoke<WebSearchConfig | null>('get_plugin_config', { name: 'web_search' });
        if (cfg) {
          setWebSearchProvider(cfg.provider ?? 'tavily');
          setWebSearchTavilyApiKey(cfg.tavily_api_key ?? '');
          setWebSearchBraveApiKey(cfg.brave_api_key ?? '');
          setWebSearchAllowedDomains((cfg.allowed_domains ?? []).join('\n'));
        }
      } catch {
        // プラグイン未登録時は無視
      }
    };
    loadWebSearchConfig();
  }, []);

  const handleSave = async () => {
    if (!draft) return;
    setSaving(true);
    try {
      await updateConfig(draft);
      showToast('設定を保存した');
    } catch {
      showToast('設定の保存に失敗', 'error');
    } finally {
      setSaving(false);
    }
  };

  const handleSaveWebSearchConfig = async () => {
    setWebSearchSaving(true);
    try {
      const config: WebSearchConfig = {
        provider: webSearchProvider,
        tavily_api_key: webSearchTavilyApiKey.trim() || null,
        brave_api_key: webSearchBraveApiKey.trim() || null,
        allowed_domains: webSearchAllowedDomains
          .split('\n')
          .map((d) => d.trim())
          .filter((d) => d.length > 0),
      };
      await invoke('set_plugin_config', { name: 'web_search', config });
      showToast('Web検索設定を保存した');
    } catch {
      showToast('Web検索設定の保存に失敗', 'error');
    } finally {
      setWebSearchSaving(false);
    }
  };

  const handleAddCustomTool = async () => {
    if (!newToolName.trim() || !newToolDescription.trim()) {
      showToast('名前と説明は必須', 'error');
      return;
    }
    if (newToolType === 'http' && !newToolEndpoint.trim()) {
      showToast('エンドポイントURLは必須', 'error');
      return;
    }
    if (newToolType === 'cli' && !newToolCommand.trim()) {
      showToast('コマンドは必須', 'error');
      return;
    }
    try {
      await registerCustomPlugin({
        name: newToolName.trim(),
        description: newToolDescription.trim(),
        type: newToolType,
        endpoint: newToolType === 'http' ? newToolEndpoint.trim() : undefined,
        command: newToolType === 'cli' ? newToolCommand.trim() : undefined,
      });
      showToast('カスタムツールを追加した');
      setShowAddToolForm(false);
      setNewToolName('');
      setNewToolDescription('');
      setNewToolType('http');
      setNewToolEndpoint('');
      setNewToolCommand('');
    } catch {
      showToast('カスタムツールの追加に失敗', 'error');
    }
  };

  const handleRemovePlugin = async (name: string) => {
    try {
      await removePlugin(name);
      showToast('プラグインを削除した');
    } catch {
      showToast('プラグインの削除に失敗', 'error');
    }
  };

  const handleTogglePlugin = async (name: string, enabled: boolean) => {
    try {
      if (enabled) {
        await enablePlugin(name);
      } else {
        await disablePlugin(name);
      }
    } catch {
      showToast('プラグインの切り替えに失敗', 'error');
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
            className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors flex items-center ${
              activeTab === tab.id
                ? 'border-primary text-primary'
                : 'border-transparent text-muted-foreground hover:text-foreground'
            }`}
          >
            {tab.label}
            {tab.badge && (
              <span className="ml-1 px-1.5 py-0.5 text-[10px] rounded bg-yellow-500/20 text-yellow-600">
                {tab.badge}
              </span>
            )}
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
            <div>
              <label htmlFor="voicepeak-path" className="block text-xs text-muted-foreground mb-1">
                VoicePeak 実行ファイルパス
              </label>
              <input
                id="voicepeak-path"
                type="text"
                value={draft.tts.voicepeak_path ?? ''}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    tts: { ...draft.tts, voicepeak_path: e.target.value || undefined },
                  })
                }
                placeholder="C:\Program Files\VOICEPEAK\voicepeak.exe"
                className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
              />
              <p className="mt-1 text-xs text-muted-foreground">
                未指定時はPATHから「voicepeak」を検索
              </p>
            </div>
            <div>
              <label htmlFor="irodori-caption-base-url" className="block text-xs text-muted-foreground mb-1">
                IrodoriTTS キャプションモード ベースURL
              </label>
              <input
                id="irodori-caption-base-url"
                type="text"
                value={draft.tts.irodori_caption_base_url ?? ''}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    tts: { ...draft.tts, irodori_caption_base_url: e.target.value || undefined },
                  })
                }
                placeholder="http://localhost:8080"
                className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
              />
            </div>
            <div>
              <label htmlFor="irodori-refaudio-base-url" className="block text-xs text-muted-foreground mb-1">
                IrodoriTTS 参照音源モード ベースURL
              </label>
              <input
                id="irodori-refaudio-base-url"
                type="text"
                value={draft.tts.irodori_reference_audio_base_url ?? ''}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    tts: { ...draft.tts, irodori_reference_audio_base_url: e.target.value || undefined },
                  })
                }
                placeholder="http://localhost:8080"
                className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
              />
              <p className="mt-1 text-xs text-muted-foreground">
                キャラクターのIrodoriTTSモード選択に応じて使い分けられる
              </p>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label htmlFor="tts-max-chunk" className="block text-xs text-muted-foreground mb-1">
                  分割文字数
                </label>
                <input
                  id="tts-max-chunk"
                  type="number"
                  value={draft.tts.max_chunk_size ?? 140}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      tts: { ...draft.tts, max_chunk_size: Number(e.target.value) || 140 },
                    })
                  }
                  placeholder="140"
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                />
              </div>
              <div>
                <label htmlFor="tts-timeout" className="block text-xs text-muted-foreground mb-1">
                  タイムアウト（秒）
                </label>
                <input
                  id="tts-timeout"
                  type="number"
                  value={draft.tts.timeout_seconds ?? 60}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      tts: { ...draft.tts, timeout_seconds: Number(e.target.value) || 60 },
                    })
                  }
                  placeholder="60"
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                />
              </div>
            </div>
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

        {activeTab === 'tools' && (
          <div className="space-y-4">
            {/* Web検索設定（折り畳み可能） */}
            <details className="rounded-lg border border-border group">
              <summary className="flex items-center gap-2 px-4 py-3 cursor-pointer hover:bg-muted/40 transition-colors list-none">
                <ChevronRight className="w-4 h-4 text-muted-foreground transition-transform group-open:rotate-90" />
                <Globe className="w-4 h-4 text-muted-foreground" />
                <h3 className="text-sm font-medium">Web検索設定</h3>
              </summary>
              <div className="px-4 pb-4 space-y-3 border-t border-border/50">

              {/* プロバイダ選択 */}
              <div>
                <label htmlFor="web-search-provider" className="block text-xs text-muted-foreground mb-1">
                  検索プロバイダ
                </label>
                <select
                  id="web-search-provider"
                  value={webSearchProvider}
                  onChange={(e) => setWebSearchProvider(e.target.value as SearchProvider)}
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                >
                  <option value="tavily">Tavily</option>
                  <option value="brave">Brave Search</option>
                </select>
              </div>

              {/* API Key — Tavily */}
              <div>
                <label htmlFor="web-search-tavily-api-key" className="block text-xs text-muted-foreground mb-1">
                  Tavily API Key
                </label>
                <input
                  id="web-search-tavily-api-key"
                  type="password"
                  value={webSearchTavilyApiKey}
                  onChange={(e) => setWebSearchTavilyApiKey(e.target.value)}
                  placeholder="tvly-..."
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                />
              </div>

              {/* API Key — Brave */}
              <div>
                <label htmlFor="web-search-brave-api-key" className="block text-xs text-muted-foreground mb-1">
                  Brave Search API Key
                </label>
                <input
                  id="web-search-brave-api-key"
                  type="password"
                  value={webSearchBraveApiKey}
                  onChange={(e) => setWebSearchBraveApiKey(e.target.value)}
                  placeholder="BSA..."
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                />
              </div>

              {/* 許可ドメイン */}
              <div>
                <label htmlFor="web-search-domains" className="block text-xs text-muted-foreground mb-1">
                  許可ドメイン（fetch_page 用ホワイトリスト）
                </label>
                <textarea
                  id="web-search-domains"
                  value={webSearchAllowedDomains}
                  onChange={(e) => setWebSearchAllowedDomains(e.target.value)}
                  placeholder={"example.com\nen.wikipedia.org\ndocs.rs"}
                  rows={4}
                  className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary resize-y"
                />
                <p className="mt-1 text-xs text-muted-foreground">
                  1行に1ドメイン。fetch_page はここに記載されたドメインのみアクセス可能。
                </p>
              </div>

              {/* 保存ボタン */}
              <div className="flex justify-end">
                <button
                  onClick={handleSaveWebSearchConfig}
                  disabled={webSearchSaving}
                  className="px-3 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 flex items-center gap-1 transition-colors"
                >
                  {webSearchSaving ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <Save className="w-3.5 h-3.5" />
                  )}
                  Web検索設定を保存
                </button>
              </div>
              </div>
            </details>

            {/* Header with Add button */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Puzzle className="w-4 h-4 text-muted-foreground" />
                <h3 className="text-sm font-medium">登録済みプラグイン / ツール</h3>
              </div>
              <button
                onClick={() => setShowAddToolForm(true)}
                className="px-3 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:bg-primary/90 flex items-center gap-1 transition-colors"
              >
                <Plus className="w-3.5 h-3.5" />
                カスタムツール追加
              </button>
            </div>

            {/* Error */}
            {pluginsError && (
              <div className="p-3 rounded-md bg-destructive/10 text-destructive text-sm">
                {pluginsError}
              </div>
            )}

            {/* Add Custom Tool Form */}
            {showAddToolForm && (
              <div className="p-4 rounded-lg border border-primary/30 bg-primary/5 space-y-3">
                <h4 className="text-sm font-medium">カスタムツール追加</h4>
                <div>
                  <label htmlFor="new-tool-name" className="block text-xs text-muted-foreground mb-1">
                    ツール名
                  </label>
                  <input
                    id="new-tool-name"
                    type="text"
                    value={newToolName}
                    onChange={(e) => setNewToolName(e.target.value)}
                    placeholder="my_custom_tool"
                    className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                  />
                </div>
                <div>
                  <label htmlFor="new-tool-desc" className="block text-xs text-muted-foreground mb-1">
                    説明
                  </label>
                  <input
                    id="new-tool-desc"
                    type="text"
                    value={newToolDescription}
                    onChange={(e) => setNewToolDescription(e.target.value)}
                    placeholder="ツールの説明"
                    className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                  />
                </div>
                <div>
                  <label htmlFor="new-tool-type" className="block text-xs text-muted-foreground mb-1">
                    タイプ
                  </label>
                  <select
                    id="new-tool-type"
                    value={newToolType}
                    onChange={(e) => setNewToolType(e.target.value as CustomToolType)}
                    className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                  >
                    <option value="http">HTTP Webhook</option>
                    <option value="cli">CLI コマンド</option>
                  </select>
                </div>
                {newToolType === 'http' ? (
                  <div>
                    <label htmlFor="new-tool-endpoint" className="block text-xs text-muted-foreground mb-1">
                      エンドポイントURL
                    </label>
                    <input
                      id="new-tool-endpoint"
                      type="text"
                      value={newToolEndpoint}
                      onChange={(e) => setNewToolEndpoint(e.target.value)}
                      placeholder="https://api.example.com/tool"
                      className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                    />
                  </div>
                ) : (
                  <div>
                    <label htmlFor="new-tool-command" className="block text-xs text-muted-foreground mb-1">
                      コマンド
                    </label>
                    <input
                      id="new-tool-command"
                      type="text"
                      value={newToolCommand}
                      onChange={(e) => setNewToolCommand(e.target.value)}
                      placeholder="python /path/to/script.py"
                      className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
                    />
                  </div>
                )}
                <div className="flex gap-2 pt-1">
                  <button
                    onClick={handleAddCustomTool}
                    className="px-3 py-1.5 text-xs rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
                  >
                    追加
                  </button>
                  <button
                    onClick={() => setShowAddToolForm(false)}
                    className="px-3 py-1.5 text-xs rounded-md border border-border text-muted-foreground hover:text-foreground transition-colors"
                  >
                    キャンセル
                  </button>
                </div>
              </div>
            )}

            {/* Plugin List */}
            {pluginsLoading && plugins.length === 0 ? (
              <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
                <Loader2 className="w-4 h-4 animate-spin mr-2" />
                読み込み中...
              </div>
            ) : plugins.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-32 text-muted-foreground gap-2">
                <Puzzle className="w-8 h-8" />
                <p className="text-sm">プラグインが登録されていない</p>
              </div>
            ) : (
              <div className="space-y-2">
                {plugins.map((plugin) => {
                  const builtin = isBuiltinPlugin(plugin.name);
                  return (
                    <div
                      key={plugin.name}
                      className="p-3 rounded-lg border border-border bg-card flex items-center gap-3"
                    >
                      {/* Icon */}
                      <div className="w-8 h-8 rounded-md bg-muted flex items-center justify-center flex-shrink-0">
                        <Puzzle className="w-4 h-4 text-muted-foreground" />
                      </div>

                      {/* Info */}
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-sm truncate">{plugin.name}</span>
                          {builtin && (
                            <span className="px-1.5 py-0.5 text-[10px] rounded bg-blue-500/20 text-blue-600">
                              組み込み
                            </span>
                          )}
                          <span className="text-xs text-muted-foreground">v{plugin.version}</span>
                        </div>
                        <p className="text-xs text-muted-foreground truncate">{plugin.description}</p>
                      </div>

                      {/* Toggle */}
                      <label className="flex items-center gap-2 cursor-pointer flex-shrink-0">
                        <input
                          type="checkbox"
                          checked={plugin.enabled}
                          onChange={(e) => handleTogglePlugin(plugin.name, e.target.checked)}
                          className="rounded"
                        />
                        <span className="text-xs text-muted-foreground">
                          {plugin.enabled ? '有効' : '無効'}
                        </span>
                      </label>

                      {/* Delete button (custom plugins only) */}
                      {!builtin && (
                        <button
                          onClick={() => handleRemovePlugin(plugin.name)}
                          className="p-1.5 rounded-md text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors flex-shrink-0"
                          title="削除"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      )}
                    </div>
                  );
                })}
              </div>
            )}

            <p className="text-xs text-muted-foreground">
              組み込みプラグインは無効化のみ可能（削除不可）。カスタムツールは追加・削除が可能。
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
