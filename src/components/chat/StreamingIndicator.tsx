interface StreamingIndicatorProps {
  content?: string;
}

export function StreamingIndicator({ content }: StreamingIndicatorProps) {
  return (
    <div className="flex items-start gap-3 px-4 py-2">
      <div className="flex flex-col gap-1 max-w-[70%]">
        {content && (
          <div className="rounded-lg bg-card text-card-foreground px-3 py-2 text-sm whitespace-pre-wrap">
            {content}
          </div>
        )}
        <div className="flex items-center gap-1 px-2">
          <span className="h-2 w-2 rounded-full bg-muted-foreground animate-bounce [animation-delay:0ms]" />
          <span className="h-2 w-2 rounded-full bg-muted-foreground animate-bounce [animation-delay:150ms]" />
          <span className="h-2 w-2 rounded-full bg-muted-foreground animate-bounce [animation-delay:300ms]" />
        </div>
      </div>
    </div>
  );
}
