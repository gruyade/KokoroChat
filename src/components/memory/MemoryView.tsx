import { useEffect, useState, useCallback } from 'react';
import { Brain, Trash2, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCharacterStore } from '../../stores';
import { useOperationQueue } from '../../hooks/useOperationQueue';
import type { Memory } from '../../types';

export function MemoryView() {
  const { selectedCharacterId, characters } = useCharacterStore();
  const [memories, setMemories] = useState<Memory[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [deletedIds, setDeletedIds] = useState<Set<string>>(new Set());
  const { processing, enqueue } = useOperationQueue();

  const selectedCharacter = characters.find((c) => c.id === selectedCharacterId);

  const loadMemories = useCallback(async () => {
    if (!selectedCharacterId) return;
    setLoading(true);
    setError(null);
    setDeletedIds(new Set());
    try {
      const result = await invoke<Memory[]>('list_memories', {
        characterId: selectedCharacterId,
      });
      setMemories(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [selectedCharacterId]);

  useEffect(() => {
    if (!selectedCharacterId) return;
    loadMemories();
  }, [selectedCharacterId, loadMemories]);

  // memory:generated イベントをリッスンし、該当キャラクターの記憶を即時反映
  useEffect(() => {
    if (!selectedCharacterId) return;

    const unlisten = listen<{ character_id: string }>('memory:generated', (event) => {
      if (event.payload.character_id === selectedCharacterId) {
        loadMemories();
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [selectedCharacterId, loadMemories]);

  const handleDelete = (id: string) => {
    if (deletedIds.has(id)) return;
    setDeletedIds((prev) => new Set(prev).add(id));
    enqueue(async () => {
      try {
        await invoke('delete_memory', { id });
      } catch (e) {
        setError(String(e));
      }
    });
  };

  const visibleMemories = memories.filter((m) => !deletedIds.has(m.id));

  if (!selectedCharacterId) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground gap-3">
        <Brain className="h-12 w-12" />
        <p className="text-sm">キャラクターを選択して記憶を表示</p>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden p-6">
      {/* Header */}
      <div className="flex items-center gap-2 mb-6">
        <Brain className="w-5 h-5" />
        <h1 className="text-xl font-semibold">記憶一覧</h1>
        {selectedCharacter && (
          <span className="text-sm text-muted-foreground">— {selectedCharacter.name}</span>
        )}
        {processing && (
          <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />
        )}
      </div>

      {/* Error */}
      {error && (
        <div className="mb-4 p-3 rounded-md bg-destructive/10 text-destructive text-sm">
          {error}
        </div>
      )}

      {/* Memory List */}
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
            <Loader2 className="w-4 h-4 animate-spin mr-2" />
            読み込み中...
          </div>
        ) : visibleMemories.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-32 text-muted-foreground gap-2">
            <Brain className="w-8 h-8" />
            <p className="text-sm">記憶がまだ生成されていない</p>
          </div>
        ) : (
          <div className="space-y-3">
            {visibleMemories.map((memory) => (
              <div
                key={memory.id}
                className="p-4 rounded-lg border border-border bg-card group"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="flex-1 min-w-0">
                    <p className="text-sm whitespace-pre-wrap">{memory.content}</p>
                    <div className="mt-2 flex items-center gap-3 text-xs text-muted-foreground">
                      <span>{new Date(memory.created_at).toLocaleString('ja-JP')}</span>
                      {memory.source_session_id && <span>セッション由来</span>}
                    </div>
                  </div>
                  <button
                    onClick={() => handleDelete(memory.id)}
                    className="p-1.5 rounded opacity-0 group-hover:opacity-100 hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-all"
                    aria-label="削除"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
