import { useEffect, useState } from 'react';
import { X, CheckCircle2, AlertCircle, Info } from 'lucide-react';
import { useUIStore } from '../../stores';
import type { Toast as ToastData, ToastType } from '../../stores/ui.store';

const ICON_MAP: Record<ToastType, typeof CheckCircle2> = {
  success: CheckCircle2,
  error: AlertCircle,
  info: Info,
};

const COLOR_MAP: Record<ToastType, string> = {
  success: 'border-green-500/50 bg-green-500/10 text-green-400',
  error: 'border-destructive/50 bg-destructive/10 text-destructive',
  info: 'border-blue-500/50 bg-blue-500/10 text-blue-400',
};

function ToastItem({ toast, onRemove }: { toast: ToastData; onRemove: () => void }) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    // マウント後にアニメーション開始
    requestAnimationFrame(() => setVisible(true));
  }, []);

  const Icon = ICON_MAP[toast.type];

  return (
    <div
      className={`flex items-center gap-2 px-4 py-3 rounded-lg border shadow-lg backdrop-blur-sm transition-all duration-300 ${COLOR_MAP[toast.type]} ${
        visible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-2'
      }`}
      role="alert"
    >
      <Icon className="w-4 h-4 shrink-0" />
      <span className="text-sm font-medium flex-1">{toast.message}</span>
      <button
        onClick={onRemove}
        className="p-0.5 rounded hover:bg-white/10 transition-colors"
        aria-label="閉じる"
      >
        <X className="w-3.5 h-3.5" />
      </button>
    </div>
  );
}

export function ToastContainer() {
  const { toasts, removeToast } = useUIStore();

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} onRemove={() => removeToast(toast.id)} />
      ))}
    </div>
  );
}
