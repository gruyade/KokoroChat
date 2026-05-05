import { create } from 'zustand';

type ActiveView = 'chat' | 'characters' | 'settings' | 'plugins' | 'memory' | 'thought';

interface UIState {
  theme: 'light' | 'dark';
  sidebarOpen: boolean;
  activeView: ActiveView;
  toggleTheme: () => void;
  toggleSidebar: () => void;
  setActiveView: (view: ActiveView) => void;
}

export const useUIStore = create<UIState>((set) => ({
  theme: 'dark',
  sidebarOpen: true,
  activeView: 'chat',

  toggleTheme: () => {
    set((state) => ({ theme: state.theme === 'light' ? 'dark' : 'light' }));
  },

  toggleSidebar: () => {
    set((state) => ({ sidebarOpen: !state.sidebarOpen }));
  },

  setActiveView: (view: ActiveView) => {
    set({ activeView: view });
  },
}));
