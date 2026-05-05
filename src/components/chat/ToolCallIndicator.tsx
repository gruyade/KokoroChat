import { Wrench, Loader2 } from 'lucide-react';

interface ToolCallIndicatorProps {
  toolName: string;
}

export function ToolCallIndicator({ toolName }: ToolCallIndicatorProps) {
  return (
    <div className="flex items-center gap-2 rounded-md border border-border bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
      <Wrench className="h-4 w-4 shrink-0" />
      <span className="truncate">ツール実行中: {toolName}</span>
      <Loader2 className="h-4 w-4 shrink-0 animate-spin ml-auto" />
    </div>
  );
}
