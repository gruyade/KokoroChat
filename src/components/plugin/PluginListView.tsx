import { useEffect } from 'react';
import { Puzzle, Loader2 } from 'lucide-react';
import { usePluginStore } from '../../stores';
import { PluginCard } from './PluginCard';

export function PluginListView() {
  const { plugins, loading, error, fetchPlugins, enablePlugin, disablePlugin } = usePluginStore();

  useEffect(() => {
    fetchPlugins();
  }, [fetchPlugins]);

  const handleToggle = async (name: string, enabled: boolean) => {
    if (enabled) {
      await enablePlugin(name);
    } else {
      await disablePlugin(name);
    }
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden p-6">
      {/* Header */}
      <div className="flex items-center gap-2 mb-6">
        <Puzzle className="w-5 h-5" />
        <h1 className="text-xl font-semibold">プラグイン管理</h1>
      </div>

      {/* Error */}
      {error && (
        <div className="mb-4 p-3 rounded-md bg-destructive/10 text-destructive text-sm">
          {error}
        </div>
      )}

      {/* Plugin List */}
      <div className="flex-1 overflow-y-auto">
        {loading && plugins.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
            <Loader2 className="w-4 h-4 animate-spin mr-2" />
            読み込み中...
          </div>
        ) : plugins.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-32 text-muted-foreground gap-2">
            <Puzzle className="w-8 h-8" />
            <p className="text-sm">プラグインが登録されていない</p>
          </div>
        ) : (
          <div className="grid gap-3 grid-cols-1 lg:grid-cols-2">
            {plugins.map((plugin) => (
              <PluginCard
                key={plugin.name}
                plugin={plugin}
                onToggle={(enabled) => handleToggle(plugin.name, enabled)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
