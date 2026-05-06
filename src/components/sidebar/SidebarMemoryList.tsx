import { useEffect, useState, useCallback } from 'react';
import { Brain, Loader2, Trash2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useCharacterStore } from '../../stores';
import { useOperationQueue } from '../../hooks/useOperationQueue';
import type { Memory } from '../../types';

export function SidebarMemoryList() {
  const { selectedCharacterId } = useCharacterStore();
  const [memories, setMemories] = useState<Memory[]>([]);
  const [loading, setLoading] = useState(false);
  const [deletedIds, setDeletedIds] = useState<Set<string>>(new Set());
  const { processing, enqueue } = useOperationQueue();

  const loadMemories = useCallback(() => {
    if (!selectedCharacterId) return;
    setLoading(true);
    setDeletedIds(new Set());
    invoke<Memory[]>('list_memories', { characterId: selectedCharacterId })
      .then(setMemories)
      .catch(() => setMemories([]))
      .finally(() => setLoading(false));
  }, [selectedCharacterId]);

  useEffect(() => {
    if (!selectedCharacterId) return;
    loadMemories();
  }, [selectedCharacterId, loadMemories]);

  const handleDelete = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    if (deletedIds.has(id)) return;
    setDeletedIds((prev) => new Set(prev).add(id));
    enqueue(async () => {
      await invoke('delete_memory', { id }).catch(() => {});
    });
  };

  const visibleMemories = memories.filter((m) => !deletedIds.has(m.id));

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

  if (visibleMemories.length === 0) {
    return (
      <div className="p-3 flex flex-col items-center gap-2 text-muted-foreground">
        {processing ? (
          <>
            <Loader2 className="w-6 h-6 animate-spin" />
            <span className="text-xs">処理中...</span>
          </>
        ) : (
          <>
            <Brain className="w-6 h-6" />
            <span className="text-xs">記憶なし</span>
          </>
        )}
      </div>
    );
  }

  return (
    <div className="p-2 space-y-2">
      {visibleMemories.map((memory) => (
        <div key={memory.id} className="group p-2.5 rounded-lg bg-muted/50 border border-border">
          <div className="flex items-start justify-between gap-1">
            <p className="text-xs leading-relaxed flex-1">{memory.content}</p>
            <button
              onClick={(e) => handleDelete(e, memory.id)}
              className="p-0.5 rounded opacity-0 group-hover:opacity-100 hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-all shrink-0"
              title="削除"
            >
              <Trash2 className="w-3 h-3" />
            </button>
          </div>
          <div className="text-xs text-muted-foreground/70 mt-1">
            {new Date(memory.updated_at).toLocaleString('ja-JP', { month: 'short', day: 'numeric' })}
            {memory.source_session_id && ' · セッション由来'}
          </div>
        </div>
      ))}
    </div>
  );
}
