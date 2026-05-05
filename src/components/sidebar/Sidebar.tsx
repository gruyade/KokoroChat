import {
  MessageSquare,
  Users,
  Settings,
  Puzzle,
  Brain,
  Lightbulb,
  Moon,
  Sun,
  PanelLeftClose,
  PanelLeft,
} from 'lucide-react';
import { useUIStore } from '../../stores';
import { ChatList } from './ChatList';
import { CharacterList } from './CharacterList';

const navItems = [
  { view: 'chat' as const, icon: MessageSquare, label: 'チャット' },
  { view: 'characters' as const, icon: Users, label: 'キャラクター' },
  { view: 'plugins' as const, icon: Puzzle, label: 'プラグイン' },
  { view: 'memory' as const, icon: Brain, label: '記憶' },
  { view: 'thought' as const, icon: Lightbulb, label: '思考' },
  { view: 'settings' as const, icon: Settings, label: '設定' },
];

export function Sidebar() {
  const { theme, sidebarOpen, activeView, toggleTheme, toggleSidebar, setActiveView } =
    useUIStore();

  return (
    <aside
      className={`flex flex-col border-r border-border bg-card transition-all duration-200 ${
        sidebarOpen ? 'w-64' : 'w-14'
      }`}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-3 border-b border-border">
        {sidebarOpen && (
          <h1 className="text-sm font-semibold text-foreground truncate">AI Character Chat</h1>
        )}
        <button
          onClick={toggleSidebar}
          className="p-1.5 rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground"
          aria-label={sidebarOpen ? 'サイドバーを閉じる' : 'サイドバーを開く'}
        >
          {sidebarOpen ? (
            <PanelLeftClose className="h-4 w-4" />
          ) : (
            <PanelLeft className="h-4 w-4" />
          )}
        </button>
      </div>

      {/* Navigation */}
      <nav className="flex flex-col gap-0.5 px-2 py-2">
        {navItems.map(({ view, icon: Icon, label }) => (
          <button
            key={view}
            onClick={() => setActiveView(view)}
            className={`flex items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors ${
              activeView === view
                ? 'bg-accent text-accent-foreground font-medium'
                : 'text-muted-foreground hover:bg-accent/50 hover:text-accent-foreground'
            }`}
            title={label}
          >
            <Icon className="h-4 w-4 shrink-0" />
            {sidebarOpen && <span>{label}</span>}
          </button>
        ))}
      </nav>

      {/* Content area — shows list based on active view */}
      {sidebarOpen && (
        <div className="flex-1 overflow-y-auto border-t border-border pt-2">
          {activeView === 'chat' && <ChatList />}
          {activeView === 'characters' && <CharacterList />}
        </div>
      )}

      {/* Footer — theme toggle */}
      <div className="mt-auto border-t border-border px-2 py-2">
        <button
          onClick={toggleTheme}
          className="flex items-center gap-2 rounded-md px-3 py-2 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground w-full"
          title={theme === 'dark' ? 'ライトモード' : 'ダークモード'}
        >
          {theme === 'dark' ? (
            <Sun className="h-4 w-4 shrink-0" />
          ) : (
            <Moon className="h-4 w-4 shrink-0" />
          )}
          {sidebarOpen && <span>{theme === 'dark' ? 'ライトモード' : 'ダークモード'}</span>}
        </button>
      </div>
    </aside>
  );
}
