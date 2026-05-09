import { useEffect, useState, useCallback } from 'react';
import { Lightbulb, Loader2, Trash2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCharacterStore } from '../../stores';
import { useOperationQueue } from '../../hooks/useOperationQueue';
import type { Thought } from '../../types';

export function SidebarThoughtList() {
  const { selectedCharacterId } = useCharacterStore();
  const [thoughts, setThoughts] = useState<Thought[]>([]);
  const [loading, setLoading] = useState(false);
  const [deletedIds, setDeletedIds] = useState<Set<string>>(new Set());
  const { processing, enqueue } = useOperationQueue();

  const loadThoughts = useCallback((characterId: string) => {
    setLoading(true);
    setDeletedIds(new Set());
    invoke<Thought[]>('get_thoughts', { characterId, limit: 20 })
      .then(setThoughts)
      .catch(() => setThoughts([]))
      .finally(() => setLoading(false));
  }, []);

  const handleDelete = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    if (deletedIds.has(id)) return;
    setDeletedIds((prev) => new Set(prev).add(id));
    enqueue(async () => {
      await invoke('delete_thought', { id }).catch(() => {});
    });
  };

  useEffect(() => {
    if (!selectedCharacterId) return;
    loadThoughts(selectedCharacterId);

    let cancelled = false;
    let unlisten: (() => void) | null = null;

    listen<{ character_id: string; thought: Thought }>('thought:generated', (event) => {
      if (cancelled) return;
      if (event.payload.character_id === selectedCharacterId) {
        setThoughts((prev) => [event.payload.thought, ...prev].slice(0, 20));
      }
    }).then((fn) => {
      if (cancelled) { fn(); return; }
      unlisten = fn;
    });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [selectedCharacterId, loadThoughts]);

  const visibleThoughts = thoughts.filter((t) => !deletedIds.has(t.id));

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

  if (visibleThoughts.length === 0) {
    return (
      <div className="p-3 flex flex-col items-center gap-2 text-muted-foreground">
        {processing ? (
          <>
            <Loader2 className="w-6 h-6 animate-spin" />
            <span className="text-xs">処理中...</span>
          </>
        ) : (
          <>
            <Lightbulb className="w-6 h-6" />
            <span className="text-xs">思考なし</span>
          </>
        )}
      </div>
    );
  }

  return (
    <div className="p-2 space-y-2">
      {visibleThoughts.map((thought) => (
        <div key={thought.id} className="group p-2.5 rounded-lg bg-muted/50 border border-border">
          <div className="flex items-start justify-between gap-1">
            <p className="text-xs leading-relaxed flex-1">{thought.content}</p>
            <button
              onClick={(e) => handleDelete(e, thought.id)}
              className="p-0.5 rounded opacity-0 group-hover:opacity-100 hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-all shrink-0"
              title="削除"
            >
              <Trash2 className="w-3 h-3" />
            </button>
          </div>
          {thought.context && (
            <p className="text-xs text-muted-foreground mt-1 italic">{thought.context}</p>
          )}
          <div className="text-xs text-muted-foreground/70 mt-1">
            {new Date(thought.created_at).toLocaleString('ja-JP', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
          </div>
        </div>
      ))}
    </div>
  );
}
