import { useState, useCallback } from 'react';
import { Copy, Check, ChevronRight, ChevronDown } from 'lucide-react';

interface JsonViewerProps {
  data: unknown;
}

/** JSON データを折り畳み可能なツリー形式で表示するウィジェット */
export function JsonViewer({ data }: JsonViewerProps) {
  const [copied, setCopied] = useState(false);
  const formatted = JSON.stringify(data, null, 2);

  const handleCopy = useCallback(async () => {
    await navigator.clipboard.writeText(formatted);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [formatted]);

  return (
    <div className="relative group/json rounded-md border border-border bg-muted/30 overflow-hidden">
      <div className="flex items-center justify-between px-2.5 py-1.5 border-b border-border/50 bg-muted/50">
        <span className="text-[11px] font-medium text-muted-foreground">JSON</span>
        <button
          onClick={handleCopy}
          className="p-0.5 rounded text-muted-foreground hover:text-foreground transition-colors"
          title={copied ? 'コピー済み' : 'コピー'}
        >
          {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
        </button>
      </div>
      <pre className="text-[11px] text-muted-foreground whitespace-pre-wrap break-all font-mono p-2.5 overflow-x-auto max-h-[300px] overflow-y-auto">
        {formatted}
      </pre>
    </div>
  );
}

/** 再帰的に展開可能なJSONツリーノード */
function JsonTreeNode({ label, value, depth = 0 }: { label?: string; value: unknown; depth?: number }) {
  const [expanded, setExpanded] = useState(depth < 2);

  if (value === null) {
    return (
      <div className="flex items-center gap-1" style={{ paddingLeft: `${depth * 12}px` }}>
        {label && <span className="text-blue-400">{label}: </span>}
        <span className="text-orange-400">null</span>
      </div>
    );
  }

  if (typeof value === 'object' && value !== null) {
    const isArray = Array.isArray(value);
    const entries = isArray
      ? (value as unknown[]).map((v, i) => [String(i), v] as const)
      : Object.entries(value as Record<string, unknown>);
    const bracket = isArray ? ['[', ']'] : ['{', '}'];

    return (
      <div>
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-0.5 hover:bg-muted/50 rounded px-0.5"
          style={{ paddingLeft: `${depth * 12}px` }}
        >
          {expanded ? <ChevronDown className="h-2.5 w-2.5" /> : <ChevronRight className="h-2.5 w-2.5" />}
          {label && <span className="text-blue-400">{label}: </span>}
          <span className="text-muted-foreground">
            {bracket[0]}{!expanded && `...${bracket[1]} (${entries.length})`}
          </span>
        </button>
        {expanded && (
          <>
            {entries.map(([key, val]) => (
              <JsonTreeNode key={key} label={key} value={val} depth={depth + 1} />
            ))}
            <div style={{ paddingLeft: `${depth * 12}px` }} className="text-muted-foreground">
              {bracket[1]}
            </div>
          </>
        )}
      </div>
    );
  }

  return (
    <div className="flex items-center gap-1" style={{ paddingLeft: `${depth * 12}px` }}>
      {label && <span className="text-blue-400">{label}: </span>}
      <span className={typeof value === 'string' ? 'text-green-400' : 'text-orange-400'}>
        {typeof value === 'string' ? `"${value}"` : String(value)}
      </span>
    </div>
  );
}

/** ツリー表示モード付きの高機能JSONビューア */
export function JsonTreeViewer({ data }: JsonViewerProps) {
  return (
    <div className="rounded-md border border-border bg-muted/30 overflow-hidden">
      <div className="px-2.5 py-1.5 border-b border-border/50 bg-muted/50">
        <span className="text-[11px] font-medium text-muted-foreground">JSON Tree</span>
      </div>
      <div className="text-[11px] font-mono p-2 overflow-x-auto max-h-[300px] overflow-y-auto">
        <JsonTreeNode value={data} />
      </div>
    </div>
  );
}
