import { useEffect, useState } from 'react';
import { X, Wrench, Lock, ChevronRight, BookOpen } from 'lucide-react';
import { usePluginStore } from '../../stores/plugin.store';
import { useChatStore } from '../../stores/chat.store';
import { useKnowledgeStore } from '../../stores/knowledge.store';
import { FileOpsDirectoryManager } from './FileOpsDirectoryManager';
import { KnowledgeSection } from './KnowledgeSection';

interface ToolManagementPaneProps {
  onClose: () => void;
}

/**
 * チャット画面右側に表示される開閉可能なツール管理ペイン。
 * 各プラグインをアコーディオンで折り畳み表示し、セッション単位でツールの有効/無効を切り替え可能。
 * グローバルで無効化されたツールはグレーアウト（選択不可）。
 */
export function ToolManagementPane({ onClose }: ToolManagementPaneProps) {
  const { plugins, fetchPlugins, loading, sessionPermissions, setSessionToolEnabled, initSessionPermissions } =
    usePluginStore();
  const currentSessionId = useChatStore((s) => s.currentSessionId);
  const { entries: knowledgeEntries, fetchEntries: fetchKnowledgeEntries } = useKnowledgeStore();
  const [expandedPlugins, setExpandedPlugins] = useState<Record<string, boolean>>({});

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

  // ナレッジエントリを取得
  useEffect(() => {
    if (currentSessionId) {
      fetchKnowledgeEntries(currentSessionId);
    }
  }, [currentSessionId, fetchKnowledgeEntries]);

  const permissions = currentSessionId ? (sessionPermissions[currentSessionId] ?? {}) : {};

  const handleToggle = (toolName: string, enabled: boolean) => {
    if (!currentSessionId) return;
    setSessionToolEnabled(currentSessionId, toolName, enabled);
  };

  const toggleExpanded = (pluginName: string) => {
    setExpandedPlugins((prev) => ({ ...prev, [pluginName]: !prev[pluginName] }));
  };

  return (
    <div className="w-full border-l border-border bg-background flex flex-col h-full overflow-hidden">
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
      <div className="flex-1 overflow-y-auto p-3 space-y-1">
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
          <>
          {plugins.map((plugin) => {
            const isExpanded = expandedPlugins[plugin.name] ?? false;

            return (
              <div key={plugin.name} className="rounded-md border border-border/50 overflow-hidden">
                {/* Accordion header */}
                <button
                  onClick={() => toggleExpanded(plugin.name)}
                  className="w-full flex items-center gap-1.5 px-3 py-2 hover:bg-muted/40 transition-colors text-left"
                  aria-expanded={isExpanded}
                  aria-label={`${plugin.name} を${isExpanded ? '閉じる' : '開く'}`}
                >
                  <ChevronRight
                    className={`w-3 h-3 text-muted-foreground transition-transform ${
                      isExpanded ? 'rotate-90' : ''
                    }`}
                  />
                  <span className="text-xs font-medium text-foreground">
                    {plugin.name}
                  </span>
                  <span className="text-[10px] text-muted-foreground ml-auto">
                    {plugin.tools.length} tools
                  </span>
                  {!plugin.enabled && (
                    <Lock className="w-3 h-3 text-muted-foreground/60 ml-1" aria-label="グローバルで無効" />
                  )}
                </button>

                {/* Accordion content */}
                {isExpanded && (
                  <div className="px-2 pb-2 space-y-1 border-t border-border/30">
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

                    {/* file_ops の場合はディレクトリ管理UIを内包 */}
                    {plugin.name === 'file_ops' && currentSessionId && (
                      <div className="mt-2 pt-2 border-t border-border/30">
                        <FileOpsDirectoryManager sessionId={currentSessionId} />
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}

          {/* Knowledge Plugin アコーディオン */}
          {currentSessionId && (() => {
            const isKnowledgeExpanded = expandedPlugins['__knowledge__'] ?? false;
            return (
              <div className="rounded-md border border-border/50 overflow-hidden">
                <button
                  onClick={() => toggleExpanded('__knowledge__')}
                  className="w-full flex items-center gap-1.5 px-3 py-2 hover:bg-muted/40 transition-colors text-left"
                  aria-expanded={isKnowledgeExpanded}
                  aria-label={`ナレッジ を${isKnowledgeExpanded ? '閉じる' : '開く'}`}
                >
                  <ChevronRight
                    className={`w-3 h-3 text-muted-foreground transition-transform ${
                      isKnowledgeExpanded ? 'rotate-90' : ''
                    }`}
                  />
                  <BookOpen className="w-3 h-3 text-muted-foreground" />
                  <span className="text-xs font-medium text-foreground">
                    ナレッジ
                  </span>
                  {knowledgeEntries.length > 0 && (
                    <span className="ml-auto text-[10px] bg-primary/15 text-primary px-1.5 py-0.5 rounded-full font-medium">
                      {knowledgeEntries.length}
                    </span>
                  )}
                </button>

                {isKnowledgeExpanded && (
                  <div className="border-t border-border/30">
                    <KnowledgeSection sessionId={currentSessionId} />
                  </div>
                )}
              </div>
            );
          })()}
          </>
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
