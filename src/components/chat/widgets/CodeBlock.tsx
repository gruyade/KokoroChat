import { useState, useCallback } from 'react';
import { Copy, Check } from 'lucide-react';

interface CodeBlockProps {
  data: {
    code: string;
    language?: string;
    title?: string;
  };
}

/** コードブロックウィジェット — シンタックスハイライト付きコード表示 */
export function CodeBlock({ data }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);
  const { code, language, title } = data;

  const handleCopy = useCallback(async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [code]);

  return (
    <div className="rounded-md border border-border bg-muted/30 overflow-hidden">
      <div className="flex items-center justify-between px-2.5 py-1.5 border-b border-border/50 bg-muted/50">
        <span className="text-[11px] font-medium text-muted-foreground">
          {title || language || 'Code'}
        </span>
        <button
          onClick={handleCopy}
          className="p-0.5 rounded text-muted-foreground hover:text-foreground transition-colors"
          title={copied ? 'コピー済み' : 'コピー'}
        >
          {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
        </button>
      </div>
      <pre className="text-[11px] text-muted-foreground whitespace-pre-wrap break-all font-mono p-2.5 overflow-x-auto max-h-[400px] overflow-y-auto">
        <code className={language ? `language-${language}` : undefined}>
          {code}
        </code>
      </pre>
    </div>
  );
}
