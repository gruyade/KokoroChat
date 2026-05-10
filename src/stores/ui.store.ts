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

/** localStorageからテーマを復元、なければシステム設定を参照 */
function getInitialTheme(): 'light' | 'dark' {
  try {
    const stored = localStorage.getItem('theme');
    if (stored === 'light' || stored === 'dark') return stored;
    if (typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches) return 'dark';
  } catch {
    // テスト環境等でlocalStorageが利用不可の場合
  }
  return 'dark'; // デフォルトはダーク
}

/** DOMにテーマクラスを反映 */
function applyThemeToDOM(theme: 'light' | 'dark') {
  try {
    if (theme === 'dark') {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  } catch {
    // テスト環境等でdocumentが利用不可の場合
  }
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

// 初期テーマをDOMに即座に反映
const initialTheme = getInitialTheme();
applyThemeToDOM(initialTheme);

export const useUIStore = create<UIState>((set) => ({
  theme: initialTheme,
  sidebarOpen: true,
  activeView: 'chat',
  sidebarTab: 'chat',
  toasts: [],

  toggleTheme: () => {
    set((state) => {
      const newTheme = state.theme === 'light' ? 'dark' : 'light';
      applyThemeToDOM(newTheme);
      try { localStorage.setItem('theme', newTheme); } catch { /* noop */ }
      return { theme: newTheme };
    });
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
