import { useEffect } from 'react';
import { useUIStore, useCharacterStore } from './stores';
import { useChat } from './hooks/useChat';
import { Sidebar } from './components/sidebar/Sidebar';
import { ChatView } from './components/chat/ChatView';
import { CharacterView } from './components/character/CharacterView';
import { SettingsView } from './components/settings/SettingsView';

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
  const { fetchCharacters } = useCharacterStore();

  // Tauriイベントリスナーを設定（chat:stream, spontaneous:message, tool:executing）
  useChat();

  // 初回マウント時にキャラクター一覧を取得
  useEffect(() => {
    fetchCharacters();
  }, [fetchCharacters]);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'dark') {
      root.classList.add('dark');
    } else {
      root.classList.remove('dark');
    }
  }, [theme]);

  return (
    <div className="h-screen flex bg-background text-foreground">
      <Sidebar />
      <MainContent />
    </div>
  );
}

export default App;
