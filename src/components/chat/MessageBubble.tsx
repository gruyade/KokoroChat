import { useState, type ReactNode } from 'react';
import { Bot, User, Sparkles, Wrench, Copy, RefreshCw, Trash2, Info, Pencil, Volume2, ChevronRight, ChevronDown } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import type { ChatMessageRecord } from '../../types';
import type { ToolCall } from '../../types/plugin';
import { EditableMessage } from './EditableMessage';
import { MarkdownRenderer } from './MarkdownRenderer';
import { ToolResultRenderer } from './ToolResultRenderer';
import { useChatStore, useCharacterStore, useConfigStore } from '../../stores';
import { useAudioStore } from '../../hooks/useAudio';
import { AvatarImage } from '../common/AvatarImage';


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

/** 純粋関数: メッセージコンテンツを前処理する */
function prepareContent(message: ChatMessageRecord) {
  const isSystemMessage =
    message.role === 'user' && message.content.startsWith('[SYSTEM] ');
  const rawContent = isSystemMessage ? message.content.slice(9) : message.content;
  // 《》で囲まれた地の文マーカーをイタリック記法に変換
  const displayContent = rawContent.replace(/《([^》]*)》/g, '*$1*');
  return { isSystemMessage, displayContent };
}

/** ホバーアクションボタン — 共通スタイル */
function ActionButton({
  onClick,
  disabled,
  title,
  destructive = false,
  children,
}: {
  onClick?: () => void;
  disabled?: boolean;
  title?: string;
  destructive?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`p-1 rounded text-muted-foreground transition-colors disabled:opacity-50 ${
        destructive
          ? 'hover:bg-destructive/10 hover:text-destructive'
          : 'hover:bg-muted hover:text-foreground'
      }`}
      title={title}
    >
      {children}
    </button>
  );
}

/** ホバー時に表示するアクションバー */
function MessageActionBar({
  align = 'start',
  children,
}: {
  align?: 'start' | 'end';
  children: ReactNode;
}) {
  return (
    <div
      className={`flex items-center gap-0.5 h-6 overflow-visible ${
        align === 'end' ? 'justify-end' : ''
      }`}
    >
      <div className="flex items-center gap-0.5 transition-all pointer-events-auto opacity-0 invisible group-hover:opacity-100 group-hover:visible">
        {children}
      </div>
    </div>
  );
}

/** 添付ファイル一覧 */
function AttachmentList({
  attachments,
}: {
  attachments: NonNullable<ChatMessageRecord['attachments']>;
}) {
  return (
    <div className="flex flex-col gap-1 mt-1">
      {attachments.map((att, i) =>
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
      )}
    </div>
  );
}

/** 折り畳み可能なツール呼び出し表示（アシスタントメッセージ内） */
function CollapsibleToolCall({ toolCall }: { toolCall: ToolCall }) {
  const [expanded, setExpanded] = useState(false);
  const argsStr = JSON.stringify(toolCall.arguments, null, 2);

  return (
    <div className="rounded-md border border-border bg-muted/30 text-xs">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 w-full px-2.5 py-1.5 text-left text-muted-foreground hover:bg-muted/50 transition-colors rounded-md"
      >
        {expanded ? <ChevronDown className="h-3 w-3 shrink-0" /> : <ChevronRight className="h-3 w-3 shrink-0" />}
        <Wrench className="h-3 w-3 shrink-0" />
        <span className="truncate">{toolCall.name} を呼び出し</span>
      </button>
      {expanded && (
        <div className="px-2.5 pb-2 pt-0.5 border-t border-border/50">
          <pre className="text-[11px] text-muted-foreground whitespace-pre-wrap break-all font-mono bg-muted/40 rounded p-1.5 overflow-x-auto">
            {argsStr}
          </pre>
        </div>
      )}
    </div>
  );
}

