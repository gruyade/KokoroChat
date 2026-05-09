import { useState, useRef, useEffect } from 'react';
import { Check, X } from 'lucide-react';

interface EditableMessageProps {
  originalContent: string;
  onConfirm: (newContent: string) => void;
  onCancel: () => void;
}

export function EditableMessage({ originalContent, onConfirm, onCancel }: EditableMessageProps) {
  const [content, setContent] = useState(originalContent);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.focus();
      // カーソルを末尾に移動
      textareaRef.current.selectionStart = textareaRef.current.value.length;
      textareaRef.current.selectionEnd = textareaRef.current.value.length;
    }
  }, []);

  // textarea の高さを内容に合わせて自動調整
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
    }
  }, [content]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      if (content.trim()) {
        onConfirm(content);
      }
    }
  };

  return (
    <div className="flex flex-col gap-2 w-full">
      <textarea
        ref={textareaRef}
        value={content}
        onChange={(e) => setContent(e.target.value)}
        onKeyDown={handleKeyDown}
        className="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground resize-none focus:outline-none focus:ring-2 focus:ring-primary/50"
        rows={2}
      />
      <div className="flex items-center gap-1 justify-end">
        <button
          onClick={onCancel}
          className="p-1.5 rounded text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
          title="キャンセル (Esc)"
        >
          <X className="h-4 w-4" />
        </button>
        <button
          onClick={() => content.trim() && onConfirm(content)}
          disabled={!content.trim()}
          className="p-1.5 rounded text-primary hover:bg-primary/10 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title="確定して再送信 (Ctrl+Enter)"
        >
          <Check className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
