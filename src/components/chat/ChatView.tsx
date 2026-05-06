import { useEffect, useRef, useCallback } from 'react';
import { MessageSquare } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useChatStore, useCharacterStore } from '../../stores';
import { MessageBubble } from './MessageBubble';
import { MessageInput } from './MessageInput';
import { StreamingIndicator } from './StreamingIndicator';
import { ChatHeaderControls } from './ChatHeaderControls';

/**
 * オートスクロール判定: 底から200px以内ならtrue
 */
export function shouldAutoScroll(scrollHeight: number, scrollTop: number, clientHeight: number): boolean {
  return (scrollHeight - scrollTop - clientHeight) <= 200;
}

export function ChatView() {
  const { currentSessionId, messages, isStreaming, isAbortable, streamingContent, error, sendMessage, createSession, fetchHistory, stopGeneration } =
    useChatStore();
  const { selectedCharacterId, characters } = useCharacterStore();
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const prevMessageCountRef = useRef(0);
  const isNearBottomRef = useRef(true);
  const rafIdRef = useRef<number | null>(null);

  // スクロールイベントでオートスクロール状態を更新
  const handleScroll = useCallback(() => {
    const container = scrollContainerRef.current;
    if (!container) return;
    isNearBottomRef.current = shouldAutoScroll(
      container.scrollHeight,
      container.scrollTop,
      container.clientHeight
    );
  }, []);

  // スクロールイベントリスナー登録
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;
    container.addEventListener('scroll', handleScroll, { passive: true });
    return () => container.removeEventListener('scroll', handleScroll);
  }, [handleScroll]);

  // requestAnimationFrame ベースのスムーズスクロール
  const smoothScrollToBottom = useCallback(() => {
    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current);
    }
    rafIdRef.current = requestAnimationFrame(() => {
      const container = scrollContainerRef.current;
      if (!container) return;
      container.scrollTo({
        top: container.scrollHeight,
        behavior: 'smooth',
      });
      rafIdRef.current = null;
    });
  }, []);

  // 即座にスクロール（セッション切り替え等）
  const scrollToBottomInstant = useCallback(() => {
    const container = scrollContainerRef.current;
    if (!container) return;
    container.scrollTop = container.scrollHeight;
    isNearBottomRef.current = true;
  }, []);

  // メッセージ数が変わった時（新メッセージ追加）→ スムーズに最下部へ
  useEffect(() => {
    if (messages.length !== prevMessageCountRef.current) {
      prevMessageCountRef.current = messages.length;
      if (isNearBottomRef.current) {
        smoothScrollToBottom();
      }
    }
  }, [messages.length, smoothScrollToBottom]);

  // ストリーミング中 → オートスクロール有効時のみスムーズ追従
  useEffect(() => {
    if (isStreaming && streamingContent && isNearBottomRef.current) {
      smoothScrollToBottom();
    }
  }, [streamingContent, isStreaming, smoothScrollToBottom]);

  // セッション切り替え時 → 即座に最下部へ
  useEffect(() => {
    // 少し遅延させてDOMレンダリング後にスクロール
    const timer = setTimeout(() => scrollToBottomInstant(), 50);
    return () => clearTimeout(timer);
  }, [currentSessionId, scrollToBottomInstant]);

  // クリーンアップ: 未処理のrAFをキャンセル
  useEffect(() => {
    return () => {
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
      }
    };
  }, []);

  // セッション未選択
  if (!currentSessionId) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground gap-4 p-6">
        <MessageSquare className="h-12 w-12" />
        {!selectedCharacterId ? (
          <p className="text-sm">サイドバーからキャラクターを選択してください</p>
        ) : (
          <>
            <p className="text-sm">
              {characters.find((c) => c.id === selectedCharacterId)?.name ?? 'キャラクター'}とチャットを開始
            </p>
            <button
              onClick={() => createSession(selectedCharacterId)}
              className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
            >
              新しいチャットを開始
            </button>
          </>
        )}
      </div>
    );
  }

  const handleSend = (content: string, isSystem?: boolean) => {
    if (isSystem) {
      sendMessage(`[SYSTEM] ${content}`);
    } else {
      sendMessage(content);
    }
  };

  const handleDeleteMessage = async (messageId: string) => {
    // メッセージ削除後に履歴を再取得
    try {
      await invoke('delete_message', { id: messageId });
      if (currentSessionId) {
        fetchHistory(currentSessionId);
      }
    } catch {
      // delete_messageコマンドが未実装の場合はローカルから削除
      useChatStore.setState((state) => ({
        messages: state.messages.filter((m) => m.id !== messageId),
      }));
    }
  };

  const handleRegenerateMessage = async (messageId: string) => {
    useChatStore.getState().regenerateMessage(messageId);
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Header controls */}
      <div className="flex items-center justify-end px-3 py-1.5 border-b border-border/50">
        <ChatHeaderControls />
      </div>

      {/* Messages area */}
      <div ref={scrollContainerRef} className="flex-1 overflow-y-auto overflow-x-visible py-4">
        {messages.length === 0 && !isStreaming && (
          <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
            メッセージを送信して会話を始めましょう
          </div>
        )}
        {messages.map((msg) => (
          <MessageBubble
            key={msg.id}
            message={msg}
            onRegenerate={handleRegenerateMessage}
            onDelete={handleDeleteMessage}
          />
        ))}
        {isStreaming && <StreamingIndicator content={streamingContent} />}
      </div>

      {/* Error display */}
      {error && (
        <div className="px-4 py-2 bg-destructive/10 border-t border-destructive/20 text-destructive text-sm">
          {error}
        </div>
      )}

      {/* Input area */}
      <MessageInput onSend={handleSend} disabled={isStreaming} isStreaming={isStreaming} isAbortable={isAbortable} onStop={stopGeneration} />
    </div>
  );
}
