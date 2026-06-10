import { useState, useEffect } from 'react';
import { ChevronRight, ChevronDown } from 'lucide-react';

/** redacted thinking のマーカー文字列 */
const REDACTED_THINKING_MARKER = '[REDACTED_THINKING]';

interface ThinkingSectionProps {
  thinkingContent: string;
  isStreaming?: boolean;
  isRedacted?: boolean;
  defaultExpanded?: boolean;
}

/**
 * 折り畳み可能な Thinking/Reasoning Content 表示セクション。
 * ストリーミング中はデフォルト展開、完了後はデフォルト折り畳み。
 * redacted_thinking 検出時はプレースホルダー表示。
 */
export function ThinkingSection({
  thinkingContent,
  isStreaming = false,
  isRedacted = false,
  defaultExpanded = false,
}: ThinkingSectionProps) {
  const [expanded, setExpanded] = useState(isStreaming || defaultExpanded);

  // ストリーミング開始時に展開、完了時に折り畳み
  useEffect(() => {
    if (isStreaming) {
      setExpanded(true);
    } else if (!defaultExpanded) {
      setExpanded(false);
    }
  }, [isStreaming, defaultExpanded]);

  // コンテンツ内に REDACTED マーカーが含まれるか検出
  const hasRedactedMarker = thinkingContent.includes(REDACTED_THINKING_MARKER);
  const showRedactedPlaceholder = isRedacted || hasRedactedMarker;

  return (
    <div className="rounded-md border-l-[3px] border-amber-500/70 bg-amber-50/50 dark:bg-amber-950/20 mb-1">
      {/* ヘッダー: トグルボタン */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 w-full px-2.5 py-1.5 text-left text-xs text-amber-800 dark:text-amber-300 hover:bg-amber-100/50 dark:hover:bg-amber-900/30 transition-colors rounded-t-md"
        aria-expanded={expanded}
        aria-label="思考プロセスの表示切り替え"
      >
        {expanded ? (
          <ChevronDown className="h-3 w-3 shrink-0" />
        ) : (
          <ChevronRight className="h-3 w-3 shrink-0" />
        )}
        <span className="font-medium">思考プロセス</span>
        {isStreaming && (
          <span className="ml-1 text-[10px] text-amber-600 dark:text-amber-400 animate-pulse">
            思考中...
          </span>
        )}
      </button>

      {/* コンテンツ本体 */}
      {expanded && (
        <div className="px-3 pb-2 pt-0.5 border-t border-amber-200/50 dark:border-amber-800/30">
          {showRedactedPlaceholder ? (
            <p className="text-xs text-muted-foreground italic py-2 text-center opacity-70">
              思考内容は非表示です
            </p>
          ) : (
            <div className="max-h-[300px] overflow-y-auto">
              <p className="text-xs italic text-amber-900/80 dark:text-amber-200/80 whitespace-pre-wrap break-words leading-relaxed">
                {thinkingContent}
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
