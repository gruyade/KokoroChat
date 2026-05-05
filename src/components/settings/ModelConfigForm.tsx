import { useState } from 'react';
import { Eye, EyeOff, TestTube2, Loader2 } from 'lucide-react';
import type { ModelSettings, ModelPurpose } from '../../types';
import { useConfigStore } from '../../stores';

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

  const update = (field: keyof ModelSettings, value: string | number) => {
    onChange({ ...settings, [field]: value });
  };

  return (
    <div className="space-y-3 p-4 rounded-lg border border-border">
      <h3 className="text-sm font-medium">{label}</h3>

      {/* Base URL */}
      <div>
        <label htmlFor={`${purpose}-url`} className="block text-xs text-muted-foreground mb-1">
          Base URL
        </label>
        <input
          id={`${purpose}-url`}
          type="url"
          value={settings.base_url}
          onChange={(e) => update('base_url', e.target.value)}
          placeholder="https://api.openai.com/v1"
          className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
        />
      </div>

      {/* Model */}
      <div>
        <label htmlFor={`${purpose}-model`} className="block text-xs text-muted-foreground mb-1">
          モデル名
        </label>
        <input
          id={`${purpose}-model`}
          type="text"
          value={settings.model}
          onChange={(e) => update('model', e.target.value)}
          placeholder="gpt-4o"
          className="w-full px-3 py-1.5 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
        />
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
          disabled={testing || !settings.base_url || !settings.model}
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
