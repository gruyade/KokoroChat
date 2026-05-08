import { useState } from 'react';
import { Bot, User, Sparkles, Wrench, Copy, RefreshCw, Trash2, Info, Pencil, Volume2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import type { ChatMessageRecord } from '../../types';
import { ToolCallIndicator } from './ToolCallIndicator';
import { EditableMessage } from './EditableMessage';
import { MarkdownRenderer } from './MarkdownRenderer';
import { useChatStore, useCharacterStore, useConfigStore } from '../../stores';
import { useAudioStore } from '../../hooks/useAudio';

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
  const [copied, setCopied] = useState(false);
  const [ttsLoading, setTtsLoading] = useState(false);
  const { editingMessageId, setEditingMessage, editAndResend } = useChatStore();
  const { selectedCharacterId } = useCharacterStore();
  const { config: appConfig } = useConfigStore();
  const ttsEnabled = appConfig?.tts?.enabled ?? false;

  const isEditing = editingMessageId === message.id;

  // [SYSTEM]プレフィックス付きメッセージはシステムメッセージとして表示
  const isSystemMessage = message.role === 'user' && message.content.startsWith('[SYSTEM] ');
  const rawContent = isSystemMessage ? message.content.slice(9) : message.content;
  // 《》で囲まれた地の文マーカーを除去して表示（イタリック表示に変換）
  const displayContent = rawContent.replace(/《([^》]*)》/g, '*$1*');

  const handleCopy = async () => {
    await navigator.clipboard.writeText(displayContent);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const handleRegenerate = () => {
    onRegenerate?.(message.id);
  };

  const handleDelete = () => {
    onDelete?.(message.id);
  };

  const handleEdit = () => {
    setEditingMessage(message.id);
  };

  const handleEditConfirm = (newContent: string) => {
    editAndResend(message.id, newContent);
  };

  const handleGenerateSpeech = async () => {
    if (!selectedCharacterId || ttsLoading) return;
    setTtsLoading(true);
    try {
      const audioBase64 = await invoke<string>('generate_speech_for_message', {
        text: message.content,
        characterId: selectedCharacterId,
      });
      if (audioBase64) {
        const playFn = useAudioStore.getState().playAudioFn;
        if (playFn) {
          playFn(audioBase64);
        }
      }
    } catch (e) {
      console.error('[TTS] generate_speech_for_message failed:', e);
    } finally {
      setTtsLoading(false);
    }
  };

  const handleEditCancel = () => {
    setEditingMessage(null);
  };

  // システムメッセージ: 右寄せバッジスタイルで表示（ホバーアクションは通常メッセージと同じ下部配置）
  if (isSystemMessage) {
    if (isEditing) {
      return (
        <div className="flex justify-end px-4 py-1">
          <EditableMessage
            originalContent={message.content}
            onConfirm={handleEditConfirm}
            onCancel={handleEditCancel}
          />
        </div>
      );
    }

    return (
      <div
        className="group relative flex justify-end px-4 py-1 will-change-transform"
      >
        <div className="flex flex-col gap-1">
          <div className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-muted text-muted-foreground text-xs">
            <Info className="h-3 w-3" />
            <span>{displayContent}</span>
          </div>
          {/* Action buttons — 通常メッセージと同じ下部配置 */}
          <div className="flex items-center gap-0.5 h-6 overflow-visible justify-end">
            <div className="flex items-center gap-0.5 transition-all pointer-events-auto opacity-0 invisible group-hover:opacity-100 group-hover:visible">
              <button
                onClick={handleEdit}
                className="p-1 rounded text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                title="編集"
              >
                <Pencil className="h-3.5 w-3.5" />
              </button>
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
        </div>
      </div>
    );
  }

  return (
    <div
      className={`group relative flex ${config.align} px-4 py-1 will-change-transform`}
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
              {/* 添付ファイルのみの場合はテキストバブルを非表示 */}
              {!(message.content === '(添付ファイル)' && message.role === 'user' && message.attachments && message.attachments.length > 0) && (
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
              )}

              {/* Action buttons — 常にスペース確保、ホバーで表示 */}
              <div className={`flex items-center gap-0.5 h-6 overflow-visible ${message.role === 'user' ? 'justify-end' : ''}`}>
                <div className="flex items-center gap-0.5 transition-all pointer-events-auto opacity-0 invisible group-hover:opacity-100 group-hover:visible">
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
                  {message.role === 'assistant' && ttsEnabled && (
                    <button
                      onClick={handleGenerateSpeech}
                      disabled={ttsLoading}
                      className="p-1 rounded text-muted-foreground hover:bg-muted hover:text-foreground transition-colors disabled:opacity-50"
                      title="音声生成"
                    >
                      <Volume2 className={`h-3.5 w-3.5 ${ttsLoading ? 'animate-pulse' : ''}`} />
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
            <div className="flex flex-col gap-1 mt-1">
              {message.attachments.map((att, i) => (
                att.attachment_type === 'image' && att.base64_data ? (
                  <img
                    key={i}
                    src={`data:image/png;base64,${att.base64_data}`}
                    alt={att.file_name}
                    className="max-w-[240px] max-h-[180px] rounded-md border border-border object-contain"
                  />
                ) : (
                  <span
                    key={i}
                    className="inline-flex items-center gap-1 rounded bg-muted px-2 py-0.5 text-xs text-muted-foreground"
                  >
                    📎 {att.file_name}
                  </span>
                )
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
