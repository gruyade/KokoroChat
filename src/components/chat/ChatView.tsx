import { useEffect, useRef } from 'react';
import { MessageSquare } from 'lucide-react';
import { useChatStore } from '../../stores';
import { MessageBubble } from './MessageBubble';
import { MessageInput } from './MessageInput';
import { StreamingIndicator } from './StreamingIndicator';

export function ChatView() {
  const { currentSessionId, messages, isStreaming, streamingContent, error, sendMessage } =
    useChatStore();
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll on new messages or streaming content
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, streamingContent]);

  // No session selected — placeholder
  if (!currentSessionId) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground gap-3">
        <MessageSquare className="h-12 w-12" />
        <p className="text-sm">チャットセッションを選択してください</p>
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
