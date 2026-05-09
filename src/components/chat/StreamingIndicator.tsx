interface StreamingIndicatorProps {
  content?: string;
}

export function StreamingIndicator({ content }: StreamingIndicatorProps) {
  return (
    <div className="flex justify-start px-4 py-1">
      <div className="flex items-start gap-2 max-w-[70%]">
        <div className="shrink-0 mt-1 p-1 rounded-full bg-muted">
          <span className="block h-4 w-4 text-muted-foreground text-xs text-center">●</span>
        </div>
        <div className="flex flex-col gap-1">
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
        </div>
      </div>
    </div>
  );
}
