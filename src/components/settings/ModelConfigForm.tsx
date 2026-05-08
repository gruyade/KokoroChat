import { useState, useRef, useEffect } from 'react';
import { Eye, EyeOff, TestTube2, Loader2, RefreshCw } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import type { ModelSettings, ModelPurpose, LLMProvider } from '../../types';
import { useConfigStore } from '../../stores';

/** プロバイダー設定定義 */
interface ProviderConfig {
  label: string;
  defaultBaseUrl?: string;
  supportsModelList: boolean;
}

const PROVIDERS: Record<LLMProvider, ProviderConfig> = {
  openai: { label: 'OpenAI', defaultBaseUrl: 'https://api.openai.com/v1', supportsModelList: true },
  anthropic: { label: 'Anthropic', defaultBaseUrl: 'https://api.anthropic.com/v1', supportsModelList: true },
  google: { label: 'Google', defaultBaseUrl: 'https://generativelanguage.googleapis.com/v1beta', supportsModelList: true },
  openai_compatible: { label: 'OpenAI互換', supportsModelList: true },
};

interface ModelConfigFormProps {
  purpose: ModelPurpose;
  label: string;
  settings: ModelSettings;
  onChange: (settings: ModelSettings) => void;
}

export function ModelConfigForm({ purpose, label, settings, onChange }: ModelConfigFormProps) {
  const { testLLMConnection } = useConfigStore();
  const [showApiKey, setShowApiKey] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);

  // モデル一覧取得関連
  const [fetchingModels, setFetchingModels] = useState(false);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [modelFetchError, setModelFetchError] = useState<string | null>(null);
  const [showModelDropdown, setShowModelDropdown] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // ドロップダウン外クリックで閉じる
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setShowModelDropdown(false);
      }
    };
    if (showModelDropdown) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [showModelDropdown]);

  const selectedProvider = settings.provider ?? null;
  const providerConfig = selectedProvider ? PROVIDERS[selectedProvider] : null;

  // Base URLが必須かどうか（OpenAI互換 or プロバイダー未選択時）
  const isBaseUrlRequired = selectedProvider === 'openai_compatible' || selectedProvider === null;

  // 実効Base URL（デフォルト値適用後）
  const effectiveBaseUrl = settings.base_url || providerConfig?.defaultBaseUrl || '';

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      await testLLMConnection(settings);
      setTestResult({ success: true, message: '接続成功' });
    } catch (e) {
      setTestResult({ success: false, message: String(e) });
    } finally {
      setTesting(false);
    }
  };

  const update = (field: keyof ModelSettings, value: string | number | LLMProvider | undefined) => {
    onChange({ ...settings, [field]: value });
  };

  const handleProviderChange = (provider: LLMProvider | '') => {
    if (provider === '') {
      // プロバイダー未選択に戻す
      const { provider: _removed, ...rest } = settings;
      onChange({ ...rest, provider: undefined });
    } else {
      const config = PROVIDERS[provider];
      onChange({
        ...settings,
        provider,
        // プロバイダー変更時にBase URLをクリア（デフォルト値が適用される）
        base_url: settings.base_url || '',
      });
      // モデル一覧をリセット
      setAvailableModels([]);
      setModelFetchError(null);
      void config; // suppress unused
    }
  };

  const handleFetchModels = async () => {
    setFetchingModels(true);
    setModelFetchError(null);
    setAvailableModels([]);
    try {
      const baseUrl = effectiveBaseUrl;
      const models = await invoke<string[]>('fetch_available_models', {
        baseUrl,
        apiKey: settings.api_key || null,
      });
      setAvailableModels(models);
    } catch (e) {
      setModelFetchError(String(e));
    } finally {
      setFetchingModels(false);
    }
  };

  const handleModelSelect = (model: string) => {
    update('model', model);
    setShowModelDropdown(false);
  };

  // モデル一覧取得ボタンの表示条件
  const canFetchModels = effectiveBaseUrl && settings.api_key;

  return (
    <div className="space-y-3 p-4 rounded-lg border border-border">
      <h3 className="text-sm font-medium">{label}</h3>

      {/* Provider Selection */}
      <div>
        <label htmlFor={`${purpose}-provider`} className="block text-xs text-muted-foreground mb-1">
          プロバイダー
        </label>
        <select
          id={`${purpose}-provider`}
          value={settings.provider ?? ''}
          onChange={(e) => handleProviderChange(e.target.value as LLMProvider | '')}
          className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
        >
          <option value="">選択してください</option>
          {Object.entries(PROVIDERS).map(([key, config]) => (
            <option key={key} value={key}>
              {config.label}
            </option>
          ))}
        </select>
      </div>

      {/* Base URL */}
      <div>
        <label htmlFor={`${purpose}-url`} className="block text-xs text-muted-foreground mb-1">
          Base URL
          {!isBaseUrlRequired && providerConfig?.defaultBaseUrl && (
            <span className="ml-1 text-muted-foreground/60">
              （未入力時: {providerConfig.defaultBaseUrl}）
            </span>
          )}
          {isBaseUrlRequired && (
            <span className="ml-1 text-destructive">*</span>
          )}
        </label>
        <input
          id={`${purpose}-url`}
          type="url"
          value={settings.base_url}
          onChange={(e) => update('base_url', e.target.value)}
          placeholder={providerConfig?.defaultBaseUrl ?? 'https://api.openai.com/v1'}
          className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
        />
      </div>

      {/* Model - with combo box behavior */}
      <div>
        <label htmlFor={`${purpose}-model`} className="block text-xs text-muted-foreground mb-1">
          モデル名
        </label>
        <div className="relative" ref={dropdownRef}>
          <input
            id={`${purpose}-model`}
            type="text"
            value={settings.model}
            onChange={(e) => update('model', e.target.value)}
            onFocus={() => availableModels.length > 0 && setShowModelDropdown(true)}
            placeholder="gpt-4o"
            className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
          />
          {/* Model dropdown */}
          {showModelDropdown && availableModels.length > 0 && (
            <div className="absolute z-50 w-full mt-1 max-h-48 overflow-y-auto rounded-md border border-border bg-background shadow-lg">
              {availableModels
                .filter((m) => m.toLowerCase().includes(settings.model.toLowerCase()))
                .map((model) => (
                  <button
                    key={model}
                    type="button"
                    onClick={() => handleModelSelect(model)}
                    className="w-full px-3 py-1.5 text-left text-sm hover:bg-muted transition-colors"
                  >
                    {model}
                  </button>
                ))}
            </div>
          )}
        </div>
        {/* Fetch models button */}
        {canFetchModels && (
          <div className="mt-1.5 flex items-center gap-2">
            <button
              type="button"
              onClick={handleFetchModels}
              disabled={fetchingModels}
              className="px-2.5 py-1 text-xs rounded-md border border-border hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1.5 transition-colors"
            >
              {fetchingModels ? (
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <RefreshCw className="w-3.5 h-3.5" />
              )}
              モデル一覧取得
            </button>
            {modelFetchError && (
              <span className="text-xs text-destructive truncate max-w-[200px]" title={modelFetchError}>
                {modelFetchError}
              </span>
            )}
            {availableModels.length > 0 && (
              <span className="text-xs text-muted-foreground">
                {availableModels.length}件取得
              </span>
            )}
          </div>
        )}
      </div>

      {/* API Key */}
      <div>
        <label htmlFor={`${purpose}-key`} className="block text-xs text-muted-foreground mb-1">
          API Key
        </label>
        <div className="relative">
          <input
            id={`${purpose}-key`}
            type={showApiKey ? 'text' : 'password'}
            value={settings.api_key ?? ''}
            onChange={(e) => update('api_key', e.target.value)}
            placeholder="sk-..."
            className="w-full px-3 py-1.5 pr-10 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
          />
          <button
            type="button"
            onClick={() => setShowApiKey(!showApiKey)}
            className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            aria-label={showApiKey ? 'APIキーを隠す' : 'APIキーを表示'}
          >
            {showApiKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
          </button>
        </div>
      </div>

      {/* Temperature */}
      <div>
        <label htmlFor={`${purpose}-temp`} className="block text-xs text-muted-foreground mb-1">
          Temperature: {settings.temperature.toFixed(2)}
        </label>
        <input
          id={`${purpose}-temp`}
          type="range"
          min="0"
          max="2"
          step="0.05"
          value={settings.temperature}
          onChange={(e) => update('temperature', parseFloat(e.target.value))}
          className="w-full"
        />
      </div>

      {/* Test Connection */}
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={handleTest}
          disabled={testing || !effectiveBaseUrl || !settings.model}
          className="px-3 py-1.5 text-xs rounded-md border border-border hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1.5 transition-colors"
        >
          {testing ? (
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
          ) : (
            <TestTube2 className="w-3.5 h-3.5" />
          )}
          接続テスト
        </button>
        {testResult && (
          <span
            className={`text-xs ${testResult.success ? 'text-green-500' : 'text-destructive'}`}
          >
            {testResult.message}
          </span>
        )}
      </div>
    </div>
  );
}
