import { useState, useEffect } from 'react';
import { Save, X } from 'lucide-react';
import type { Character } from '../../types';

interface CharacterFormProps {
  character?: Character | null;
  onSave: (data: { name: string; description: string; system_prompt: string }) => void;
  onCancel: () => void;
  loading?: boolean;
}

export function CharacterForm({ character, onSave, onCancel, loading }: CharacterFormProps) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [systemPrompt, setSystemPrompt] = useState('');

  useEffect(() => {
    if (character) {
      setName(character.name);
      setDescription(character.description);
      setSystemPrompt(character.system_prompt);
    } else {
      setName('');
      setDescription('');
      setSystemPrompt('');
    }
  }, [character]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    onSave({ name: name.trim(), description: description.trim(), system_prompt: systemPrompt });
  };

  const isEditing = !!character;

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">
          {isEditing ? 'キャラクター編集' : '新規キャラクター作成'}
        </h2>
        <button
          type="button"
          onClick={onCancel}
          className="p-1.5 rounded hover:bg-muted text-muted-foreground"
          aria-label="キャンセル"
        >
          <X className="w-5 h-5" />
        </button>
      </div>

      {/* Name */}
      <div>
        <label htmlFor="char-name" className="block text-sm font-medium mb-1">
          名前
        </label>
        <input
          id="char-name"
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="キャラクター名"
          className="w-full px-3 py-2 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-primary"
          required
        />
      </div>

      {/* Description */}
      <div>
        <label htmlFor="char-desc" className="block text-sm font-medium mb-1">
          概要
        </label>
        <textarea
          id="char-desc"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="キャラクターの概要説明"
          rows={3}
          className="w-full px-3 py-2 rounded-md border border-border bg-background text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary"
        />
      </div>

      {/* System Prompt */}
      <div>
        <label htmlFor="char-prompt" className="block text-sm font-medium mb-1">
          System Prompt
        </label>
        <textarea
          id="char-prompt"
          value={systemPrompt}
          onChange={(e) => setSystemPrompt(e.target.value)}
          placeholder="キャラクターのSystem Prompt（空の場合はLLMが自動生成）"
          rows={8}
          className="w-full px-3 py-2 rounded-md border border-border bg-background text-sm font-mono resize-y focus:outline-none focus:ring-2 focus:ring-primary"
        />
      </div>

      {/* Actions */}
      <div className="flex gap-2 justify-end">
        <button
          type="button"
          onClick={onCancel}
          className="px-4 py-2 text-sm rounded-md border border-border hover:bg-muted transition-colors"
        >
          キャンセル
        </button>
        <button
          type="submit"
          disabled={!name.trim() || loading}
          className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2 transition-colors"
        >
          <Save className="w-4 h-4" />
          {loading ? '保存中...' : '保存'}
        </button>
      </div>
    </form>
  );
}
