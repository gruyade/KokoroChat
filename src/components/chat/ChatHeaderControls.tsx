import { useState, useCallback } from 'react';
import { Brain, MessageSquare, Volume2, VolumeX, Sparkles, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { create } from 'zustand';
import { useAudioStore } from '../../hooks/useAudio';
import { useChatStore } from '../../stores/chat.store';
import { useConfigStore } from '../../stores/config.store';
import { useUIStore } from '../../stores/ui.store';

// 一時停止状態をZustandストアで管理（画面切り替えでもリセットされない）
interface PauseState {
  thoughtPaused: boolean;
  spontaneousPaused: boolean;
  setThoughtPaused: (paused: boolean) => void;
  setSpontaneousPaused: (paused: boolean) => void;
}

export const usePauseStore = create<PauseState>((set) => ({
  thoughtPaused: false,
  spontaneousPaused: false,
  setThoughtPaused: (paused) => set({ thoughtPaused: paused }),
  setSpontaneousPaused: (paused) => set({ spontaneousPaused: paused }),
}));

export function ChatHeaderControls() {
  const { thoughtPaused, spontaneousPaused, setThoughtPaused, setSpontaneousPaused } = usePauseStore();
  const { volume, setVolume } = useAudioStore();
  const { config } = useConfigStore();
  const currentSessionId = useChatStore((s) => s.currentSessionId);
  const [error, setError] = useState<string | null>(null);
  const [memoryGenerating, setMemoryGenerating] = useState(false);

  const ttsEnabled = config?.tts?.enabled ?? false;
  const thoughtEnabled = config?.thought?.enabled ?? false;
  const spontaneousEnabled = config?.spontaneous?.enabled ?? false;

  const handleGenerateMemory = useCallback(async () => {
    const sessionId = useChatStore.getState().currentSessionId;
    if (!sessionId || memoryGenerating) return;
    setMemoryGenerating(true);
    try {
      await invoke('generate_memory_manual', { sessionId });
      useUIStore.getState().showToast('記憶を生成した');
    } catch (e) {
      useUIStore.getState().showToast(`記憶生成に失敗: ${e}`, 'error');
    } finally {
      setMemoryGenerating(false);
    }
  }, [memoryGenerating]);

  const toggleThought = useCallback(async () => {
    const newState = !thoughtPaused;
    setThoughtPaused(newState);
    setError(null);
    try {
      if (newState) {
        await invoke('pause_thought_engine');
      } else {
        await invoke('resume_thought_engine');
      }
    } catch (e) {
      setThoughtPaused(!newState);
      setError(String(e));
      setTimeout(() => setError(null), 3000);
    }
  }, [thoughtPaused, setThoughtPaused]);

  const toggleSpontaneous = useCallback(async () => {
    const newState = !spontaneousPaused;
    setSpontaneousPaused(newState);
    setError(null);
    try {
      if (newState) {
        await invoke('pause_spontaneous');
      } else {
        await invoke('resume_spontaneous');
      }
    } catch (e) {
      setSpontaneousPaused(!newState);
      setError(String(e));
      setTimeout(() => setError(null), 3000);
    }
  }, [spontaneousPaused, setSpontaneousPaused]);

  return (
    <div className="flex items-center gap-1">
      {/* ボリュームコントロール — TTS有効時のみ表示 */}
      {ttsEnabled && (
        <div className="flex items-center gap-1 mr-2">
          <button
            onClick={() => setVolume(volume > 0 ? 0 : 1)}
            title={volume > 0 ? 'ミュート' : 'ミュート解除'}
            className="p-1.5 rounded-md transition-colors text-foreground hover:bg-muted/50"
          >
            {volume > 0 ? <Volume2 className="h-4 w-4" /> : <VolumeX className="h-4 w-4" />}
          </button>
          <input
            type="range"
            min={0}
            max={1}
            step={0.05}
            value={volume}
            onChange={(e) => setVolume(parseFloat(e.target.value))}
            className="w-16 h-1.5 rounded-lg appearance-none bg-muted cursor-pointer"
            title={`音量: ${Math.round(volume * 100)}%`}
          />
        </div>
      )}

      {/* メモリ生成ボタン */}
      <button
        onClick={handleGenerateMemory}
        disabled={!currentSessionId || memoryGenerating}
        title="記憶を手動生成"
        className={`p-1.5 rounded-md transition-colors ${
          !currentSessionId || memoryGenerating
            ? 'text-muted-foreground opacity-50 cursor-not-allowed'
            : 'text-foreground hover:bg-muted/50'
        }`}
      >
        {memoryGenerating ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <Sparkles className="h-4 w-4" />
        )}
      </button>

      {/* 思考生成トグル */}
      {thoughtEnabled && (
        <button
          onClick={toggleThought}
          title={thoughtPaused ? '思考生成を再開' : '思考生成を一時停止'}
          className={`p-1.5 rounded-md transition-colors ${
            thoughtPaused
              ? 'text-muted-foreground bg-muted/50 opacity-50'
              : 'text-foreground hover:bg-muted/50'
          }`}
        >
          <Brain className="h-4 w-4" />
        </button>
      )}

      {/* 自発的発話トグル */}
      {spontaneousEnabled && (
        <button
          onClick={toggleSpontaneous}
          title={spontaneousPaused ? '自発的発話を再開' : '自発的発話を一時停止'}
          className={`p-1.5 rounded-md transition-colors ${
            spontaneousPaused
              ? 'text-muted-foreground bg-muted/50 opacity-50'
              : 'text-foreground hover:bg-muted/50'
          }`}
        >
          <MessageSquare className="h-4 w-4" />
        </button>
      )}

      {/* エラー表示 */}
      {error && (
        <span className="text-xs text-destructive ml-1 max-w-[150px] truncate">
          {error}
        </span>
      )}
    </div>
  );
}
