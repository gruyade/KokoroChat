import { create } from 'zustand';

/** メインコンテンツ領域に表示するビュー */
type ActiveView = 'chat' | 'characters' | 'settings';

/** サイドバー内のタブ */
type SidebarTab = 'chat' | 'thought' | 'memory';

/** トーストの種類 */
export type ToastType = 'success' | 'error' | 'info';

export interface Toast {
  id: string;
  message: string;
  type: ToastType;
}

interface UIState {
  theme: 'light' | 'dark';
  sidebarOpen: boolean;
  activeView: ActiveView;
  sidebarTab: SidebarTab;
  toasts: Toast[];
  toggleTheme: () => void;
  toggleSidebar: () => void;
  setActiveView: (view: ActiveView) => void;
  setSidebarTab: (tab: SidebarTab) => void;
  showToast: (message: string, type?: ToastType) => void;
  removeToast: (id: string) => void;
}

let toastCounter = 0;

export const useUIStore = create<UIState>((set) => ({
  theme: 'dark',
  sidebarOpen: true,
  activeView: 'chat',
  sidebarTab: 'chat',
  toasts: [],

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

  showToast: (message: string, type: ToastType = 'success') => {
    const id = `toast-${++toastCounter}`;
    set((state) => ({ toasts: [...state.toasts, { id, message, type }] }));
    setTimeout(() => {
      set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
    }, 3000);
  },

  removeToast: (id: string) => {
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
  },
}));
