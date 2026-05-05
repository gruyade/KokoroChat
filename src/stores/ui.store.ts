import { create } from 'zustand';

/** メインコンテンツ領域に表示するビュー */
type ActiveView = 'chat' | 'characters' | 'settings';

/** サイドバー内のタブ */
type SidebarTab = 'chat' | 'thought' | 'memory';

interface UIState {
  theme: 'light' | 'dark';
  sidebarOpen: boolean;
  activeView: ActiveView;
  sidebarTab: SidebarTab;
  toggleTheme: () => void;
  toggleSidebar: () => void;
  setActiveView: (view: ActiveView) => void;
  setSidebarTab: (tab: SidebarTab) => void;
}

export const useUIStore = create<UIState>((set) => ({
  theme: 'dark',
  sidebarOpen: true,
  activeView: 'chat',
  sidebarTab: 'chat',

  toggleTheme: () => {
    set((state) => ({ theme: state.theme === 'light' ? 'dark' : 'light' }));
  },

  toggleSidebar: () => {
    set((state) => ({ sidebarOpen: !state.sidebarOpen }));
  },

  setActiveView: (view: ActiveView) => {
    set({ activeView: view });
  },

  setSidebarTab: (tab: SidebarTab) => {
    set({ sidebarTab: tab });
  },
}));
