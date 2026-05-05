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
  const appendStreamChunk = useChatStore((s) => s.appendStreamChunk);
  const finishStreaming = useChatStore((s) => s.finishStreaming);
  const streamingContent = useChatStore((s) => s.streamingContent);

  useEffect(() => {
    let streamContent = '';

    const setupListeners = async () => {
      const unlistenStream = await listen<ChatStreamEvent>('chat:stream', (event) => {
        const { chunk, done } = event.payload;
        if (done) {
          finishStreaming(streamContent);
          streamContent = '';
        } else {
          streamContent += chunk;
          appendStreamChunk(chunk);
        }
      });

      const unlistenSpontaneous = await listen<SpontaneousEvent>(
        'spontaneous:message',
        (event) => {
          // 自発的発話を完了メッセージとして処理
          finishStreaming(event.payload.message);
        }
      );

      const unlistenTool = await listen<ToolExecutingEvent>('tool:executing', () => {
        // ツール実行中の状態はストリーミング中として扱う
        // UI側でStreamingIndicatorやToolCallIndicatorが表示される
      });

      return () => {
        unlistenStream();
        unlistenSpontaneous();
        unlistenTool();
      };
    };

    let cleanup: (() => void) | undefined;
    setupListeners().then((fn) => {
      cleanup = fn;
    });

    return () => {
      cleanup?.();
    };
  }, [appendStreamChunk, finishStreaming]);

  return { sendMessage, isStreaming, streamingContent };
}