/** 折り畳み可能なツール実行結果表示（role=tool メッセージ） */
function CollapsibleToolResult({ message }: { message: ChatMessageRecord }) {
  const [expanded, setExpanded] = useState(false);
  const summary = message.content.length > 80
    ? message.content.slice(0, 80) + '…'
    : message.content;

  return (
    <div className="flex justify-start px-4 py-0.5">
      <div className="flex items-start gap-2 max-w-[70%]">
        <div className="shrink-0 mt-1 w-6 h-6 rounded-full bg-muted/60 flex items-center justify-center">
          <Wrench className="h-3 w-3 text-muted-foreground" />
        </div>
        <div className="flex flex-col gap-0.5">
          <div className="rounded-md border border-border/60 bg-muted/20 text-xs">
            <button
              onClick={() => setExpanded(!expanded)}
              className="flex items-center gap-1.5 w-full px-2.5 py-1.5 text-left text-muted-foreground hover:bg-muted/30 transition-colors rounded-md"
            >
              {expanded ? <ChevronDown className="h-3 w-3 shrink-0" /> : <ChevronRight className="h-3 w-3 shrink-0" />}
              <span className="truncate">🔧 ツール結果{!expanded && `: ${summary}`}</span>
            </button>
            {expanded && (
              <div className="px-2.5 pb-2 pt-0.5 border-t border-border/50">
                <ToolResultRenderer content={message.content} />
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export function MessageBubble({ message, onRegenerate, onDelete }: MessageBubbleProps) {
  const config = getRoleConfig(message.role);
  const [copied, setCopied] = useState(false);
  const [ttsLoading, setTtsLoading] = useState(false);
  const { editingMessageId, setEditingMessage, editAndResend } = useChatStore();
  const { selectedCharacterId, characters } = useCharacterStore();
  const { config: appConfig } = useConfigStore();
  const ttsEnabled = appConfig?.tts?.enabled ?? false;
  const selectedCharacter = characters.find((c) => c.id === selectedCharacterId);

  const isEditing = editingMessageId === message.id;

  // role=tool かつ tool_call_id を持つメッセージは折り畳みツール結果として表示
  if (message.role === 'tool' && message.tool_call_id) {
    return <CollapsibleToolResult message={message} />;
  }

  const { isSystemMessage, displayContent } = prepareContent(message);

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
          {/* Action buttons */}
          <MessageActionBar align="end">
            <ActionButton onClick={handleEdit} title="編集">
              <Pencil className="h-3.5 w-3.5" />
            </ActionButton>
            {onDelete && (
              <ActionButton onClick={handleDelete} title="削除" destructive>
                <Trash2 className="h-3.5 w-3.5" />
              </ActionButton>
            )}
          </MessageActionBar>
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
          <div className="shrink-0 mt-1 w-7 h-7 rounded-full bg-muted flex items-center justify-center overflow-hidden">
            {selectedCharacter?.avatar_path ? (
              <AvatarImage
                avatarPath={selectedCharacter.avatar_path}
                alt={selectedCharacter.name}
                className="w-full h-full object-cover"
              />
            ) : (
              <config.icon className="h-4 w-4 text-muted-foreground" />
            )}
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
              {/* テキストが空でない場合のみバブルを表示 */}
              {message.content.trim() !== '' && (
                <div className={`rounded-lg px-3 py-2 text-sm ${config.bubble}`}>
                  <MarkdownRenderer
                    content={displayContent}
                    className={`prose prose-sm max-w-none prose-p:my-1 prose-headings:my-2 prose-ul:my-1 prose-ol:my-1 prose-pre:my-2 prose-code:text-xs ${
                      message.role === 'user'
                        ? 'prose-p:text-primary-foreground prose-headings:text-primary-foreground prose-strong:text-primary-foreground prose-code:text-primary-foreground prose-li:text-primary-foreground text-primary-foreground'
                        : 'prose-headings:text-card-foreground prose-p:text-card-foreground prose-strong:text-card-foreground prose-code:text-card-foreground prose-li:text-card-foreground text-card-foreground'
                    }`}
                  />
                </div>
              )}
            </>
          )}

          {/* Attachments */}
          {message.attachments && message.attachments.length > 0 && (
            <AttachmentList attachments={message.attachments} />
          )}
          {/* Tool calls (collapsible) */}
          {message.tool_calls && message.tool_calls.length > 0 && (
            <div className="flex flex-col gap-1 mt-1">
              {message.tool_calls.map((tc) => (
                <CollapsibleToolCall key={tc.id} toolCall={tc} />
              ))}
            </div>
          )}

          {/* Action buttons — 全要素の下に配置、ホバーで表示 */}
          {!isEditing && (
            <MessageActionBar align={message.role === 'user' ? 'end' : 'start'}>
              <ActionButton onClick={handleCopy} title={copied ? 'コピー済み' : 'コピー'}>
                <Copy className="h-3.5 w-3.5" />
              </ActionButton>
              {message.role === 'user' && !isSystemMessage && (
                <ActionButton onClick={handleEdit} title="編集">
                  <Pencil className="h-3.5 w-3.5" />
                </ActionButton>
              )}
              {message.role === 'assistant' && onRegenerate && (
                <ActionButton onClick={handleRegenerate} title="再生成">
                  <RefreshCw className="h-3.5 w-3.5" />
                </ActionButton>
              )}
              {message.role === 'assistant' && ttsEnabled && (
                <ActionButton onClick={handleGenerateSpeech} disabled={ttsLoading} title="音声生成">
                  <Volume2 className={`h-3.5 w-3.5 ${ttsLoading ? 'animate-pulse' : ''}`} />
                </ActionButton>
              )}
              {onDelete && (
                <ActionButton onClick={handleDelete} title="削除" destructive>
                  <Trash2 className="h-3.5 w-3.5" />
                </ActionButton>
              )}
            </MessageActionBar>
          )}
        </div>
      </div>
    </div>
  );
}
