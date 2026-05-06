import { useEffect, useState } from 'react';
import { Brain, Loader2, Trash2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useCharacterStore } from '../../stores';
import { useOperationQueue } from '../../hooks/useOperationQueue';
import type { Memory } from '../../types';

export function SidebarMemoryList() {
  const { selectedCharacterId } = useCharacterStore();
  const [memories, setMemories] = useState<Memory[]>([]);
  const [loading, setLoading] = useState(false);
  const { pendingIds, processing, enqueue } = useOperationQueue();

  useEffect(() => {
    if (!selectedCharacterId) return;
    loadMemories();
  }, [selectedCharacterId]);

  const loadMemories = () => {
    if (!selectedCharacterId) return;
    setLoading(true);
    invoke<Memory[]>('list_memories', { characterId: selectedCharacterId })
      .then(setMemories)
      .catch(() => setMemories([]))
      .finally(() => setLoading(false));
  };

  const handleDelete = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    if (pendingIds.has(id)) return;
    enqueue(id, async () => {
      try {
        await invoke('delete_memory', { id });
        setMemories((prev) => prev.filter((m) => m.id !== id));
      } catch {
        // 失敗時はリストから除去しない
      }
    });
  };

  if (!selectedCharacterId) {
    return <div className="p-3 text-xs text-muted-foreground">キャラクターを選択してください</div>;
  }

  if (loading) {
    return (
      <div className="p-3 flex items-center gap-2 text-xs text-muted-foreground">
        <Loader2 className="w-3 h-3 animate-spin" />読み込み中...
      </div>
    );
  }

  if (memories.length === 0 && !processing) {
    return (
      <div className="p-3 flex flex-col items-center gap-2 text-muted-foreground">
        <Brain className="w-6 h-6" />
        <span className="text-xs">記憶なし</span>
      </div>
    );
  }

  return (
    <div className="p-2 space-y-2">
      {processing && (
        <div className="flex items-center gap-1.5 px-2 py-1 text-xs text-muted-foreground">
          <Loader2 className="w-3 h-3 animate-spin" />
          <span>処理中...</span>
        </div>
      )}
      {memories.map((memory) => {
        const isPending = pendingIds.has(memory.id);
        return (
          <div
            key={memory.id}
            className={`group p-2.5 rounded-lg bg-muted/50 border border-border transition-opacity ${isPending ? 'opacity-40 pointer-events-none' : ''}`}
          >
            <div className="flex items-start justify-between gap-1">
              <p className="text-xs leading-relaxed flex-1">{memory.content}</p>
              {isPending ? (
                <Loader2 className="w-3 h-3 animate-spin text-muted-foreground shrink-0" />
              ) : (
                <button
                  onClick={(e) => handleDelete(e, memory.id)}
                  className="p-0.5 rounded opacity-0 group-hover:opacity-100 hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-all shrink-0"
                  title="削除"
                >
                  <Trash2 className="w-3 h-3" />
                </button>
              )}
            </div>
            <div className="text-xs text-muted-foreground/70 mt-1">
              {new Date(memory.updated_at).toLocaleString('ja-JP', { month: 'short', day: 'numeric' })}
              {memory.source_session_id && ' · セッション由来'}
            </div>
          </div>
        );
      })}
    </div>
  );
}
