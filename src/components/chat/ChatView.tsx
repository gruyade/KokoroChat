import { useEffect, useRef, useState, useCallback } from 'react';
import { MessageSquare, Wrench, Shield } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useChatStore, useCharacterStore, useKnowledgeStore } from '../../stores';
import { useUIStore } from '../../stores/ui.store';
import { MessageBubble } from './MessageBubble';
import { MessageInput, type MessageInputRef } from './MessageInput';
import { StreamingIndicator } from './StreamingIndicator';
import { ChatHeaderControls } from './ChatHeaderControls';
import { ToolManagementPane } from './ToolManagementPane';
import { ToolCallIndicator } from './ToolCallIndicator';

/** file_ops アクセス許可リクエストのイベントペイロード */
interface FileOpsAccessRequestPayload {
  session_id: string;
  request_id: string;
  path: string;
  requires_write: boolean;
}

/** 保留中のアクセス許可リクエスト */
interface PendingAccessRequest {
  requestId: string;
  path: string;
  requiresWrite: boolean;
}

/**
 * オートスクロール判定: 底から200px以内ならtrue
 */
export function shouldAutoScroll(scrollHeight: number, scrollTop: number, clientHeight: number): boolean {
  return (scrollHeight - scrollTop - clientHeight) <= 200;
}

/** position が指定 DOM 要素の BoundingRect 内にあるか判定 */
function isPositionInElement(el: Element, position: { x: number; y: number }): boolean {
  const rect = el.getBoundingClientRect();
  return (
    position.x >= rect.left && position.x <= rect.right &&
    position.y >= rect.top && position.y <= rect.bottom
  );
}

