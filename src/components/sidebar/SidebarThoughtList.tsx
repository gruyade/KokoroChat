import { useEffect, useState } from 'react';
import { Lightbulb, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useCharacterStore } from '../../stores';
import type { Thought } from '../../types';

export function SidebarThoughtList() {
  const { selectedCharacterId } = useCharacterStore();
  const [thoughts, setThoughts] = useState<Thought[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!selectedCharacterId) return;
    setLoading(true);
    invoke<Thought[]>('get_thoughts', { characterId: selectedCharacterId, limit: 20 })
      .then(setThoughts)
      .catch(() => setThoughts([]))
      .finally(() => setLoading(false));
  }, [selectedCharacterId]);

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

  if (thoughts.length === 0) {
    return (
      <div className="p-3 flex flex-col items-center gap-2 text-muted-foreground">
        <Lightbulb className="w-6 h-6" />
        <span className="text-xs">思考なし</span>
      </div>
    );
  }

  return (
    <div className="p-2 space-y-2">
      {thoughts.map((thought) => (
        <div key={thought.id} className="p-2.5 rounded-lg bg-muted/50 border border-border">
          <p className="text-xs leading-relaxed">{thought.content}</p>
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
