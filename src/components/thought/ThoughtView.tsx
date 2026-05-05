import { useEffect, useState } from 'react';
import { Lightbulb, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useCharacterStore } from '../../stores';
import type { Thought } from '../../types';

export function ThoughtView() {
  const { selectedCharacterId, characters } = useCharacterStore();
  const [thoughts, setThoughts] = useState<Thought[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedCharacter = characters.find((c) => c.id === selectedCharacterId);

  useEffect(() => {
    if (!selectedCharacterId) return;
    loadThoughts();
  }, [selectedCharacterId]);

  const loadThoughts = async () => {
    if (!selectedCharacterId) return;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<Thought[]>('list_thoughts', {
        characterId: selectedCharacterId,
      });
      setThoughts(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  if (!selectedCharacterId) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-muted-foreground gap-3">
        <Lightbulb className="h-12 w-12" />
        <p className="text-sm">キャラクターを選択して思考履歴を表示</p>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden p-6">
      {/* Header */}
      <div className="flex items-center gap-2 mb-6">
        <Lightbulb className="w-5 h-5" />
        <h1 className="text-xl font-semibold">思考履歴</h1>
        {selectedCharacter && (
          <span className="text-sm text-muted-foreground">— {selectedCharacter.name}</span>
        )}
      </div>

      {/* Error */}
      {error && (
        <div className="mb-4 p-3 rounded-md bg-destructive/10 text-destructive text-sm">
          {error}
        </div>
      )}

      {/* Thought List */}
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
            <Loader2 className="w-4 h-4 animate-spin mr-2" />
            読み込み中...
          </div>
        ) : thoughts.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-32 text-muted-foreground gap-2">
            <Lightbulb className="w-8 h-8" />
            <p className="text-sm">思考がまだ生成されていない</p>
          </div>
        ) : (
          <div className="space-y-3">
            {thoughts.map((thought) => (
              <div
                key={thought.id}
                className="p-4 rounded-lg border border-border bg-card"
              >
                <p className="text-sm whitespace-pre-wrap">{thought.content}</p>
                {thought.context && (
                  <p className="mt-2 text-xs text-muted-foreground italic">
                    コンテキスト: {thought.context}
                  </p>
                )}
                <div className="mt-2 text-xs text-muted-foreground">
                  {new Date(thought.created_at).toLocaleString('ja-JP')}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
