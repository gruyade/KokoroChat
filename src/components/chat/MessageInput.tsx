import { useState, useRef, useCallback, useEffect, type KeyboardEvent } from 'react';
import { Send, Paperclip, Info, Square } from 'lucide-react';
import { useConfigStore } from '../../stores';
import type { SendKey } from '../../types';

interface MessageInputProps {
  onSend: (content: string, isSystem?: boolean) => void;
  disabled?: boolean;
  isStreaming?: boolean;
  isAbortable?: boolean;
  onStop?: () => void;
}

export function MessageInput({ onSend, disabled = false, isStreaming = false, isAbortable = false, onStop }: MessageInputProps) {
  const [value, setValue] = useState('');
  const [systemMode, setSystemMode] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sendKey: SendKey = useConfigStore((s) => s.config?.ui.send_key) ?? 'enter';

  const adjustHeight = useCallback(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, 160)}px`;
    }
  }, []);

  const handleSend = useCallback(() => {
    const trimmed = value.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed, systemMode);
    setValue('');
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
    textareaRef.current?.focus();
  }, [value, disabled, onSend, systemMode]);

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
      // それ以外のEnter系コンビネーションはデフォルト動作（改行挿入）
    },
    [handleSend, sendKey]
  );

  // ストリーミング完了後にフォーカスを復帰
  const prevStreamingRef = useRef(isStreaming);
  useEffect(() => {
    if (prevStreamingRef.current && !isStreaming) {
      textareaRef.current?.focus();
    }
    prevStreamingRef.current = isStreaming;
  }, [isStreaming]);

  return (
    <div className="border-t border-border bg-background px-4 py-3">
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
          placeholder={systemMode ? '状況説明やルールを入力...' : 'メッセージを入力...'}
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
            disabled={disabled || !value.trim()}
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
}
