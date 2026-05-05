import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useChatStore } from '../stores/chat.store';

/** chat:stream イベントペイロード */
interface ChatStreamEvent {
  session_id: string;
  chunk: string;
  done: boolean;
}

/** spontaneous:message イベントペイロード */
interface SpontaneousEvent {
  session_id: string;
  message: string;
}

/** tool:executing イベントペイロード */
interface ToolExecutingEvent {
  session_id: string;
  tool_name: string;
}

/**
 * チャット操作Hook
 * - chat:stream イベントリスナー（ストリーミング受信）
 * - spontaneous:message イベントリスナー（自発的発話受信）
 * - tool:executing イベントリスナー（ツール実行通知）
 */
export function useChat() {
  const sendMessage = useChatStore((s) => s.sendMessage);
  const isStreaming = useChatStore((s) => s.isStreaming);
  const streamingContent = useChatStore((s) => s.streamingContent);

  useEffect(() => {
    let streamContent = '';
    let cancelled = false;

    const unlisteners: (() => void)[] = [];

    const setupListeners = async () => {
      if (cancelled) return;

      const unlistenStream = await listen<ChatStreamEvent>('chat:stream', (event) => {
        if (cancelled) return;
        const { chunk, done } = event.payload;
        if (done) {
          useChatStore.getState().finishStreaming(streamContent);
          streamContent = '';
        } else {
          streamContent += chunk;
          useChatStore.getState().appendStreamChunk(chunk);
        }
      });
      if (cancelled) { unlistenStream(); return; }
      unlisteners.push(unlistenStream);

      const unlistenSpontaneous = await listen<SpontaneousEvent>(
        'spontaneous:message',
        (event) => {
          if (cancelled) return;
          // 自発的発話受信 → チャット履歴を再取得して反映
          const { currentSessionId, fetchHistory } = useChatStore.getState();
          if (currentSessionId && event.payload.session_id === currentSessionId) {
            fetchHistory(currentSessionId);
          }
        }
      );
      if (cancelled) { unlistenSpontaneous(); return; }
      unlisteners.push(unlistenSpontaneous);

      const unlistenTool = await listen<ToolExecutingEvent>('tool:executing', () => {});
      if (cancelled) { unlistenTool(); return; }
      unlisteners.push(unlistenTool);
    };

    setupListeners();

    return () => {
      cancelled = true;
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  return { sendMessage, isStreaming, streamingContent };
}
