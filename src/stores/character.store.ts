import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Character, CharacterUpdate } from '../types';

interface CharacterState {
  characters: Character[];
  selectedCharacterId: string | null;
  loading: boolean;
  error: string | null;
  fetchCharacters: () => Promise<void>;
  createCharacter: (name: string, description: string) => Promise<Character>;
  updateCharacter: (id: string, updates: CharacterUpdate) => Promise<void>;
  deleteCharacter: (id: string) => Promise<void>;
  selectCharacter: (id: string | null) => void;
}

export const useCharacterStore = create<CharacterState>((set, get) => ({
  characters: [],
  selectedCharacterId: null,
  loading: false,
  error: null,

  fetchCharacters: async () => {
    set({ loading: true, error: null });
    try {
      const characters = await invoke<Character[]>('list_characters');
      set({ characters, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  createCharacter: async (name: string, description: string) => {
    set({ loading: true, error: null });
    try {
      const character = await invoke<Character>('create_character', { name, description });
      set((state) => ({
        characters: [...state.characters, character],
        loading: false,
      }));
      return character;
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  updateCharacter: async (id: string, updates: CharacterUpdate) => {
    set({ loading: true, error: null });
    try {
      await invoke('update_character', { id, updates });
      const { characters } = get();
      set({
        characters: characters.map((c) =>
          c.id === id ? { ...c, ...updates, updated_at: new Date().toISOString() } : c
        ),
        loading: false,
      });
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  deleteCharacter: async (id: string) => {
    set({ loading: true, error: null });
    try {
      await invoke('delete_character', { id });
      const { characters, selectedCharacterId } = get();
      set({
        characters: characters.filter((c) => c.id !== id),
        selectedCharacterId: selectedCharacterId === id ? null : selectedCharacterId,
        loading: false,
      });
    } catch (e) {
      set({ error: String(e), loading: false });
      throw e;
    }
  },

  selectCharacter: (id: string | null) => {
    set({ selectedCharacterId: id });
  },
}));
