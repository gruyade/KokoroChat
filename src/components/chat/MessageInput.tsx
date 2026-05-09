import { useState, useRef, useCallback, useEffect, forwardRef, useImperativeHandle, type KeyboardEvent } from 'react';
import { Send, Paperclip, Info, Square } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { useConfigStore } from '../../stores';
import { AttachmentPreview } from './AttachmentPreview';
import type { Attachment, SendKey } from '../../types';

interface MessageInputProps {
  onSend: (content: string, isSystem?: boolean, attachments?: Attachment[]) => void;
  disabled?: boolean;
  isStreaming?: boolean;
  isAbortable?: boolean;
  onStop?: () => void;
  isDragOver?: boolean;
}

export interface MessageInputRef {
  addAttachment(path: string): Promise<void>;
}

export const MessageInput = forwardRef<MessageInputRef, MessageInputProps>(function MessageInput(
  { onSend, disabled = false, isStreaming = false, isAbortable = false, onStop, isDragOver = false },
  ref
) {
  const [value, setValue] = useState('');
  const [systemMode, setSystemMode] = useState(false);
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sendKey: SendKey = useConfigStore((s) => s.config?.ui.send_key) ?? 'enter';

  useImperativeHandle(ref, () => ({
    async addAttachment(path: string) {
      try {
        const attachment = await invoke<Attachment>('process_attachment', { filePath: path });
        setAttachments((prev) => [...prev, attachment]);
      } catch (e) {
        console.error('Drop attachment error:', e);
      }
    },
  }));

  const adjustHeight = useCallback(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, 160)}px`;
    }
  }, []);

  const handleSend = useCallback(() => {
    const trimmed = value.trim();
    if (!trimmed && attachments.length === 0) return;
    if (disabled) return;
    onSend(trimmed, systemMode, attachments.length > 0 ? attachments : undefined);
    setValue('');
    setAttachments([]);
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
    textareaRef.current?.focus();
  }, [value, disabled, onSend, systemMode, attachments]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key !== 'Enter') return;

      const isSendTrigger =
        (sendKey === 'enter' && !e.shiftKey && !e.ctrlKey && !e.metaKey) ||
        (sendKey === 'ctrl_enter' && (e.ctrlKey || e.metaKey) && !e.shiftKey) ||
        (sendKey === 'shift_enter' && e.shiftKey && !e.ctrlKey && !e.metaKey);

      if (isSendTrigger) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend, sendKey]
  );

  // ファイル選択ダイアログ
  const handleAttachClick = useCallback(async () => {
    try {
      const filePath = await open({
        multiple: false,
        filters: [
          {
            name: 'Supported Files',
            extensions: ['txt', 'md', 'csv', 'pdf', 'png', 'jpg', 'jpeg', 'webp'],
          },
        ],
      });
      if (!filePath) return;
      const attachment = await invoke<Attachment>('process_attachment', { filePath });
      setAttachments((prev) => [...prev, attachment]);
    } catch (e) {
      console.error('Attachment error:', e);
    }
  }, []);

  const removeAttachment = useCallback((id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id));
  }, []);

  // ストリーミング完了後にフォーカスを復帰
  const prevStreamingRef = useRef(isStreaming);
  useEffect(() => {
    if (prevStreamingRef.current && !isStreaming) {
      textareaRef.current?.focus();
    }
    prevStreamingRef.current = isStreaming;
  }, [isStreaming]);

  return (
    <div
      className={`border-t border-border bg-background px-4 py-3 ${isDragOver ? 'ring-2 ring-primary ring-inset' : ''}`}
    >
      {/* Attachment previews */}
      {attachments.length > 0 && (
        <div className="flex flex-wrap gap-2 mb-2">
          {attachments.map((a) => (
            <AttachmentPreview key={a.id} attachment={a} onRemove={() => removeAttachment(a.id)} />
          ))}
        </div>
      )}

      {/* System mode indicator */}
      {systemMode && (
        <div className="flex items-center gap-2 mb-2 px-1 text-xs text-muted-foreground">
          <Info className="h-3.5 w-3.5" />
          <span>システムモード: 状況説明やルールとして送信（会話には含まれない）</span>
        </div>
      )}
      <div className="flex items-end gap-2">
        {/* Attachment button */}
        <button
          onClick={handleAttachClick}
          className="shrink-0 p-2 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
          disabled={disabled}
          aria-label="ファイルを添付"
          title="ファイルを添付"
        >
          <Paperclip className="h-5 w-5" />
        </button>

        {/* System mode toggle */}
        <button
          onClick={() => setSystemMode(!systemMode)}
          className={`shrink-0 p-2 rounded-md transition-colors ${
            systemMode
              ? 'bg-primary/20 text-primary'
              : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
          }`}
          disabled={disabled}
          aria-label="システムモード切替"
          title={systemMode ? 'システムモード ON（状況説明/ルール追加）' : 'システムモード OFF（通常会話）'}
        >
          <Info className="h-5 w-5" />
        </button>

        {/* Textarea */}
        <textarea
          ref={textareaRef}
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            adjustHeight();
          }}
          onKeyDown={handleKeyDown}
          placeholder={isDragOver ? 'ファイルをドロップ...' : systemMode ? '状況説明やルールを入力...' : 'メッセージを入力...'}
          disabled={disabled}
          rows={1}
          className={`flex-1 resize-none rounded-md border px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring disabled:opacity-50 ${
            systemMode ? 'border-primary/50 bg-primary/5' : 'border-input bg-background'
          }`}
        />

        {/* Send / Stop button */}
        {isAbortable ? (
          <button
            onClick={onStop}
            className="shrink-0 p-2 rounded-md bg-destructive text-destructive-foreground hover:bg-destructive/90 transition-colors"
            aria-label="生成を停止"
            title="生成を停止"
          >
            <Square className="h-5 w-5" />
          </button>
        ) : (
          <button
            onClick={handleSend}
            disabled={disabled || (!value.trim() && attachments.length === 0)}
            className={`shrink-0 p-2 rounded-md disabled:opacity-50 disabled:cursor-not-allowed transition-colors ${
              systemMode
                ? 'bg-muted text-foreground hover:bg-muted/80'
                : 'bg-primary text-primary-foreground hover:bg-primary/90'
            }`}
            aria-label="送信"
            title="送信"
          >
            <Send className="h-5 w-5" />
          </button>
        )}
      </div>
    </div>
  );
});
