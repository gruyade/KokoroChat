import { useState } from 'react';
import { User, Trash2, Edit, Download } from 'lucide-react';
import type { Character } from '../../types';
import { ExportDialog } from './ExportDialog';
import { AvatarImage } from '../common/AvatarImage';

interface CharacterCardProps {
  character: Character;
  selected?: boolean;
  onSelect: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

export function CharacterCard({
  character,
  selected,
  onSelect,
  onEdit,
  onDelete,
}: CharacterCardProps) {
  const [showExportDialog, setShowExportDialog] = useState(false);

  return (
    <>
      <div
        onClick={onSelect}
        className={`p-4 rounded-lg border cursor-pointer transition-colors ${
          selected
            ? 'border-primary bg-primary/5'
            : 'border-border hover:border-primary/50 bg-card'
        }`}
      >
        <div className="flex items-start gap-3">
          {/* Avatar */}
          <div className="w-10 h-10 rounded-full bg-muted flex items-center justify-center flex-shrink-0 overflow-hidden">
            {character.avatar_path ? (
              <AvatarImage
                avatarPath={character.avatar_path}
                alt={character.name}
                className="w-10 h-10 rounded-full object-cover"
              />
            ) : (
              <User className="w-5 h-5 text-muted-foreground" />
            )}
          </div>

          {/* Info */}
          <div className="flex-1 min-w-0">
            <h3 className="font-medium text-sm truncate">{character.name}</h3>
            <p className="text-xs text-muted-foreground mt-1 line-clamp-2">
              {character.description}
            </p>
          </div>

          {/* Actions */}
          <div className="flex gap-1 flex-shrink-0">
            <button
              onClick={(e) => {
                e.stopPropagation();
                setShowExportDialog(true);
              }}
              className="p-1.5 rounded hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
              aria-label="エクスポート"
            >
              <Download className="w-4 h-4" />
            </button>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onEdit();
              }}
              className="p-1.5 rounded hover:bg-muted text-muted-foreground hover:text-foreground transition-colors"
              aria-label="編集"
            >
              <Edit className="w-4 h-4" />
            </button>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onDelete();
              }}
              className="p-1.5 rounded hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-colors"
              aria-label="削除"
            >
              <Trash2 className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      <ExportDialog
        characterId={character.id}
        characterName={character.name}
        isOpen={showExportDialog}
        onClose={() => setShowExportDialog(false)}
      />
    </>
  );
}
