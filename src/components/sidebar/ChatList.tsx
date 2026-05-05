import { Plus } from 'lucide-react';
import { useChatStore, useCharacterStore } from '../../stores';

export function ChatList() {
  const { sessions, currentSessionId, selectSession, createSession, fetchHistory } =
    useChatStore();
  const { selectedCharacterId } = useCharacterStore();

  const handleSelectSession = (sessionId: string) => {
    selectSession(sessionId);
    fetchHistory(sessionId);
  };

  const handleNewSession = async () => {
    if (!selectedCharacterId) return;
    await createSession(selectedCharacterId);
  };

  return (
    <div className="flex flex-col gap-1 px-2">
      <button
        onClick={handleNewSession}
        disabled={!selectedCharacterId}
        className="flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium text-muted-foreground hover:bg-accent hover:text-accent-foreground disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <Plus className="h-4 w-4" />
        <span>新規セッション</span>
      </button>

      <div className="flex flex-col gap-0.5">
        {sessions.map((session) => (
          <button
            key={session.id}
            onClick={() => handleSelectSession(session.id)}
            className={`flex flex-col items-start rounded-md px-3 py-2 text-sm text-left transition-colors ${
              currentSessionId === session.id
                ? 'bg-accent text-accent-foreground'
                : 'text-muted-foreground hover:bg-accent/50 hover:text-accent-foreground'
            }`}
          >
            <span className="truncate w-full font-medium">
              {session.last_message_preview || session.title || '新しいチャット'}
            </span>
            {session.last_message_at && (
              <span className="text-xs text-muted-foreground mt-0.5">
                {new Date(session.last_message_at).toLocaleDateString('ja-JP')}
              </span>
            )}
          </button>
        ))}
      </div>
    </div>
  );
}
