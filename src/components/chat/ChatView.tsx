import { useEffect, useRef, useCallback } from 'react';
import { MessageSquare } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useChatStore, useCharacterStore } from '../../stores';
import { MessageBubble } from './MessageBubble';
import { MessageInput } from './MessageInput';
import { StreamingIndicator } from './StreamingIndicator';
import { ChatHeaderControls } from './ChatHeaderControls';

export function ChatView() {
  const { currentSessionId, messages, isStreaming, isAbortable, streamingContent, error, sendMessage, createSession, fetchHistory, stopGeneration } =
    useChatStore();
  const { selectedCharacterId, characters } = useCharacterStore();
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const prevMessageCountRef = useRef(0);

  // スクロールを最下部に移動（即座に）
  const scrollToBottom = useCallback((instant = true) => {
    const container = scrollContainerRef.current;
    if (!container) return;
    if (instant) {
      container.scrollTop = container.scrollHeight;
    } else {
      // 差分が小さい場合のみスムーズ（ストリーミング中のチャンク追加）
      const distanceFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight;
      if (distanceFromBottom < 200) {
        container.scrollTop = container.scrollHeight;
      }
    }
  }, []);

  // メッセージ数が変わった時（新メッセージ追加）→ 即座に最下部へ
  useEffect(() => {
    if (messages.length !== prevMessageCountRef.current) {
      prevMessageCountRef.current = messages.length;
      scrollToBottom(true);
    }
  }, [messages.length, scrollToBottom]);

  // ストリーミング中 → 下部付近にいる場合のみ追従
  useEffect(() => {
    if (isStreaming && streamingContent) {
      scrollToBottom(false);
    }
  }, [streamingContent, isStreaming, scrollToBottom]);

  // セッション切り替え時 → 即座に最下部へ
  useEffect(() => {
    // 少し遅延させてDOMレンダリング後にスクロール
    const timer = setTimeout(() => scrollToBottom(true), 50);
    return () => clearTimeout(timer);
  }, [currentSessionId, scrollToBottom]);

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
      <div ref={scrollContainerRef} className="flex-1 overflow-y-auto py-4">
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
      <MessageInput onSend={handleSend} disabled={isStreaming} isAbortable={isAbortable} onStop={stopGeneration} />
    </div>
  );
}
