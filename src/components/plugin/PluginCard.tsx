import { Puzzle, ChevronDown, ChevronRight } from 'lucide-react';
import { useState } from 'react';
import type { PluginInfo } from '../../types';

interface PluginCardProps {
  plugin: PluginInfo;
  onToggle: (enabled: boolean) => void;
}

export function PluginCard({ plugin, onToggle }: PluginCardProps) {
  const [showTools, setShowTools] = useState(false);

  return (
    <div className="p-4 rounded-lg border border-border bg-card">
      <div className="flex items-start gap-3">
        {/* Icon */}
        <div className="w-9 h-9 rounded-md bg-muted flex items-center justify-center flex-shrink-0">
          <Puzzle className="w-4.5 h-4.5 text-muted-foreground" />
        </div>

        {/* Info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center justify-between">
            <h3 className="font-medium text-sm">{plugin.name}</h3>
            <label className="flex items-center gap-2 cursor-pointer flex-shrink-0">
              <input
                type="checkbox"
                checked={plugin.enabled}
                onChange={(e) => onToggle(e.target.checked)}
                className="rounded"
              />
              <span className="text-xs text-muted-foreground">
                {plugin.enabled ? '有効' : '無効'}
              </span>
            </label>
          </div>
          <p className="text-xs text-muted-foreground mt-1">{plugin.description}</p>
          <span className="text-xs text-muted-foreground">v{plugin.version}</span>
        </div>
      </div>

      {/* Tools */}
      {plugin.tools.length > 0 && (
        <div className="mt-3">
          <button
            onClick={() => setShowTools(!showTools)}
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            {showTools ? (
              <ChevronDown className="w-3.5 h-3.5" />
            ) : (
              <ChevronRight className="w-3.5 h-3.5" />
            )}
            ツール一覧 ({plugin.tools.length})
          </button>
          {showTools && (
            <ul className="mt-2 space-y-1 pl-5">
              {plugin.tools.map((tool) => (
                <li key={tool.name} className="text-xs">
                  <span className="font-mono text-foreground">{tool.name}</span>
                  <span className="text-muted-foreground ml-2">— {tool.description}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
