interface StreamingIndicatorProps {
  content?: string;
  isThinking?: boolean;
}

export function StreamingIndicator({ content, isThinking = false }: StreamingIndicatorProps) {
  return (
    <div className="flex justify-start px-4 py-1">
      <div className="flex items-start gap-2 max-w-[70%]">
        <div className="shrink-0 mt-1 p-1 rounded-full bg-muted">
          <span className="block h-4 w-4 text-muted-foreground text-xs text-center">●</span>
        </div>
        <div className="flex flex-col gap-1">
          {/* Thinking状態インジケーター */}
          {isThinking && (
            <div className="flex items-center gap-1.5 text-xs text-amber-600 dark:text-amber-400 py-1 px-1">
              <span className="animate-pulse">🧠</span>
              <span>思考中</span>
              <span className="flex gap-0.5">
                <span className="animate-bounce [animation-delay:0ms]">.</span>
                <span className="animate-bounce [animation-delay:150ms]">.</span>
                <span className="animate-bounce [animation-delay:300ms]">.</span>
              </span>
            </div>
          )}
          {/* 通常のストリーミングインジケーター */}
          {!isThinking && (
            <div className="rounded-lg bg-card text-card-foreground border border-border px-3 py-2 text-sm">
              {content ? (
                <span className="whitespace-pre-wrap">{content}</span>
              ) : null}
              <span className="inline-flex items-center gap-1 ml-1">
                <span className="h-1.5 w-1.5 rounded-full bg-muted-foreground animate-bounce [animation-delay:0ms]" />
                <span className="h-1.5 w-1.5 rounded-full bg-muted-foreground animate-bounce [animation-delay:150ms]" />
                <span className="h-1.5 w-1.5 rounded-full bg-muted-foreground animate-bounce [animation-delay:300ms]" />
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
