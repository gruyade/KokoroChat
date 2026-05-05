import { useState, useEffect } from 'react';
import { Save, X, Wand2, Sparkles, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
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
  const [generating, setGenerating] = useState(false);
  const [improving, setImproving] = useState(false);

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

  const handleGenerate = async () => {
    if (!name.trim() || !description.trim()) return;
    setGenerating(true);
    try {
      const prompt = await invoke<string>('generate_system_prompt', {
        name: name.trim(),
        description: description.trim(),
      });
      setSystemPrompt(prompt);
    } catch (e) {
      alert(`生成エラー: ${e}`);
    } finally {
      setGenerating(false);
    }
  };

  const handleImprove = async () => {
    if (!name.trim() || !systemPrompt.trim()) return;
    setImproving(true);
    try {
      const improved = await invoke<string>('improve_system_prompt', {
        name: name.trim(),
        description: description.trim(),
        currentPrompt: systemPrompt,
      });
      setSystemPrompt(improved);
    } catch (e) {
      alert(`改善エラー: ${e}`);
    } finally {
      setImproving(false);
    }
  };

  const isEditing = !!character;
  const isProcessing = generating || improving || loading;

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
          placeholder="キャラクターの概要説明（性格、背景など）"
          rows={3}
          className="w-full px-3 py-2 rounded-md border border-border bg-background text-sm resize-none focus:outline-none focus:ring-2 focus:ring-primary"
        />
      </div>

      {/* System Prompt */}
      <div>
        <div className="flex items-center justify-between mb-1">
          <label htmlFor="char-prompt" className="text-sm font-medium">
            System Prompt
          </label>
          <div className="flex gap-1">
            {/* 生成ボタン */}
            <button
              type="button"
              onClick={handleGenerate}
              disabled={isProcessing || !name.trim() || !description.trim()}
              className="px-2 py-1 text-xs rounded-md bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1 transition-colors"
              title="説明内容からSystem Promptを生成"
            >
              {generating ? <Loader2 className="w-3 h-3 animate-spin" /> : <Wand2 className="w-3 h-3" />}
              生成
            </button>
            {/* 改善ボタン */}
            <button
              type="button"
              onClick={handleImprove}
              disabled={isProcessing || !name.trim() || !systemPrompt.trim()}
              className="px-2 py-1 text-xs rounded-md bg-accent text-accent-foreground hover:bg-accent/80 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1 transition-colors"
              title="現在のSystem Promptを改善"
            >
              {improving ? <Loader2 className="w-3 h-3 animate-spin" /> : <Sparkles className="w-3 h-3" />}
              改善
            </button>
          </div>
        </div>
        <textarea
          id="char-prompt"
          value={systemPrompt}
          onChange={(e) => setSystemPrompt(e.target.value)}
          placeholder="キャラクターのSystem Prompt（生成ボタンで自動作成可能）"
          rows={10}
          className="w-full px-3 py-2 rounded-md border border-border bg-background text-sm font-mono resize-y focus:outline-none focus:ring-2 focus:ring-primary"
        />
        <p className="text-xs text-muted-foreground mt-1">
          「生成」: 概要から新規作成 / 「改善」: 現在の内容をLLMで改良
        </p>
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
          disabled={!name.trim() || isProcessing}
          className="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2 transition-colors"
        >
          <Save className="w-4 h-4" />
          {loading ? '保存中...' : '保存'}
        </button>
      </div>
    </form>
  );
}
