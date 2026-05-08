import { useState, useRef, useEffect } from 'react';
import { ChevronDown } from 'lucide-react';
import { useCharacterStore } from '../../stores';
import { AvatarImage } from '../common/AvatarImage';

export function CharacterSelector() {
  const { characters, selectedCharacterId, selectCharacter } = useCharacterStore();
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const selectedCharacter = characters.find((c) => c.id === selectedCharacterId);

  // 外側クリックで閉じる
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    if (open) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [open]);

  const handleSelect = (id: string) => {
    selectCharacter(id);
    setOpen(false);
  };

  const getInitial = (name: string) => name.charAt(0);

  if (characters.length === 0) {
    return (
      <div className="p-3 border-b border-border">
        <div className="flex items-center gap-2 p-2 rounded-lg bg-muted text-muted-foreground text-sm">
          キャラクター未作成
        </div>
      </div>
    );
  }

  return (
    <div className="p-3 border-b border-border" ref={containerRef}>
      {/* 選択中キャラクター表示 */}
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 p-2 rounded-lg bg-muted cursor-pointer hover:bg-accent transition-colors"
      >
        <div className="w-8 h-8 rounded-full bg-primary/20 flex items-center justify-center text-primary text-sm font-bold shrink-0 overflow-hidden">
          {selectedCharacter?.avatar_path ? (
            <AvatarImage avatarPath={selectedCharacter.avatar_path} alt={selectedCharacter.name} className="w-full h-full object-cover" />
          ) : (
            selectedCharacter ? getInitial(selectedCharacter.name) : '?'
          )}
        </div>
        <div className="flex-1 min-w-0 text-left">
          <div className="text-sm font-medium truncate text-foreground">
            {selectedCharacter?.name ?? 'キャラクターを選択'}
          </div>
          {selectedCharacter?.description && (
            <div className="text-xs text-muted-foreground truncate">
              {selectedCharacter.description}
            </div>
          )}
        </div>
        <ChevronDown
          className={`w-4 h-4 text-muted-foreground transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </button>

      {/* ドロップダウンリスト */}
      {open && (
        <div className="mt-1 rounded-lg bg-card border border-border shadow-lg overflow-hidden max-h-60 overflow-y-auto">
          {characters.map((character) => (
            <button
              key={character.id}
              onClick={() => handleSelect(character.id)}
              className={`w-full flex items-center gap-2 px-3 py-2 text-left transition-colors ${
                selectedCharacterId === character.id
                  ? 'bg-accent text-accent-foreground'
                  : 'hover:bg-muted text-foreground'
              }`}
            >
              <div className="w-7 h-7 rounded-full bg-primary/20 flex items-center justify-center text-primary text-xs font-bold shrink-0 overflow-hidden">
                {character.avatar_path ? (
                  <AvatarImage avatarPath={character.avatar_path} alt={character.name} className="w-full h-full object-cover" />
                ) : (
                  getInitial(character.name)
                )}
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium truncate">{character.name}</div>
                {character.description && (
                  <div className="text-xs text-muted-foreground truncate">
                    {character.description}
                  </div>
                )}
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
