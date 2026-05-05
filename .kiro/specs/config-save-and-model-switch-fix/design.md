# Config Save and Model Switch Fix - Bugfix Design

## Overview

設定保存の永続化失敗とLLMリクエスト並行実行によるVRAMクラッシュの2つのバグを修正する。

- **問題1**: `apply_env_fallback`が環境変数の値でconfig.jsonの保存値を常に上書きし、ユーザーがUIで保存した設定が再起動時に失われる
- **問題2**: バックグラウンドタスク（記憶圧縮・思考エンジン）がチャットストリーミングと並行してLLMリクエストを送信し、LM Studioが複数モデルを同時ロードしてVRAM不足でクラッシュする

修正アプローチ:
1. `apply_env_fallback`のロジックを「常に上書き」から「空の場合のみフォールバック」に変更
2. グローバルなLLMリクエストセマフォ（`tokio::sync::Mutex`）を導入し、全LLMリクエストを直列化

## Glossary

- **Bug_Condition (C)**: config.jsonに非空の値が保存されている状態で環境変数も設定されている、またはLLMリクエストが並行して発行される状態
- **Property (P)**: config.jsonの非空値が環境変数で上書きされない、かつLLMリクエストが同時に1つしか実行されない
- **Preservation**: config.json未存在時の環境変数フォールバック、空値への環境変数適用、記憶圧縮・思考エンジンの正常動作
- **`apply_env_fallback`**: `ModelConfigManager`内の関数。環境変数からモデル設定を読み込みconfigに適用する
- **`LlmLock`**: 新規導入するグローバルな`tokio::sync::Mutex<()>`。LLMリクエストの直列化に使用
- **`ModelConfigManager`**: `src-tauri/src/config/model_config.rs`内の設定管理構造体
- **`DefaultMemoryManager`**: `src-tauri/src/memory/manager.rs`内の記憶圧縮管理
- **`DefaultThoughtEngine`**: `src-tauri/src/thought/engine.rs`内の思考生成エンジン

## Bug Details

### Bug Condition

2つの独立したバグ条件が存在する。

**条件A: 設定永続化失敗**

config.jsonに非空の値が保存されているにもかかわらず、`apply_env_fallback`が環境変数の値で常に上書きする。`dotenvy::dotenv()`が親ディレクトリを再帰探索して`.env`を発見するため、開発環境の`.env`がリリースビルドにも影響する。

**条件B: LLMリクエスト並行実行**

チャットストリーミング中にバックグラウンドタスクが別のLLMリクエストを送信し、LM Studioが複数の大型モデルを同時ロードしようとする。

**Formal Specification:**
```
FUNCTION isBugCondition(input)
  INPUT: input of type AppStartupOrChatEvent
  OUTPUT: boolean
  
  // 条件A: 設定上書き
  IF input.type == "config_load" THEN
    RETURN configFileExists(input.configPath)
           AND configValue(input.configPath, input.field) != ""
           AND envVarSet(input.envVarName)
           AND envVarValue(input.envVarName) != configValue(input.configPath, input.field)
  END IF
  
  // 条件B: LLMリクエスト並行実行
  IF input.type == "llm_request" THEN
    RETURN anotherLlmRequestInProgress()
  END IF
  
  RETURN false
END FUNCTION
```

### Examples

- **設定上書き**: config.jsonに`model: "gemma-3-12b"`が保存されている状態で再起動 → `.env`の`AI_CHAT_LLM_MODEL=qwen3-30b-a3b`で上書きされ、UIに反映されない
- **並行リクエスト（記憶圧縮）**: チャット送信完了直後に`tokio::spawn`で記憶圧縮が発火 → Chat用ストリーミングがまだLM Studio側で処理中の場合、Memory用モデルのロードが同時に走る
- **並行リクエスト（思考エンジン）**: チャットストリーミング中に思考エンジンのintervalが経過 → Thought用モデルのリクエストが並行送信される
- **正常ケース**: config.jsonの`model`が空文字列で`.env`に値がある → 環境変数がフォールバックとして正しく適用される（これはバグではない）

## Expected Behavior

### Preservation Requirements

**Unchanged Behaviors:**
- config.jsonが存在しない場合、環境変数の値がデフォルト設定として使用される
- config.jsonのフィールドが空文字列の場合、環境変数の値がフォールバックとして適用される
- チャットストリーミング完了後に記憶圧縮チェックが実行される（タイミングが変わるだけで実行自体は保証）
- 思考エンジンがinterval_minutes間隔で思考を生成する（LLMロック待機時間分の遅延は許容）
- UIからの設定保存がconfig.jsonに正しく永続化される
- `set_config`によるメモリ上の設定更新は即座に反映される

