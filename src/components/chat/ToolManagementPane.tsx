import { useEffect } from 'react';
import { X, Wrench, Lock } from 'lucide-react';
import { usePluginStore } from '../../stores/plugin.store';
import { useChatStore } from '../../stores/chat.store';

interface ToolManagementPaneProps {
  onClose: () => void;
}

/**
 * チャット画面右側に表示される開閉可能なツール管理ペイン。
 * セッション単位でツールの有効/無効を切り替え可能。
 * グローバルで無効化されたツールはグレーアウト（選択不可）。
 */
export function ToolManagementPane({ onClose }: ToolManagementPaneProps) {
  const { plugins, fetchPlugins, loading, sessionPermissions, setSessionToolEnabled, initSessionPermissions } =
    usePluginStore();
  const currentSessionId = useChatStore((s) => s.currentSessionId);

  // プラグイン一覧を取得
  useEffect(() => {
    fetchPlugins();
  }, [fetchPlugins]);

  // セッション許可状態を初期化
  useEffect(() => {
    if (currentSessionId && plugins.length > 0) {
      initSessionPermissions(currentSessionId);
    }
  }, [currentSessionId, plugins, initSessionPermissions]);

  const permissions = currentSessionId ? (sessionPermissions[currentSessionId] ?? {}) : {};

  const handleToggle = (toolName: string, enabled: boolean) => {
    if (!currentSessionId) return;
    setSessionToolEnabled(currentSessionId, toolName, enabled);
  };

  return (
    <div className="w-72 border-l border-border bg-background flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border/50">
        <div className="flex items-center gap-2">
          <Wrench className="w-4 h-4 text-muted-foreground" />
          <h2 className="text-sm font-semibold">ツール管理</h2>
        </div>
        <button
          onClick={onClose}
          className="p-1 rounded-md hover:bg-muted/50 transition-colors text-muted-foreground hover:text-foreground"
          aria-label="ツール管理ペインを閉じる"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-3 space-y-3">
        {loading && plugins.length === 0 ? (
          <div className="flex items-center justify-center h-20 text-muted-foreground text-xs">
            読み込み中...
          </div>
        ) : plugins.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-20 text-muted-foreground text-xs gap-1">
            <Wrench className="w-5 h-5" />
            <span>ツールが登録されていない</span>
          </div>
        ) : !currentSessionId ? (
          <div className="text-xs text-muted-foreground text-center py-4">
            チャットセッションを選択してください
          </div>
        ) : (
          plugins.map((plugin) => (
            <div key={plugin.name} className="space-y-1.5">
              {/* Plugin group header */}
              <div className="flex items-center gap-1.5 px-1">
                <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                  {plugin.name}
                </span>
                {!plugin.enabled && (
                  <Lock className="w-3 h-3 text-muted-foreground/60" aria-label="グローバルで無効" />
                )}
              </div>

              {/* Tools list */}
              {plugin.tools.map((tool) => {
                const globallyDisabled = !plugin.enabled;
                const sessionEnabled = permissions[tool.name] ?? plugin.enabled;

                return (
                  <ToolToggleItem
                    key={tool.name}
                    name={tool.name}
                    description={tool.description}
                    enabled={sessionEnabled}
                    globallyDisabled={globallyDisabled}
                    onToggle={(enabled) => handleToggle(tool.name, enabled)}
                  />
                );
              })}
            </div>
          ))
        )}
      </div>

      {/* Footer hint */}
      <div className="px-4 py-2 border-t border-border/50 text-[10px] text-muted-foreground/70">
        <Lock className="w-3 h-3 inline mr-1" />
        グローバルで無効化されたツールは変更不可
      </div>
    </div>
  );
}

/** 個別ツールのトグルアイテム */
function ToolToggleItem({
  name,
  description,
  enabled,
  globallyDisabled,
  onToggle,
}: {
  name: string;
  description: string;
  enabled: boolean;
  globallyDisabled: boolean;
  onToggle: (enabled: boolean) => void;
}) {
  return (
    <label
      className={`flex items-center gap-2 px-2 py-1.5 rounded-md transition-colors ${
        globallyDisabled
          ? 'opacity-40 cursor-not-allowed'
          : 'hover:bg-muted/40 cursor-pointer'
      }`}
    >
      <input
        type="checkbox"
        checked={globallyDisabled ? false : enabled}
        disabled={globallyDisabled}
        onChange={(e) => onToggle(e.target.checked)}
        className="rounded text-primary focus:ring-primary/50 disabled:opacity-50"
        aria-label={`${name} を${enabled ? '無効' : '有効'}にする`}
      />
      <div className="flex-1 min-w-0">
        <div className="text-xs font-mono truncate">{name}</div>
        <div className="text-[10px] text-muted-foreground truncate">{description}</div>
      </div>
    </label>
  );
}
