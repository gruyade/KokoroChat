import { useEffect, useRef } from 'react';
import { MessageSquare } from 'lucide-react';
import { useChatStore, useCharacterStore } from '../../stores';
import { MessageBubble } from './MessageBubble';
import { MessageInput } from './MessageInput';
import { StreamingIndicator } from './StreamingIndicator';

export function ChatView() {
  const { currentSessionId, messages, isStreaming, streamingContent, error, sendMessage, createSession } =
    useChatStore();
  const { characters, selectedCharacterId, selectCharacter, fetchCharacters } = useCharacterStore();
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // 初回マウント時にキャラクター一覧を取得
  useEffect(() => {
    fetchCharacters();
  }, [fetchCharacters]);

  // Auto-scroll on new messages or streaming content
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, streamingContent]);

  // No session selected — show character selection + new session UI
  if (!currentSessionId) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground gap-4 p-6">
        <MessageSquare className="h-12 w-12" />
        <p className="text-sm">新しいチャットを開始</p>

        {characters.length === 0 ? (
          <p className="text-xs">先にキャラクターを作成してください</p>
        ) : (
          <div className="flex flex-col items-center gap-3 w-full max-w-xs">
            <select
              value={selectedCharacterId ?? ''}
              onChange={(e) => selectCharacter(e.target.value || null)}
              className="w-full px-3 py-2 rounded-md border border-border bg-background text-foreground text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            >
              <option value="">キャラクターを選択...</option>
              {characters.map((c) => (
                <option key={c.id} value={c.id}>{c.name}</option>
              ))}
            </select>
            <button
              onClick={() => selectedCharacterId && createSession(selectedCharacterId)}
              disabled={!selectedCharacterId}
              className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              チャット開始
            </button>
          </div>
        )}
      </div>
    );
  }

  const handleSend = (content: string) => {
    sendMessage(content);
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Messages area */}
      <div className="flex-1 overflow-y-auto py-4">
        {messages.length === 0 && !isStreaming && (
          <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
            メッセージを送信して会話を始めましょう
          </div>
        )}
        {messages.map((msg) => (
          <MessageBubble key={msg.id} message={msg} />
        ))}
        {isStreaming && <StreamingIndicator content={streamingContent} />}
        <div ref={messagesEndRef} />
      </div>

      {/* Error display */}
      {error && (
        <div className="px-4 py-2 bg-destructive/10 border-t border-destructive/20 text-destructive text-sm">
          {error}
        </div>
      )}

      {/* Input area */}
      <MessageInput onSend={handleSend} disabled={isStreaming} />
    </div>
  );
}
