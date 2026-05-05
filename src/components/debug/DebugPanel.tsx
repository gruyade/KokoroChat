import { useState, useEffect } from 'react';
import { Bug, Brain, Lightbulb, MessageSquare, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useChatStore, useCharacterStore, useConfigStore } from '../../stores';

export function DebugPanel() {
  const { currentSessionId, fetchHistory } = useChatStore();
  const { selectedCharacterId } = useCharacterStore();
  const { config, fetchConfig, updateConfig } = useConfigStore();
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const [timerDisplay, setTimerDisplay] = useState('—');

  // 1秒ごとにグローバルタイマー状態を読み取り
  useEffect(() => {
    if (!open) return;
    const id = setInterval(() => {
      const t = window.__spontaneousTimer;
      if (!t || !t.enabled) {
        setTimerDisplay('無効');
      } else if (t.checking) {
        setTimerDisplay('判定中...');
      } else if (t.remainingSeconds !== null) {
        setTimerDisplay(`${t.remainingSeconds}秒`);
      } else {
        setTimerDisplay('—');
      }
    }, 500);
    return () => clearInterval(id);
  }, [open]);

  useEffect(() => {
    if (open && !config) fetchConfig();
  }, [open, config, fetchConfig]);

  const probability = config?.spontaneous.probability ?? 0.3;

  const handleProbabilityChange = async (value: number) => {
    if (!config) return;
    const updated = {
      ...config,
      spontaneous: { ...config.spontaneous, probability: value },
    };
    await updateConfig(updated);
  };

  const handleCompressMemory = async () => {
    if (!currentSessionId) return;
    setLoading('memory');
    setResult(null);
    try {
      const summary = await invoke<string>('debug_compress_memory', { sessionId: currentSessionId });
      setResult(`記憶圧縮完了: ${summary.slice(0, 80)}...`);
    } catch (e) {
      setResult(`エラー: ${e}`);
    } finally {
      setLoading(null);
    }
  };

  const handleGenerateThought = async () => {
    if (!selectedCharacterId) return;
    setLoading('thought');
    setResult(null);
    try {
      const thought = await invoke<{ content: string }>('debug_generate_thought', {
        characterId: selectedCharacterId,
      });
      setResult(`思考生成: ${thought.content.slice(0, 100)}...`);
    } catch (e) {
      setResult(`エラー: ${e}`);
    } finally {
      setLoading(null);
    }
  };

  const handleTriggerSpontaneous = async () => {
    if (!currentSessionId) return;
    setLoading('spontaneous');
    setResult(null);
    try {
      const content = await invoke<string>('debug_trigger_spontaneous', {
        sessionId: currentSessionId,
      });
      setResult(`自発的発話: ${content.slice(0, 100)}...`);
      // チャット履歴を再取得して画面に反映
      await fetchHistory(currentSessionId);
    } catch (e) {
      setResult(`エラー: ${e}`);
    } finally {
      setLoading(null);
    }
  };

  return (
    <div className="fixed bottom-4 right-4 z-50">
      <button
        onClick={() => setOpen(!open)}
        className="p-2 rounded-full bg-destructive text-destructive-foreground shadow-lg hover:bg-destructive/90 transition-colors"
        title="デバッグパネル"
      >
        <Bug className="w-5 h-5" />
      </button>

      {open && (
        <div className="absolute bottom-12 right-0 w-72 rounded-lg border border-border bg-card shadow-xl p-3 space-y-2">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold flex items-center gap-1.5">
              <Bug className="w-4 h-4" />
              デバッグパネル
            </h3>
            <button onClick={() => setOpen(false)} className="text-muted-foreground hover:text-foreground">
              ×
            </button>
          </div>

          <div className="text-xs text-muted-foreground space-y-0.5">
            <div>キャラ: {selectedCharacterId?.slice(0, 8) ?? '未選択'}</div>
            <div>セッション: {currentSessionId?.slice(0, 8) ?? '未選択'}</div>
            <div>自発的発話: <span className="font-mono">{timerDisplay}</span></div>
          </div>

          <div className="space-y-1.5">
            <button
              onClick={handleCompressMemory}
              disabled={!currentSessionId || loading !== null}
              className="w-full flex items-center gap-2 px-3 py-1.5 text-xs rounded-md border border-border hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {loading === 'memory' ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Brain className="w-3.5 h-3.5" />}
              記憶圧縮を実行
            </button>

            <button
              onClick={handleGenerateThought}
              disabled={!selectedCharacterId || loading !== null}
              className="w-full flex items-center gap-2 px-3 py-1.5 text-xs rounded-md border border-border hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {loading === 'thought' ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Lightbulb className="w-3.5 h-3.5" />}
              思考生成を実行
            </button>

            <button
              onClick={handleTriggerSpontaneous}
              disabled={!currentSessionId || loading !== null}
              className="w-full flex items-center gap-2 px-3 py-1.5 text-xs rounded-md border border-border hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {loading === 'spontaneous' ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <MessageSquare className="w-3.5 h-3.5" />}
              自発的発話を実行
            </button>
          </div>

          {/* 自発的発話確率スライダー */}
          <div className="space-y-1 pt-2 border-t border-border">
            <div className="flex items-center justify-between">
              <span className="text-xs text-muted-foreground">自発的発話確率</span>
              <span className="text-xs font-mono">{Math.round(probability * 100)}%</span>
            </div>
            <input
              type="range"
              min="0"
              max="100"
              value={Math.round(probability * 100)}
              onChange={(e) => handleProbabilityChange(parseInt(e.target.value) / 100)}
              className="w-full h-1.5 rounded-full appearance-none cursor-pointer bg-muted"
            />
            <div className="flex justify-between text-xs text-muted-foreground/50">
              <span>0%</span>
              <span>100%</span>
            </div>
          </div>

          {result && (
            <div className="p-2 rounded-md bg-muted text-xs whitespace-pre-wrap break-all">
              {result}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