**Scope:**
以下の入力・操作はこの修正の影響を受けない：
- マウスクリックによるUI操作全般
- config.jsonが空値のフィールドへの環境変数フォールバック
- LLMリクエストが競合しない場合のバックグラウンドタスク実行
- データベース操作（チャット履歴、キャラクター管理等）

## Hypothesized Root Cause

### 問題1: 設定永続化失敗

1. **`apply_env_for_purpose`の無条件上書きロジック**: 環境変数が設定されていれば`!val.is_empty()`チェックのみで常にconfig値を上書きする。config.jsonに既に値がある場合でも区別しない
2. **`dotenvy::dotenv()`の親ディレクトリ探索**: リリースビルドのexeが`target/release/`から実行される際、プロジェクトルートの`.env`が発見される。これ自体は仕様だが、上書きロジックと組み合わさって問題化

### 問題2: LLMリクエスト並行実行

1. **`send_message`内の`tokio::spawn`による記憶圧縮**: `send_message`の戻り値を待たずに即座にspawnされるため、チャットストリーミング完了直後（LM Studio側のモデルアンロード前）に別モデルのリクエストが飛ぶ
2. **思考エンジンのタイマーループ**: `tokio::time::sleep`後に無条件でLLMリクエストを送信するため、他のリクエストとの競合を考慮しない
3. **LLMリクエストの排他制御の欠如**: 現在のアーキテクチャにはLLMリクエスト間の調整メカニズムが存在しない

## Correctness Properties

Property 1: Bug Condition - Config Non-Empty Values Preserved on Load

_For any_ config.json where a model settings field (base_url, model, api_key) contains a non-empty value, AND the corresponding environment variable is also set to a non-empty value, the fixed `apply_env_fallback` function SHALL NOT overwrite the config.json value, preserving the user's saved settings across restarts.

**Validates: Requirements 2.1**

Property 2: Preservation - Empty Config Values Receive Env Fallback

_For any_ config.json where a model settings field (base_url, model, api_key) is empty (empty string or None for api_key), AND the corresponding environment variable is set to a non-empty value, the fixed `apply_env_fallback` function SHALL apply the environment variable value as a fallback, producing the same behavior as the original function for these inputs.

**Validates: Requirements 3.1, 3.2**

Property 3: Bug Condition - LLM Request Serialization

_For any_ sequence of concurrent LLM requests (chat streaming, memory compression, thought generation), the fixed system SHALL ensure that at most one LLM request is in-flight at any given time, preventing simultaneous model loading on LM Studio.

**Validates: Requirements 2.2, 2.3**

Property 4: Preservation - Background Tasks Still Execute

_For any_ scenario where the LLM lock is not held by another task, background tasks (memory compression, thought generation) SHALL execute their LLM requests without unnecessary delay, preserving the existing functionality of these subsystems.

**Validates: Requirements 3.3, 3.4**

## Fix Implementation

### Changes Required

**File**: `src-tauri/src/config/model_config.rs`

**Function**: `apply_env_for_purpose`

**Specific Changes**:
1. **条件付き上書きロジック**: 環境変数の値を適用する前に、既存のconfig値が空かどうかをチェック。非空の場合はスキップ
   - `settings.base_url`が空の場合のみ`{PREFIX}_BASE_URL`を適用
   - `settings.model`が空の場合のみ`{PREFIX}_MODEL`を適用
   - `settings.api_key`がNoneの場合のみ`{PREFIX}_API_KEY`を適用
2. **ログメッセージ変更**: "env override"から"env fallback"に変更し、フォールバック動作であることを明示

---

**File**: `src-tauri/src/state.rs`

**Specific Changes**:
3. **LLMロックの追加**: `AppState`に`llm_lock: Arc<tokio::sync::Mutex<()>>`フィールドを追加

---

**File**: `src-tauri/src/lib.rs`

**Specific Changes**:
4. **LLMロックの初期化**: `AppState`構築時に`llm_lock: Arc::new(tokio::sync::Mutex::new(()))`を追加

---

**File**: `src-tauri/src/commands/chat.rs`

**Function**: `send_message`

**Specific Changes**:
5. **記憶圧縮の直列化**: `tokio::spawn`を削除し、`send_message`完了後にLLMロックを取得してから記憶圧縮を実行。または、`send_message`自体がLLMロックを保持した状態で実行し、完了後にロック解放→記憶圧縮がロック取得→実行の流れにする

---

**File**: `src-tauri/src/memory/manager.rs`

**Specific Changes**:
6. **LLMロック取得**: `check_and_compress`内のLLM呼び出し前に`llm_lock`を取得。`MemoryManager` traitまたは`DefaultMemoryManager`にロック参照を追加

---

**File**: `src-tauri/src/thought/engine.rs`

