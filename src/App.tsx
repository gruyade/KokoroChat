import { useEffect } from 'react';
import { useUIStore } from './stores';
import { Sidebar } from './components/sidebar/Sidebar';
import { ChatView } from './components/chat/ChatView';

function MainContent() {
  const { activeView } = useUIStore();

  return (
    <main className="flex-1 flex flex-col overflow-hidden">
      {activeView === 'chat' && <ChatView />}
      {activeView === 'characters' && (
        <div className="flex-1 flex items-center justify-center text-muted-foreground">
          <p>キャラクター管理画面（未実装）</p>
        </div>
      )}
      {activeView === 'settings' && (
        <div className="flex-1 flex items-center justify-center text-muted-foreground">
          <p>設定画面（未実装）</p>
        </div>
      )}
      {activeView === 'plugins' && (
        <div className="flex-1 flex items-center justify-center text-muted-foreground">
          <p>プラグイン管理画面（未実装）</p>
        </div>
      )}
      {activeView === 'memory' && (
        <div className="flex-1 flex items-center justify-center text-muted-foreground">
          <p>記憶閲覧画面（未実装）</p>
        </div>
      )}
      {activeView === 'thought' && (
        <div className="flex-1 flex items-center justify-center text-muted-foreground">
          <p>思考閲覧画面（未実装）</p>
        </div>
      )}
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
