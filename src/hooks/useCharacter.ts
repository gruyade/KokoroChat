import { useEffect } from 'react';
import { useCharacterStore } from '../stores/character.store';

/**
 * キャラクター操作Hook
 * - マウント時にキャラクター一覧を取得
 * - CRUD操作を提供
 */
export function useCharacter() {
  const characters = useCharacterStore((s) => s.characters);
  const loading = useCharacterStore((s) => s.loading);
  const error = useCharacterStore((s) => s.error);
  const fetchCharacters = useCharacterStore((s) => s.fetchCharacters);
  const createCharacter = useCharacterStore((s) => s.createCharacter);
  const updateCharacter = useCharacterStore((s) => s.updateCharacter);
  const deleteCharacter = useCharacterStore((s) => s.deleteCharacter);
  const selectCharacter = useCharacterStore((s) => s.selectCharacter);
  const selectedCharacterId = useCharacterStore((s) => s.selectedCharacterId);

  useEffect(() => {
    fetchCharacters();
  }, [fetchCharacters]);

  return {
    characters,
    loading,
    error,
    selectedCharacterId,
    createCharacter,
    updateCharacter,
    deleteCharacter,
    selectCharacter,
  };
}
