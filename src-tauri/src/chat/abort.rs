// StreamAbortManager — ストリーミング中断管理

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::task::AbortHandle;

/// ストリーミング中断管理構造体
///
/// アクティブなストリーミングセッションの AbortHandle と部分コンテンツを保持し、
/// 中断リクエスト時に部分コンテンツを返却する。
pub struct StreamAbortManager {
    /// session_id → (AbortHandle, partial_content)
    #[allow(clippy::type_complexity)]
    active: Mutex<HashMap<String, (AbortHandle, Arc<Mutex<String>>)>>,
}

impl StreamAbortManager {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
        }
    }

    /// ストリーム開始時に登録
    pub fn register(
        &self,
        session_id: &str,
        abort_handle: AbortHandle,
        partial_content: Arc<Mutex<String>>,
    ) {
        let mut active = self.active.lock().unwrap();
        active.insert(session_id.to_string(), (abort_handle, partial_content));
    }

    /// ストリーム中断 — AbortHandle を abort し、部分コンテンツを返却
    /// アクティブなストリームがない場合は None を返す
    pub fn abort(&self, session_id: &str) -> Option<String> {
        let mut active = self.active.lock().unwrap();
        if let Some((handle, partial_content)) = active.remove(session_id) {
            handle.abort();
            let content = partial_content.lock().unwrap().clone();
            Some(content)
        } else {
            None
        }
    }

    /// 正常完了時にクリーンアップ
    pub fn remove(&self, session_id: &str) {
        let mut active = self.active.lock().unwrap();
        active.remove(session_id);
    }

    /// 指定セッションにアクティブなストリームがあるか確認
    pub fn has_active_stream(&self, session_id: &str) -> bool {
        let active = self.active.lock().unwrap();
        active.contains_key(session_id)
    }
}

impl Default for StreamAbortManager {
    fn default() -> Self {
        Self::new()
    }
}
