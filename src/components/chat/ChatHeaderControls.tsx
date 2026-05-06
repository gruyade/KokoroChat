import { useState, useCallback } from 'react';
import { Brain, MessageSquare } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

export function ChatHeaderControls() {
  const [thoughtPaused, setThoughtPaused] = useState(false);
  const [spontaneousPaused, setSpontaneousPaused] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
      // 失敗時: トグル状態を元に戻す
      setThoughtPaused(!newState);
      setError(String(e));
      setTimeout(() => setError(null), 3000);
    }
  }, [thoughtPaused]);

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
      // 失敗時: トグル状態を元に戻す
      setSpontaneousPaused(!newState);
      setError(String(e));
      setTimeout(() => setError(null), 3000);
    }
  }, [spontaneousPaused]);

  return (
    <div className="flex items-center gap-1">
      {/* 思考生成トグル */}
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

      {/* 自発的発話トグル */}
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

      {/* エラー表示 */}
      {error && (
        <span className="text-xs text-destructive ml-1 max-w-[150px] truncate">
          {error}
        </span>
      )}
    </div>
  );
}
