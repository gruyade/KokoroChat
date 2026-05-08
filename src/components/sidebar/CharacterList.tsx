import { Plus, User } from 'lucide-react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useCharacterStore } from '../../stores';

export function CharacterList() {
  const { characters, selectedCharacterId, selectCharacter } = useCharacterStore();

  return (
    <div className="flex flex-col gap-1 px-2">
      <button
        className="flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium text-muted-foreground hover:bg-accent hover:text-accent-foreground"
      >
        <Plus className="h-4 w-4" />
        <span>新規キャラクター</span>
      </button>

      <div className="flex flex-col gap-0.5">
        {characters.map((character) => (
          <button
            key={character.id}
            onClick={() => selectCharacter(character.id)}
            className={`flex items-center gap-2 rounded-md px-3 py-2 text-sm text-left transition-colors ${
              selectedCharacterId === character.id
                ? 'bg-accent text-accent-foreground'
                : 'text-muted-foreground hover:bg-accent/50 hover:text-accent-foreground'
            }`}
          >
            {character.avatar_path ? (
              <img
                src={convertFileSrc(character.avatar_path)}
                alt={character.name}
                className="h-5 w-5 rounded-full object-cover shrink-0"
              />
            ) : (
              <User className="h-4 w-4 shrink-0" />
            )}
            <div className="flex flex-col min-w-0">
              <span className="truncate font-medium">{character.name}</span>
              {character.description && (
                <span className="truncate text-xs text-muted-foreground">
                  {character.description}
                </span>
              )}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
