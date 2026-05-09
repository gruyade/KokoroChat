import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useChatStore } from '../stores/chat.store';
import type { TTSCompleteEvent, TTSGeneratingEvent, TTSErrorEvent } from '../types';

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
  const isTTSGenerating = useChatStore((s) => s.isTTSGenerating);

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
          // ストリーミング完了後にDBから履歴を再取得し、正しいメッセージIDを同期
          const { currentSessionId, fetchHistory } = useChatStore.getState();
          if (currentSessionId) {
            fetchHistory(currentSessionId);
          }
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

      const unlistenTTSGenerating = await listen<TTSGeneratingEvent>('tts:generating', (_event) => {
        if (cancelled) return;
        console.log('[TTS] Received tts:generating event');
        useChatStore.getState().setTTSGenerating(true);
      });
      if (cancelled) { unlistenTTSGenerating(); return; }
      unlisteners.push(unlistenTTSGenerating);

      const unlistenTTSComplete = await listen<TTSCompleteEvent>('tts:complete', (event) => {
        if (cancelled) return;
        const { session_id, text, audio } = event.payload;
        console.log('[TTS] Received tts:complete event, audio length:', audio?.length ?? 0);
        // session_idが空の場合はボタン経由の再生（ストア更新不要）
        if (session_id) {
          useChatStore.getState().finishWithAudio(text, audio);
          const { currentSessionId, fetchHistory } = useChatStore.getState();
          if (currentSessionId) {
            fetchHistory(currentSessionId);
          }
        }
      });
      if (cancelled) { unlistenTTSComplete(); return; }
      unlisteners.push(unlistenTTSComplete);

      const unlistenTTSError = await listen<TTSErrorEvent>('tts:error', (event) => {
        if (cancelled) return;
        const { text, error } = event.payload;
        console.log('[TTS] Received tts:error event:', error);
        // テキストは即座に表示（finishWithAudioと同じ処理だが音声なし）
        useChatStore.getState().finishWithAudio(text, '');
        // 完了後にDBから履歴を再取得
        const { currentSessionId, fetchHistory } = useChatStore.getState();
        if (currentSessionId) {
          fetchHistory(currentSessionId);
        }
      });
      if (cancelled) { unlistenTTSError(); return; }
      unlisteners.push(unlistenTTSError);
    };

    setupListeners();

    return () => {
      cancelled = true;
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  return { sendMessage, isStreaming, streamingContent, isTTSGenerating };
}
