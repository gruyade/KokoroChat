import { useEffect } from 'react';
import { Plus, Trash2 } from 'lucide-react';
import { useChatStore, useCharacterStore, useUIStore } from '../../stores';

export function ChatList() {
  const { sessions, currentSessionId, selectSession, createSession, deleteSession, fetchSessions, fetchHistory } =
    useChatStore();
  const { selectedCharacterId } = useCharacterStore();
  const { setActiveView } = useUIStore();

  // キャラクター変更時にセッション一覧を自動取得
  useEffect(() => {
    if (selectedCharacterId) {
      fetchSessions(selectedCharacterId);
    }
  }, [selectedCharacterId, fetchSessions]);

  const handleSelectSession = (sessionId: string) => {
    selectSession(sessionId);
    fetchHistory(sessionId);
    setActiveView('chat'); // チャット画面に遷移
  };

  const handleNewSession = async () => {
    if (!selectedCharacterId) return;
    await createSession(selectedCharacterId);
    setActiveView('chat'); // チャット画面に遷移
  };

  const handleDeleteSession = async (e: React.MouseEvent, sessionId: string) => {
    e.stopPropagation();
    if (!confirm('このチャットを削除してよいか？')) return;
    await deleteSession(sessionId);
  };

  if (!selectedCharacterId) {
    return (
      <div className="p-3 text-sm text-muted-foreground">
        キャラクターを選択してください
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Chat session list */}
      <div className="flex-1 overflow-y-auto p-2 space-y-1">
        {sessions.length === 0 && (
          <div className="p-2 text-xs text-muted-foreground">チャット履歴なし</div>
        )}
        {sessions.map((session) => (
          <div
            key={session.id}
            onClick={() => handleSelectSession(session.id)}
            className={`group w-full p-2 rounded-lg text-left transition-colors cursor-pointer ${
              currentSessionId === session.id
                ? 'bg-accent text-accent-foreground'
                : 'hover:bg-muted text-foreground'
            }`}
          >
            <div className="flex items-start justify-between gap-1">
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium truncate">
                  {session.title || session.last_message_preview || '新しいチャット'}
                </div>
                {session.last_message_preview && (
                  <div className="text-xs text-muted-foreground mt-0.5 truncate">
                    {session.last_message_preview}
                  </div>
                )}
                {session.last_message_at && (
                  <div className="text-xs text-muted-foreground/70 mt-0.5">
                    {formatRelativeTime(session.last_message_at)}
                  </div>
                )}
              </div>
              <button
                onClick={(e) => handleDeleteSession(e, session.id)}
                className="p-1 rounded opacity-0 group-hover:opacity-100 hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-all shrink-0"
                title="削除"
              >
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        ))}
      </div>

      {/* New Chat Button */}
      <div className="p-3 border-t border-border">
        <button
          onClick={handleNewSession}
          className="w-full py-2 px-3 rounded-lg bg-primary/10 text-primary text-sm font-medium hover:bg-primary/20 transition-colors flex items-center justify-center gap-2"
        >
          <Plus className="w-4 h-4" />
          新しいチャット
        </button>
      </div>
    </div>
  );
}

/** 相対時間表示 */
function formatRelativeTime(isoString: string): string {
  const date = new Date(isoString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  const diffHour = Math.floor(diffMs / 3600000);
  const diffDay = Math.floor(diffMs / 86400000);

  if (diffMin < 1) return 'たった今';
  if (diffMin < 60) return `${diffMin}分前`;
  if (diffHour < 24) return `${diffHour}時間前`;
  if (diffDay < 7) return `${diffDay}日前`;
  return date.toLocaleDateString('ja-JP');
}
