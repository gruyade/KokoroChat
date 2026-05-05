import { useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useUIStore, useCharacterStore, useChatStore, useConfigStore } from './stores';
import { useChat } from './hooks/useChat';
import { useSpontaneousTimer } from './hooks/useSpontaneousTimer';
import { Sidebar } from './components/sidebar/Sidebar';
import { ChatView } from './components/chat/ChatView';
import { CharacterView } from './components/character/CharacterView';
import { SettingsView } from './components/settings/SettingsView';
import { DebugPanel } from './components/debug/DebugPanel';

const STORAGE_KEY = 'ai-character-chat-last-state';

interface SavedState {
  characterId: string | null;
  sessionId: string | null;
}

function MainContent() {
  const { activeView } = useUIStore();

  return (
    <main className="flex-1 flex flex-col overflow-hidden">
      {activeView === 'chat' && <ChatView />}
      {activeView === 'characters' && <CharacterView />}
      {activeView === 'settings' && <SettingsView />}
    </main>
  );
}

function App() {
  const { theme } = useUIStore();
  const { fetchCharacters, selectCharacter, selectedCharacterId } = useCharacterStore();
  const { selectSession, fetchHistory, fetchSessions, currentSessionId } = useChatStore();
  const { fetchConfig } = useConfigStore();
  const restoredRef = useRef(false);

  // Tauriイベントリスナーを設定
  useChat();

  // 自発的発話タイマー
  useSpontaneousTimer();

  // 初回マウント時: キャラクター一覧取得 + 設定取得 + 前回の状態を復元
  useEffect(() => {
    const restore = async () => {
      await fetchCharacters();
      await fetchConfig();

      try {
        const saved = localStorage.getItem(STORAGE_KEY);
        if (saved) {
          const state: SavedState = JSON.parse(saved);
          if (state.characterId) {
            selectCharacter(state.characterId);
            await fetchSessions(state.characterId);
            if (state.sessionId) {
              selectSession(state.sessionId);
              await fetchHistory(state.sessionId);
            }
          }
        }
      } catch {
        // 復元失敗は無視
      }
      restoredRef.current = true;
    };
    restore();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // キャラ・チャット変更時にlocalStorageに保存（復元完了後のみ）
  useEffect(() => {
    if (!restoredRef.current) return;
    const state: SavedState = {
      characterId: selectedCharacterId,
      sessionId: currentSessionId,
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  }, [selectedCharacterId, currentSessionId]);

  // キャラクター変更時に思考エンジンを起動/停止
  useEffect(() => {
    if (selectedCharacterId) {
      invoke('start_thought_engine', { characterId: selectedCharacterId }).catch((e) =>
        console.error('[thought] start_thought_engine error:', e)
      );
    } else {
      invoke('stop_thought_engine').catch((e) =>
        console.error('[thought] stop_thought_engine error:', e)
      );
    }
    return () => {
      invoke('stop_thought_engine').catch(() => {});
    };
  }, [selectedCharacterId]);

  // テーマ適用
  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'dark') {
      root.classList.add('dark');
    } else {
      root.classList.remove('dark');
    }
  }, [theme]);

  // デバッグモード判定（URLに?debug または localStorage）
  const isDebug = window.location.search.includes('debug') || localStorage.getItem('debug') === '1';

  return (
    <div className="h-screen flex bg-background text-foreground">
      <Sidebar />
      <MainContent />
      {isDebug && <DebugPanel />}
    </div>
  );
}

export default App;
