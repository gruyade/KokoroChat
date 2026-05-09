import { useEffect, useRef, useState, useCallback } from 'react';
import { MessageSquare, UploadCloud } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useChatStore, useCharacterStore } from '../../stores';
import { MessageBubble } from './MessageBubble';
import { MessageInput, type MessageInputRef } from './MessageInput';
import { StreamingIndicator } from './StreamingIndicator';
import { ChatHeaderControls } from './ChatHeaderControls';

/**
 * オートスクロール判定: 底から200px以内ならtrue
 */
export function shouldAutoScroll(scrollHeight: number, scrollTop: number, clientHeight: number): boolean {
  return (scrollHeight - scrollTop - clientHeight) <= 200;
}

export function ChatView() {
  const { currentSessionId, messages, isStreaming, isAbortable, streamingContent, error, isTTSGenerating, sendMessage, createSession, fetchHistory, stopGeneration } =
    useChatStore();
  const { selectedCharacterId, characters } = useCharacterStore();
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const messageInputRef = useRef<MessageInputRef>(null);
  const prevMessageCountRef = useRef(0);
  const isNearBottomRef = useRef(true);
  const rafIdRef = useRef<number | null>(null);
  const isProgrammaticScrollRef = useRef(false);
  const lastSmoothScrollTimeRef = useRef(0);

  const [isDragOver, setIsDragOver] = useState(false);

  // Tauri drag-drop イベント: ファイルパスを直接取得
  useEffect(() => {
    const unlisten = listen<{ paths: string[] }>('tauri://drag-drop', async (event) => {
      setIsDragOver(false);
      for (const path of event.payload.paths) {
        await messageInputRef.current?.addAttachment(path);
      }
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // Tauri drag-over / drag-leave イベント: オーバーレイ制御
  useEffect(() => {
    const unlistenOver = listen('tauri://drag-over', () => setIsDragOver(true));
    const unlistenLeave = listen('tauri://drag-leave', () => setIsDragOver(false));
    return () => {
      unlistenOver.then(fn => fn());
      unlistenLeave.then(fn => fn());
    };
  }, []);

  // スクロールイベントでオートスクロール状態を更新
  // isProgrammaticScrollRef はscrollイベント経由の更新のみブロック（ユーザー操作イベントはブロックしない）
  const handleScroll = useCallback(() => {
    if (isProgrammaticScrollRef.current) return;
    const container = scrollContainerRef.current;
    if (!container) return;
    isNearBottomRef.current = shouldAutoScroll(
      container.scrollHeight,
      container.scrollTop,
      container.clientHeight
    );
  }, []);

  // wheel/touchmoveイベントリスナー: ユーザーが上方向にスクロールした場合、
  // isProgrammaticScrollRefに関係なく即座にisNearBottomRefをfalseに設定
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const handleWheel = (e: WheelEvent) => {
      // deltaY > 0 は下方向、deltaY < 0 は上方向
      if (e.deltaY < 0) {
        isNearBottomRef.current = false;
      } else if (e.deltaY > 0) {
        // 下方向スクロール時は現在位置で判定
        const nearBottom = shouldAutoScroll(
          container.scrollHeight,
          container.scrollTop,
          container.clientHeight
        );
        if (nearBottom) {
          isNearBottomRef.current = true;
        }
      }
    };

    let lastTouchY: number | null = null;
    const handleTouchStart = (e: TouchEvent) => {
      if (e.touches.length > 0) {
        lastTouchY = e.touches[0].clientY;
      }
    };
    const handleTouchMove = (e: TouchEvent) => {
      if (e.touches.length > 0 && lastTouchY !== null) {
        const currentY = e.touches[0].clientY;
        // タッチが下に動く = コンテンツが上にスクロール（ユーザーがスクロールアップ）
        if (currentY > lastTouchY) {
          isNearBottomRef.current = false;
        }
        lastTouchY = currentY;
      }
    };

    container.addEventListener('wheel', handleWheel, { passive: true });
    container.addEventListener('touchstart', handleTouchStart, { passive: true });
    container.addEventListener('touchmove', handleTouchMove, { passive: true });
    return () => {
      container.removeEventListener('wheel', handleWheel);
      container.removeEventListener('touchstart', handleTouchStart);
      container.removeEventListener('touchmove', handleTouchMove);
    };
    // currentSessionId を依存に含めることで、セッション選択後にcontainerが出現した際に再登録
  }, [currentSessionId]);

  // スクロールイベントリスナー登録
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;
    container.addEventListener('scroll', handleScroll, { passive: true });
    return () => container.removeEventListener('scroll', handleScroll);
  }, [handleScroll, currentSessionId]);

  // requestAnimationFrame ベースのスムーズスクロール（スロットリング付き）
  const smoothScrollToBottom = useCallback(() => {
    if (!isNearBottomRef.current) return;

    // スロットリング: 前回の呼び出しから150ms以内なら無視
    const now = Date.now();
    if (now - lastSmoothScrollTimeRef.current < 150) return;
    lastSmoothScrollTimeRef.current = now;

    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current);
    }
    isProgrammaticScrollRef.current = true;
    rafIdRef.current = requestAnimationFrame(() => {
      // rAF実行直前に再チェック: スケジュールから実行までの間にユーザーがスクロールアップした場合をキャッチ
      if (!isNearBottomRef.current) {
        isProgrammaticScrollRef.current = false;
        rafIdRef.current = null;
        return;
      }
      const container = scrollContainerRef.current;
      if (!container) return;
      container.scrollTo({
        top: container.scrollHeight,
        behavior: 'smooth',
      });
      // smooth scroll完了後にフラグ解除（300ms後）
      setTimeout(() => {
        isProgrammaticScrollRef.current = false;
      }, 300);
      rafIdRef.current = null;
    });
  }, []);

  // 即座にスクロール（セッション切り替え等）
  const scrollToBottomInstant = useCallback(() => {
    const container = scrollContainerRef.current;
    if (!container) return;
    isProgrammaticScrollRef.current = true;
    container.scrollTop = container.scrollHeight;
    isNearBottomRef.current = true;
    setTimeout(() => {
      isProgrammaticScrollRef.current = false;
    }, 50);
  }, []);

  // セッション切り替え時はスクロール状態とメッセージカウントをリセット
  useEffect(() => {
    isNearBottomRef.current = true;
    prevMessageCountRef.current = 0;
  }, [currentSessionId]);

  // メッセージロードおよび追加時のスクロール制御
  useEffect(() => {
    // メッセージがない場合は何もしない
    if (messages.length === 0) {
      prevMessageCountRef.current = 0;
      return;
    }

    // 履歴ロード時（前回0件からN件に増えた初回）は即座に最下部へ
    if (prevMessageCountRef.current === 0) {
      const timer = setTimeout(() => scrollToBottomInstant(), 50);
      prevMessageCountRef.current = messages.length;
      return () => clearTimeout(timer);
    }

    // 通常の新規メッセージ追加時はスムーズスクロール
    if (messages.length !== prevMessageCountRef.current) {
      prevMessageCountRef.current = messages.length;
      if (isNearBottomRef.current) {
        smoothScrollToBottom();
      }
    }
  }, [messages.length, smoothScrollToBottom, scrollToBottomInstant]);

  // ストリーミング中 → オートスクロール有効時のみスムーズ追従
  useEffect(() => {
    if (isStreaming && streamingContent && isNearBottomRef.current) {
      smoothScrollToBottom();
    }
  }, [streamingContent, isStreaming, smoothScrollToBottom]);

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

  const handleSend = (content: string, isSystem?: boolean, attachments?: import('../../types').Attachment[]) => {
    if (isSystem) {
      sendMessage(`[SYSTEM] ${content}`, attachments);
    } else {
      sendMessage(content, attachments);
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
    <div className="relative flex-1 flex flex-col overflow-hidden">
      {/* Drag-drop overlay */}
      {isDragOver && (
        <div className="absolute inset-0 bg-background/80 backdrop-blur-sm z-50 flex items-center justify-center pointer-events-none">
          <div className="flex flex-col items-center gap-2 text-muted-foreground">
            <UploadCloud className="h-12 w-12" />
            <span className="text-sm font-medium">ファイルをドロップして添付</span>
          </div>
        </div>
      )}

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
        {isTTSGenerating && (
          <div className="px-4 py-3 flex items-center gap-2 text-muted-foreground text-sm">
            <div className="flex gap-1">
              <span className="w-2 h-2 bg-primary rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
              <span className="w-2 h-2 bg-primary rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
              <span className="w-2 h-2 bg-primary rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
            </div>
            <span>音声を生成中...</span>
          </div>
        )}
      </div>

      {/* Error display */}
      {error && (
        <div className="px-4 py-2 bg-destructive/10 border-t border-destructive/20 text-destructive text-sm">
          {error}
        </div>
      )}

      {/* Input area */}
      <MessageInput
        ref={messageInputRef}
        onSend={handleSend}
        disabled={isStreaming || isTTSGenerating}
        isStreaming={isStreaming}
        isAbortable={isAbortable}
        onStop={stopGeneration}
        isDragOver={isDragOver}
      />
    </div>
  );
}