export function ChatView() {
  const { currentSessionId, messages, isStreaming, isAbortable, streamingContent, isThinking, error, isTTSGenerating, executingToolName, sendMessage, createSession, fetchHistory, stopGeneration } =
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
  const [dragTarget, setDragTarget] = useState<'chat' | 'knowledge' | null>(null);
  const [toolPaneOpen, setToolPaneOpen] = useState(false);
  const [pendingAccessRequests, setPendingAccessRequests] = useState<PendingAccessRequest[]>([]);
  const [paneWidth, setPaneWidth] = useState(320);
  const isResizingRef = useRef(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const addKnowledge = useKnowledgeStore((s) => s.addKnowledge);
  const showToast = useUIStore((s) => s.showToast);

  /** ドロップ位置がどのターゲット要素内にあるか判定 */
  const resolveDropTarget = useCallback((position: { x: number; y: number }): 'chat' | 'knowledge' | null => {
    // ナレッジ DropZone 判定（data-drop-target="knowledge" 属性で特定）
    const knowledgeDropZone = document.querySelector('[data-drop-target="knowledge"]');
    if (knowledgeDropZone && isPositionInElement(knowledgeDropZone, position)) {
      return 'knowledge';
    }
    // チャット入力エリア判定（data-drop-target="chat" 属性で特定）
    const chatInput = document.querySelector('[data-drop-target="chat"]');
    if (chatInput && isPositionInElement(chatInput, position)) {
      return 'chat';
    }
    return null;
  }, []);

  // Tauri drag-drop イベント: ドロップ位置に応じてルーティング
  useEffect(() => {
    const unlisten = listen<{ paths: string[]; position: { x: number; y: number } }>('tauri://drag-drop', async (event) => {
      setIsDragOver(false);
      setDragTarget(null);

      const { paths, position } = event.payload;
      const target = resolveDropTarget(position);

      if (target === 'knowledge') {
        const sessionId = useChatStore.getState().currentSessionId;
        if (!sessionId) return;
        for (const filePath of paths) {
          try {
            const content = await invoke<string>('read_text_file_for_knowledge', { filePath });
            const fileName = filePath.split(/[\\/]/).pop() ?? filePath;
            await addKnowledge(sessionId, fileName, content);
          } catch (err) {
            const fileName = filePath.split(/[\\/]/).pop() ?? filePath;
            showToast(`${fileName}: ${err instanceof Error ? err.message : String(err)}`, 'error');
          }
        }
      } else if (target === 'chat') {
        for (const path of paths) {
          await messageInputRef.current?.addAttachment(path);
        }
      }
      // target === null → どちらのエリアでもないので何もしない
    });
    return () => { unlisten.then(fn => fn()); };
  }, [resolveDropTarget, addKnowledge, showToast]);

  // Tauri drag-over / drag-leave イベント: ドロップ先に応じたハイライト表示
  useEffect(() => {
    const unlistenOver = listen<{ position: { x: number; y: number } }>('tauri://drag-over', (event) => {
      const pos = event.payload.position;
      if (pos) {
        const target = resolveDropTarget(pos);
        setDragTarget(target);
        setIsDragOver(target !== null);
      } else {
        setIsDragOver(false);
        setDragTarget(null);
      }
    });
    const unlistenLeave = listen('tauri://drag-leave', () => {
      setIsDragOver(false);
      setDragTarget(null);
    });
    return () => {
      unlistenOver.then(fn => fn());
      unlistenLeave.then(fn => fn());
    };
  }, [resolveDropTarget]);

  // file_ops:request_access イベントリスナー（常時マウント）
  useEffect(() => {
    const unlisten = listen<FileOpsAccessRequestPayload>('file_ops:request_access', (event) => {
      const { request_id, path, requires_write } = event.payload;
      setPendingAccessRequests((prev) => [
        ...prev,
        { requestId: request_id, path, requiresWrite: requires_write },
      ]);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // アクセス許可/拒否を解決
  const resolveAccessRequest = useCallback(async (requestId: string, granted: boolean) => {
    try {
      await invoke('resolve_file_ops_access', { requestId, granted });
    } catch {
      // エラー時も UI から除去
    }
    setPendingAccessRequests((prev) => prev.filter((r) => r.requestId !== requestId));
  }, []);

  // --- リサイズハンドル用イベントハンドラ ---
  const handleResizeMove = useCallback((e: MouseEvent) => {
    if (!isResizingRef.current) return;
    const container = containerRef.current;
    if (!container) return;
    const containerRect = container.getBoundingClientRect();
    const newWidth = containerRect.right - e.clientX;
    setPaneWidth(Math.max(200, Math.min(600, newWidth)));
  }, []);

  const handleResizeEnd = useCallback(() => {
    isResizingRef.current = false;
    document.removeEventListener('mousemove', handleResizeMove);
    document.removeEventListener('mouseup', handleResizeEnd);
  }, [handleResizeMove]);

  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    isResizingRef.current = true;
    document.addEventListener('mousemove', handleResizeMove);
    document.addEventListener('mouseup', handleResizeEnd);
  }, [handleResizeMove, handleResizeEnd]);

  // スクロールイベントでオートスクロール状態を更新
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

  // wheel/touchmoveイベントリスナー
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const handleWheel = (e: WheelEvent) => {
      if (e.deltaY < 0) {
        isNearBottomRef.current = false;
      } else if (e.deltaY > 0) {
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

    const now = Date.now();
    if (now - lastSmoothScrollTimeRef.current < 150) return;
    lastSmoothScrollTimeRef.current = now;

    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current);
    }
    isProgrammaticScrollRef.current = true;
    rafIdRef.current = requestAnimationFrame(() => {
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
    if (messages.length === 0) {
      prevMessageCountRef.current = 0;
      return;
    }

    if (prevMessageCountRef.current === 0) {
      const timer = setTimeout(() => scrollToBottomInstant(), 50);
      prevMessageCountRef.current = messages.length;
      return () => clearTimeout(timer);
    }

    if (messages.length !== prevMessageCountRef.current) {
      prevMessageCountRef.current = messages.length;
      if (isNearBottomRef.current) {
        smoothScrollToBottom();
      }
    }
  }, [messages.length, smoothScrollToBottom, scrollToBottomInstant]);

  // ストリーミング中のオートスクロール
  useEffect(() => {
    if (isStreaming && streamingContent && isNearBottomRef.current) {
      smoothScrollToBottom();
    }
  }, [streamingContent, isStreaming, smoothScrollToBottom]);

  // クリーンアップ
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
    try {
      await invoke('delete_message', { id: messageId });
      if (currentSessionId) {
        fetchHistory(currentSessionId);
      }
    } catch {
      useChatStore.setState((state) => ({
        messages: state.messages.filter((m) => m.id !== messageId),
      }));
    }
  };

  const handleRegenerateMessage = async (messageId: string) => {
    useChatStore.getState().regenerateMessage(messageId);
  };

  return (
    <div ref={containerRef} className="relative flex-1 flex overflow-hidden">
      {/* Main chat area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header controls */}
        <div className="flex items-center justify-end px-3 py-1.5 border-b border-border/50">
          <ChatHeaderControls />
          <button
            onClick={() => setToolPaneOpen((v) => !v)}
            title={toolPaneOpen ? 'ツール管理を閉じる' : 'ツール管理を開く'}
            className={`p-1.5 rounded-md transition-colors ml-1 ${
              toolPaneOpen
                ? 'text-primary bg-primary/10'
                : 'text-foreground hover:bg-muted/50'
            }`}
            aria-label="ツール管理パネルの切り替え"
          >
            <Wrench className="h-4 w-4" />
          </button>
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
          {isStreaming && !executingToolName && <StreamingIndicator content={streamingContent} isThinking={isThinking} />}
          {executingToolName && (
            <div className="px-4 py-2">
              <ToolCallIndicator toolName={executingToolName} />
            </div>
          )}
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

        {/* File ops permission request dialog */}
        {pendingAccessRequests.length > 0 && (
          <div className="absolute inset-0 bg-background/60 backdrop-blur-sm z-40 flex items-center justify-center">
            <div className="bg-background border border-border rounded-lg shadow-lg p-5 w-80 space-y-3">
              {pendingAccessRequests.slice(0, 1).map((req) => (
                <div key={req.requestId} className="space-y-3">
                  <div className="flex items-center gap-2">
                    <Shield className="w-5 h-5 text-amber-500 flex-shrink-0" />
                    <span className="text-sm font-semibold">ファイルアクセス許可</span>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    AIが以下のパスへの{req.requiresWrite ? '読み書き' : '読み取り'}アクセスを要求しています:
                  </p>
                  <div className="text-xs font-mono bg-muted rounded px-2 py-1.5 break-all">
                    {req.path}
                  </div>
                  <div className="flex gap-2 pt-1">
                    <button
                      onClick={() => resolveAccessRequest(req.requestId, true)}
                      className="flex-1 px-3 py-1.5 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
                    >
                      許可
                    </button>
                    <button
                      onClick={() => resolveAccessRequest(req.requestId, false)}
                      className="flex-1 px-3 py-1.5 text-sm rounded-md bg-muted text-muted-foreground hover:bg-destructive/20 hover:text-destructive transition-colors"
                    >
                      拒否
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Error display */}
        {error && (
          <div className="px-4 py-2 bg-destructive/10 border-t border-destructive/20 text-destructive text-sm">
            {error}
          </div>
        )}

        {/* Input area — data-drop-target="chat" でドロップ判定対象 */}
        <div data-drop-target="chat">
          <MessageInput
            ref={messageInputRef}
            onSend={handleSend}
            disabled={isStreaming || isTTSGenerating}
            isStreaming={isStreaming}
            isAbortable={isAbortable}
            onStop={stopGeneration}
            isDragOver={isDragOver && dragTarget === 'chat'}
          />
        </div>
      </div>

      {/* Tool management pane (right side) with resize handle */}
      {toolPaneOpen && (
        <>
          {/* Resize handle */}
          <div
            onMouseDown={handleResizeStart}
            className="w-1 hover:w-1.5 bg-border hover:bg-primary/50 cursor-col-resize transition-colors flex-shrink-0"
          />
          {/* Tool pane with dynamic width */}
          <div style={{ width: paneWidth }} className="flex-shrink-0 overflow-hidden">
            <ToolManagementPane onClose={() => setToolPaneOpen(false)} />
          </div>
        </>
      )}
    </div>
  );
}
