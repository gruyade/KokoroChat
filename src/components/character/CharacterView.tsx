import { useEffect, useState } from 'react';
import { Plus, Users } from 'lucide-react';
import { useCharacterStore } from '../../stores';
import { CharacterCard } from './CharacterCard';
import { CharacterForm } from './CharacterForm';
import type { Character } from '../../types';

export function CharacterView() {
  const {
    characters,
    loading,
    error,
    fetchCharacters,
    createCharacter,
    updateCharacter,
    deleteCharacter,
  } = useCharacterStore();

  const [showForm, setShowForm] = useState(false);
  const [editingCharacter, setEditingCharacter] = useState<Character | null>(null);

  useEffect(() => {
    fetchCharacters();
  }, [fetchCharacters]);

  const handleCreate = () => {
    setEditingCharacter(null);
    setShowForm(true);
  };

  const handleEdit = (character: Character) => {
    setEditingCharacter(character);
    setShowForm(true);
  };

  const handleDelete = async (id: string) => {
    if (!confirm('このキャラクターを削除すると、関連するチャット履歴・記憶・思考もすべて削除される。よろしいか？')) {
      return;
    }
    await deleteCharacter(id);
  };

  const handleSave = async (data: { name: string; description: string; system_prompt: string }) => {
    if (editingCharacter) {
      await updateCharacter(editingCharacter.id, {
        name: data.name,
        description: data.description,
        system_prompt: data.system_prompt,
      });
    } else {
      await createCharacter(data.name, data.description, data.system_prompt || undefined);
    }
    setShowForm(false);
    setEditingCharacter(null);
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-2">
          <Users className="w-5 h-5" />
          <h1 className="text-xl font-semibold">キャラクター管理</h1>
        </div>
        <button
          onClick={handleCreate}
          className="px-3 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 flex items-center gap-2 transition-colors"
        >
          <Plus className="w-4 h-4" />
          新規作成
        </button>
      </div>

      {/* Error */}
      {error && (
        <div className="mb-4 p-3 rounded-md bg-destructive/10 text-destructive text-sm">
          {error}
        </div>
      )}

      {/* Form */}
      {showForm && (
        <div className="mb-6 p-4 rounded-lg border border-border bg-card">
          <CharacterForm
            character={editingCharacter}
            onSave={handleSave}
            onCancel={() => {
              setShowForm(false);
              setEditingCharacter(null);
            }}
            loading={loading}
          />
        </div>
      )}

      {/* Character List */}
      <div className="flex-1 overflow-y-auto">
        {loading && characters.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
            読み込み中...
          </div>
        ) : characters.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-32 text-muted-foreground gap-2">
            <Users className="w-8 h-8" />
            <p className="text-sm">キャラクターがまだ作成されていない</p>
          </div>
        ) : (
          <div className="grid gap-3 grid-cols-1 md:grid-cols-2 lg:grid-cols-3">
            {characters.map((character) => (
              <CharacterCard
                key={character.id}
                character={character}
                onSelect={() => handleEdit(character)}
                onEdit={() => handleEdit(character)}
                onDelete={() => handleDelete(character.id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
