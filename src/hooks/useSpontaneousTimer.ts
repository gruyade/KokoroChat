/**
 * 自発的発話タイマー
 * 
 * グローバル変数でタイマー状態を管理。
 * window.__spontaneousTimer に状態を書き込み、どこからでも参照可能。
 */
import { invoke } from '@tauri-apps/api/core';
import { useEffect, useRef } from 'react';
import { useChatStore, useConfigStore } from '../stores';

// グローバル状態（windowに公開）
declare global {
  interface Window {
    __spontaneousTimer: {
      remainingSeconds: number | null;
      checking: boolean;
      enabled: boolean;
      targetTime: number;
    };
  }
}

window.__spontaneousTimer = {
  remainingSeconds: null,
  checking: false,
  enabled: false,
  targetTime: 0,
};

let globalIntervalId: ReturnType<typeof setInterval> | null = null;
let globalChecking = false;

function stopGlobalInterval() {
  if (globalIntervalId !== null) {
    clearInterval(globalIntervalId);
    globalIntervalId = null;
  }
}

function resetGlobalTimer(intervalSec: number) {
  window.__spontaneousTimer.targetTime = Date.now() + intervalSec * 1000;
  window.__spontaneousTimer.remainingSeconds = intervalSec;
}

async function fireCheck(sessionId: string) {
  if (globalChecking) return;
  globalChecking = true;
  window.__spontaneousTimer.checking = true;
  try {
    const triggered = await invoke<boolean>('trigger_spontaneous_check', { sessionId });
    console.debug('[spontaneous] check result:', triggered);
  } catch (e) {
    console.error('[spontaneous] invoke error:', e);
  } finally {
    globalChecking = false;
    window.__spontaneousTimer.checking = false;
  }
}

/**
 * App.tsxで1回だけ呼ぶ。タイマーを起動・管理する。
 */
export function useSpontaneousTimer() {
  const currentSessionId = useChatStore((s) => s.currentSessionId);
  const messagesLength = useChatStore((s) => s.messages.length);
  const config = useConfigStore((s) => s.config);

  const enabled = config?.spontaneous.enabled ?? false;
  const intervalSec = config?.spontaneous.min_interval_seconds ?? 60;

  const sessionIdRef = useRef(currentSessionId);
  sessionIdRef.current = currentSessionId;

  const intervalSecRef = useRef(intervalSec);
  intervalSecRef.current = intervalSec;

  // 有効/無効の反映
  window.__spontaneousTimer.enabled = enabled;

  // メッセージ追加 or セッション変更でタイマーリセット
  useEffect(() => {
    if (enabled && currentSessionId) {
      resetGlobalTimer(intervalSec);
    }
  }, [messagesLength, currentSessionId, enabled, intervalSec]);

  // メインのインターバル管理
  useEffect(() => {
    stopGlobalInterval();

    if (!enabled || !currentSessionId) {
      window.__spontaneousTimer.remainingSeconds = null;
      window.__spontaneousTimer.enabled = false;
      return;
    }

    window.__spontaneousTimer.enabled = true;
    if (window.__spontaneousTimer.targetTime === 0) {
      resetGlobalTimer(intervalSec);
    }

    globalIntervalId = setInterval(async () => {
      const now = Date.now();
      const target = window.__spontaneousTimer.targetTime;
      const remaining = Math.max(0, Math.ceil((target - now) / 1000));
      window.__spontaneousTimer.remainingSeconds = remaining;

      if (remaining <= 0 && !globalChecking) {
        const sid = sessionIdRef.current;
        if (sid) {
          await fireCheck(sid);
          // 判定後にタイマーリセット
          resetGlobalTimer(intervalSecRef.current);
        }
      }
    }, 1000);

    return () => stopGlobalInterval();
  }, [enabled, currentSessionId, intervalSec]);
}