**Specific Changes**:
7. **LLMロック取得**: 思考生成ループ内のLLM呼び出し前に`llm_lock`を取得。`DefaultThoughtEngine`にロック参照を追加

---

**File**: `src-tauri/src/chat/engine.rs`

**Specific Changes**:
8. **チャットストリーミングのLLMロック取得**: `chat_stream`呼び出し前にLLMロックを取得し、ストリーミング完了後に解放。これにより他のタスクはストリーミング完了まで待機

## Testing Strategy

### Validation Approach

テスト戦略は2フェーズ: まず未修正コードでバグを再現する反例を発見し、次に修正後のコードで正しい動作と既存動作の保持を検証する。

### Exploratory Bug Condition Checking

**Goal**: 未修正コードでバグを再現し、根本原因の分析を確認または反証する。

**Test Plan**: `apply_env_for_purpose`に対して、config.jsonに非空値がある状態で環境変数を設定し、上書きされることを確認する。LLM並行実行については、モックを使って同時リクエストが可能であることを確認する。

**Test Cases**:
1. **Config上書きテスト**: config.jsonに`model: "gemma-3-12b"`を設定し、環境変数`AI_CHAT_LLM_MODEL=qwen3-30b-a3b`を設定して`load_or_default`を呼び出す → 値が上書きされる（未修正コードで失敗）
2. **全フィールド上書きテスト**: base_url, model, api_keyすべてに非空値を設定し、環境変数で上書きされることを確認
3. **並行LLMリクエストテスト**: 2つのLLMリクエストを同時に発行し、両方が並行実行されることを確認（未修正コードで失敗 = 並行実行が可能）

**Expected Counterexamples**:
- config.jsonの`model`フィールドが環境変数の値で上書きされる
- 2つのLLMリクエストが同時にin-flightになる

### Fix Checking

**Goal**: バグ条件が成立する全入力に対して、修正後の関数が期待動作を生成することを検証。

**Pseudocode:**
```
FOR ALL input WHERE isBugCondition(input) DO
  IF input.type == "config_load" THEN
    result := apply_env_fallback_fixed(input.config)
    ASSERT result.field == input.config.originalValue  // 非空値は保持
  END IF
  IF input.type == "llm_request" THEN
    result := execute_with_lock(input.request)
    ASSERT no_concurrent_requests_observed()
  END IF
END FOR
```

### Preservation Checking

**Goal**: バグ条件が成立しない全入力に対して、修正後の関数が元の関数と同じ結果を生成することを検証。

**Pseudocode:**
```
FOR ALL input WHERE NOT isBugCondition(input) DO
  IF input.type == "config_load" AND input.config.field == "" THEN
    ASSERT apply_env_fallback_fixed(input.config) == apply_env_fallback_original(input.config)
  END IF
  IF input.type == "llm_request" AND NOT anotherLlmRequestInProgress() THEN
    ASSERT execute_with_lock(input.request) completes without unnecessary delay
  END IF
END FOR
```

**Testing Approach**: Property-based testingを推奨。理由：
- 環境変数とconfig値の組み合わせが多数あり、手動テストでは網羅困難
- 空文字列、None、非空値の境界条件を自動生成で網羅
- フォールバック動作の保持を強く保証

**Test Plan**: 未修正コードで空値フィールドへの環境変数適用動作を観察し、修正後もその動作が維持されることをproperty-based testで検証。

**Test Cases**:
1. **空値フォールバック保持**: config.jsonのフィールドが空文字列の場合、環境変数が正しく適用されることを検証
2. **None api_keyフォールバック保持**: api_keyがNoneの場合、環境変数が正しく適用されることを検証
3. **config未存在時のフォールバック保持**: config.jsonが存在しない場合、デフォルト設定に環境変数が適用されることを検証
4. **LLMロック非競合時の即時実行**: ロックが空いている場合、バックグラウンドタスクが遅延なく実行されることを検証

### Unit Tests

- `apply_env_for_purpose`の条件分岐テスト（非空値保持、空値フォールバック、None処理）
- `LlmLock`の取得・解放テスト
- `send_message`後の記憶圧縮がLLMロックを取得することのテスト

### Property-Based Tests

- ランダムなModelSettings値（空/非空/None）と環境変数値の組み合わせを生成し、非空値が保持されることを検証
- ランダムな空値configと環境変数の組み合わせを生成し、フォールバックが正しく適用されることを検証
- ランダムなタイミングでLLMリクエストを発行し、同時に1つしか実行されないことを検証

### Integration Tests

- アプリ起動→設定保存→再起動のフルフローで設定が永続化されることを検証
- チャット送信→記憶圧縮→思考生成の順序でLLMリクエストが直列実行されることを検証
- 設定変更後のバックグラウンドタスクが新しい設定値を使用することを検証
