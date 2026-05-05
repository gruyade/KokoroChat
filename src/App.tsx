import { useEffect } from 'react';
import { useUIStore } from './stores';
import { Sidebar } from './components/sidebar/Sidebar';
import { ChatView } from './components/chat/ChatView';
import { CharacterView } from './components/character/CharacterView';
import { SettingsView } from './components/settings/SettingsView';
import { PluginListView } from './components/plugin/PluginListView';
import { MemoryView } from './components/memory/MemoryView';
import { ThoughtView } from './components/thought/ThoughtView';

function MainContent() {
  const { activeView } = useUIStore();

  return (
    <main className="flex-1 flex flex-col overflow-hidden">
      {activeView === 'chat' && <ChatView />}
      {activeView === 'characters' && <CharacterView />}
      {activeView === 'settings' && <SettingsView />}
      {activeView === 'plugins' && <PluginListView />}
      {activeView === 'memory' && <MemoryView />}
      {activeView === 'thought' && <ThoughtView />}
    </main>
  );
}

function App() {
  const { theme } = useUIStore();

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
