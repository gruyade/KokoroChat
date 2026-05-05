import { Bot, User, Sparkles, Wrench } from 'lucide-react';
import type { ChatMessageRecord } from '../../types';
import { ToolCallIndicator } from './ToolCallIndicator';

interface MessageBubbleProps {
  message: ChatMessageRecord;
}

function getRoleConfig(role: ChatMessageRecord['role']) {
  switch (role) {
    case 'user':
      return {
        align: 'justify-end' as const,
        bubble: 'bg-primary text-primary-foreground',
        icon: User,
        showIcon: false,
      };
    case 'assistant':
      return {
        align: 'justify-start' as const,
        bubble: 'bg-card text-card-foreground border border-border',
        icon: Bot,
        showIcon: true,
      };
    case 'spontaneous':
      return {
        align: 'justify-start' as const,
        bubble: 'bg-accent text-accent-foreground border border-border',
        icon: Sparkles,
        showIcon: true,
      };
    case 'tool':
      return {
        align: 'justify-start' as const,
        bubble: 'bg-muted text-muted-foreground border border-border',
        icon: Wrench,
        showIcon: true,
      };
  }
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const config = getRoleConfig(message.role);

  return (
    <div className={`flex ${config.align} px-4 py-1`}>
      <div
        className={`flex items-start gap-2 max-w-[70%] ${message.role === 'user' ? 'flex-row-reverse' : ''}`}
      >
        {config.showIcon && (
          <div className="shrink-0 mt-1 p-1 rounded-full bg-muted">
            <config.icon className="h-4 w-4 text-muted-foreground" />
          </div>
        )}
        <div className="flex flex-col gap-1">
          {message.role === 'spontaneous' && (
            <span className="text-xs text-muted-foreground flex items-center gap-1">
              <Sparkles className="h-3 w-3" />
              自発的発話
            </span>
          )}
          <div className={`rounded-lg px-3 py-2 text-sm whitespace-pre-wrap ${config.bubble}`}>
            {message.content}
          </div>
          {/* Attachments */}
          {message.attachments && message.attachments.length > 0 && (
            <div className="flex flex-wrap gap-1 mt-1">
              {message.attachments.map((att, i) => (
                <span
                  key={i}
                  className="inline-flex items-center gap-1 rounded bg-muted px-2 py-0.5 text-xs text-muted-foreground"
                >
                  📎 {att.file_name}
                </span>
              ))}
            </div>
          )}
          {/* Tool calls */}
          {message.tool_calls && message.tool_calls.length > 0 && (
            <div className="flex flex-col gap-1 mt-1">
              {message.tool_calls.map((tc) => (
                <ToolCallIndicator key={tc.id} toolName={tc.name} />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
