import { MessageSquare, Users, Settings } from 'lucide-react';
import { useUIStore } from '../../stores';
import { CharacterSelector } from './CharacterSelector';
import { ChatList } from './ChatList';
import { SidebarThoughtList } from './SidebarThoughtList';
import { SidebarMemoryList } from './SidebarMemoryList';

const sidebarTabs = [
  { tab: 'chat' as const, label: 'チャット' },
  { tab: 'thought' as const, label: '思考' },
  { tab: 'memory' as const, label: '記憶' },
];

export function Sidebar() {
  const { sidebarTab, activeView, setSidebarTab, setActiveView } = useUIStore();

  const handleBackToChat = () => {
    setActiveView('chat');
  };

  return (
    <aside className="w-72 flex flex-col border-r border-border bg-card">
      {/* App Header — クリックでチャット画面に戻る */}
      <div
        className="p-4 border-b border-border cursor-pointer hover:bg-muted/50 transition-colors"
        onClick={handleBackToChat}
      >
        <h1 className="text-lg font-semibold text-primary">AI Character Chat</h1>
      </div>

      {/* Character Selector */}
      <CharacterSelector />

      {/* Content Tabs: チャット / 思考 / 記憶 */}
      <div className="flex border-b border-border">
        {sidebarTabs.map(({ tab, label }) => (
          <button
            key={tab}
            onClick={() => {
              setSidebarTab(tab);
              setActiveView('chat'); // タブクリックでメインをチャットに戻す
            }}
            className={`flex-1 py-2 text-xs transition-colors ${
              sidebarTab === tab
                ? 'text-primary border-b-2 border-primary font-medium'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-y-auto">
        {sidebarTab === 'chat' && <ChatList />}
        {sidebarTab === 'thought' && <SidebarThoughtList />}
        {sidebarTab === 'memory' && <SidebarMemoryList />}
      </div>

      {/* Bottom Nav Icons */}
      <div className="p-2 border-t border-border flex gap-1">
        <button
          onClick={handleBackToChat}
          className={`flex-1 p-2 rounded-lg transition-colors ${
            activeView === 'chat'
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
          title="チャット"
        >
          <MessageSquare className="w-5 h-5 mx-auto" />
        </button>
        <button
          onClick={() => setActiveView('characters')}
          className={`flex-1 p-2 rounded-lg transition-colors ${
            activeView === 'characters'
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
          title="キャラクター管理"
        >
          <Users className="w-5 h-5 mx-auto" />
        </button>
        <button
          onClick={() => setActiveView('settings')}
          className={`flex-1 p-2 rounded-lg transition-colors ${
            activeView === 'settings'
              ? 'bg-accent text-accent-foreground'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
          title="設定"
        >
          <Settings className="w-5 h-5 mx-auto" />
        </button>
      </div>
    </aside>
  );
}
