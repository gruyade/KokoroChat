import { useState } from 'react';
import { Bot, User, Sparkles, Wrench, Copy, RefreshCw, Trash2, Info, Pencil } from 'lucide-react';
import type { ChatMessageRecord } from '../../types';
import { ToolCallIndicator } from './ToolCallIndicator';
import { EditableMessage } from './EditableMessage';
import { MarkdownRenderer } from './MarkdownRenderer';
import { useChatStore } from '../../stores';

interface MessageBubbleProps {
  message: ChatMessageRecord;
  onRegenerate?: (messageId: string) => void;
  onDelete?: (messageId: string) => void;
}

function getRoleConfig(role: ChatMessageRecord['role']) {
  switch (role) {
    case 'user':
      return {
        align: 'justify-end' as const,
        bubble: 'bg-primary text-primary-foreground',
        icon: User,
        showIcon: false,
        label: '',
      };
    case 'assistant':
      return {
        align: 'justify-start' as const,
        bubble: 'bg-card text-card-foreground border border-border',
        icon: Bot,
        showIcon: true,
        label: '',
      };
    case 'spontaneous':
      return {
        align: 'justify-start' as const,
        bubble: 'bg-accent text-accent-foreground border border-border',
        icon: Sparkles,
        showIcon: true,
        label: '自発的発話',
      };
    case 'tool':
      return {
        align: 'justify-start' as const,
        bubble: 'bg-muted text-muted-foreground border border-border',
        icon: Wrench,
        showIcon: true,
        label: '',
      };
  }
}

export function MessageBubble({ message, onRegenerate, onDelete }: MessageBubbleProps) {
  const config = getRoleConfig(message.role);
  const [showMenu, setShowMenu] = useState(false);
  const [copied, setCopied] = useState(false);
  const { editingMessageId, setEditingMessage, editAndResend } = useChatStore();

  const isEditing = editingMessageId === message.id;

  // [SYSTEM]プレフィックス付きメッセージはシステムメッセージとして表示
  const isSystemMessage = message.role === 'user' && message.content.startsWith('[SYSTEM] ');
  const displayContent = isSystemMessage ? message.content.slice(9) : message.content;

  const handleCopy = async () => {
    await navigator.clipboard.writeText(displayContent);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const handleRegenerate = () => {
    onRegenerate?.(message.id);
    setShowMenu(false);
  };

  const handleDelete = () => {
    onDelete?.(message.id);
    setShowMenu(false);
  };

  const handleEdit = () => {
    setEditingMessage(message.id);
    setShowMenu(false);
  };

  const handleEditConfirm = (newContent: string) => {
    editAndResend(message.id, newContent);
  };

  const handleEditCancel = () => {
    setEditingMessage(null);
  };

  // システムメッセージの特別表示
  if (isSystemMessage) {
    return (
      <div className="flex justify-center px-4 py-1">
        <div className="flex items-center gap-2 max-w-[80%] rounded-md bg-muted/50 border border-border/50 px-3 py-1.5 text-xs text-muted-foreground">
          <Info className="h-3 w-3 shrink-0" />
          <span className="whitespace-pre-wrap">{displayContent}</span>
        </div>
      </div>
    );
  }

  return (
    <div
      className={`group relative flex ${config.align} px-4 py-1 will-change-transform`}
      onMouseEnter={() => setShowMenu(true)}
      onMouseLeave={() => setShowMenu(false)}
    >
      <div
        className={`flex items-start gap-2 max-w-[70%] ${message.role === 'user' ? 'flex-row-reverse' : ''}`}
      >
        {config.showIcon && (
          <div className="shrink-0 mt-1 p-1 rounded-full bg-muted">
            <config.icon className="h-4 w-4 text-muted-foreground" />
          </div>
        )}
        <div className="flex flex-col gap-1">
          {config.label && (
            <span className="text-xs text-muted-foreground flex items-center gap-1">
              <Sparkles className="h-3 w-3" />
              {config.label}
            </span>
          )}
          {isEditing ? (
            <EditableMessage
              originalContent={message.content}
              onConfirm={handleEditConfirm}
              onCancel={handleEditCancel}
            />
          ) : (
            <>
              <div className={`rounded-lg px-3 py-2 text-sm ${config.bubble}`}>
                <MarkdownRenderer
                  content={displayContent}
                  className={`prose prose-sm max-w-none prose-p:my-1 prose-headings:my-2 prose-ul:my-1 prose-ol:my-1 prose-pre:my-2 prose-code:text-xs ${
                    message.role === 'user'
                      ? 'prose-p:text-primary-foreground prose-headings:text-primary-foreground prose-strong:text-primary-foreground prose-code:text-primary-foreground prose-li:text-primary-foreground text-primary-foreground'
                      : 'prose-invert'
                  }`}
                />
              </div>

              {/* Action buttons — 常にスペース確保、ホバーで表示 */}
              <div className={`flex items-center gap-0.5 h-6 overflow-visible ${message.role === 'user' ? 'justify-end' : ''}`}>
                <div className={`flex items-center gap-0.5 transition-opacity pointer-events-auto ${showMenu ? 'opacity-100' : 'opacity-0 pointer-events-none'}`}>
                  <button
                    onClick={handleCopy}
                    className="p-1 rounded text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                    title={copied ? 'コピー済み' : 'コピー'}
                  >
                    <Copy className="h-3.5 w-3.5" />
                  </button>
                  {message.role === 'user' && !isSystemMessage && (
                    <button
                      onClick={handleEdit}
                      className="p-1 rounded text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                      title="編集"
                    >
                      <Pencil className="h-3.5 w-3.5" />
                    </button>
                  )}
                  {message.role === 'assistant' && onRegenerate && (
                    <button
                      onClick={handleRegenerate}
                      className="p-1 rounded text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                      title="再生成"
                    >
                      <RefreshCw className="h-3.5 w-3.5" />
                    </button>
                  )}
                  {onDelete && (
                    <button
                      onClick={handleDelete}
                      className="p-1 rounded text-muted-foreground hover:bg-destructive/10 hover:text-destructive transition-colors"
                      title="削除"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  )}
                </div>
              </div>
            </>
          )}

          {/* Attachments */}
          {message.attachments && message.attachments.length > 0 && (
            <div className="flex flex-wrap gap-1 mt-1">
              {message.attachments.map((att, i) => (
                <span
                  key={i}
                  className="inline-flex items-center gap-1 rounded bg-muted px-2 py-0.5 text-xs text-muted-foreground"
                >
                  📎 {att.file_name}
                </span>
              ))}
            </div>
          )}
          {/* Tool calls */}
          {message.tool_calls && message.tool_calls.length > 0 && (
            <div className="flex flex-col gap-1 mt-1">
              {message.tool_calls.map((tc) => (
                <ToolCallIndicator key={tc.id} toolName={tc.name} />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
