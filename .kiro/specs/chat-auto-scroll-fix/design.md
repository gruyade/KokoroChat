# チャット自動スクロール修正 Bugfix Design

## Overview

AIストリーミング中にユーザーがスクロールアップしている場合でも強制的に最下部へスクロールされるバグの修正。`ChatView.tsx`のスクロール制御ロジックにおいて、`isProgrammaticScrollRef`フラグが`handleScroll`を抑制し`isNearBottomRef`の更新を妨げる問題、および`smoothScrollToBottom`が高頻度で呼ばれる際の競合状態を解消する。

## Glossary

- **Bug_Condition (C)**: ユーザーがスクロールアップ中（底から200px超離れている）にストリーミング更新が発生する状態
- **Property (P)**: バグ条件下でスクロール位置が維持され、自動スクロールが抑制される動作
- **Preservation**: ユーザーが底付近にいる場合の自動スクロール、セッション切り替え時の即時スクロール等の既存動作
- **`isNearBottomRef`**: `ChatView.tsx`内のrefで、ユーザーが底から200px以内にいるかを追跡するフラグ
- **`isProgrammaticScrollRef`**: プログラムによるスクロール中に`handleScroll`を無視するためのフラグ
- **`smoothScrollToBottom`**: rAFベースのスムーズスクロール関数。`isNearBottomRef`がtrueの場合のみ実行

## Bug Details

### Bug Condition

ユーザーがスクロールアップして過去メッセージを閲覧中に、ストリーミングコンテンツが更新されると強制的に最下部へスクロールされる。根本原因は`isProgrammaticScrollRef`フラグが`handleScroll`内で`isNearBottomRef`の更新をブロックし、一度`isNearBottomRef`がtrueになると、ユーザーがスクロールアップしても`false`に更新されないこと。

**Formal Specification:**
```
FUNCTION isBugCondition(input)
  INPUT: input of type { userScrollPosition: number, scrollHeight: number, clientHeight: number, isStreaming: boolean, streamingContentUpdated: boolean }
  OUTPUT: boolean

  distanceFromBottom := input.scrollHeight - input.userScrollPosition - input.clientHeight
  userIsScrolledUp := distanceFromBottom > 200
  
  RETURN userIsScrolledUp = true
         AND input.isStreaming = true
         AND input.streamingContentUpdated = true
         AND isNearBottomRef.current = true  // バグ: フラグが正しく更新されていない
END FUNCTION
```

### Examples

- ユーザーが500px上にスクロール中、ストリーミングチャンクが到着 → 期待: 位置維持 / 実際: 最下部へ強制スクロール
- ユーザーがスクロールアップ直後（300ms以内）にストリーミング更新 → 期待: 位置維持 / 実際: `isProgrammaticScrollRef`がtrueのため`handleScroll`が無視され`isNearBottomRef`がfalseにならず、自動スクロール実行
- ストリーミング中に高頻度（50ms間隔）でチャンク到着、ユーザーが途中でスクロールアップ → 期待: スクロールアップ検知後は位置維持 / 実際: 次のチャンクで即座に最下部へ戻される
- ユーザーが底から150px（200px以内）の位置にいる場合 → 期待: 自動スクロール継続（これは正常動作）

## Expected Behavior

### Preservation Requirements

**Unchanged Behaviors:**
- ユーザーが底付近（200px以内）にいる状態でのストリーミング中自動スクロール
- 新メッセージ追加時（ユーザーが底付近の場合）のスムーズスクロール
- セッション切り替え時の即時最下部スクロール
- ユーザーが手動で最下部までスクロールダウンした場合の自動スクロール再有効化
- マウスホイール・タッチによるスクロール操作の応答性

**Scope:**
ユーザーが底付近（200px以内）にいる全てのケースは、この修正の影響を受けない。以下を含む:
- 底付近でのストリーミング自動追従
- セッション切り替え時のスクロール
- 新メッセージ追加時のスクロール
- 手動スクロールダウンによる自動スクロール再有効化

## Hypothesized Root Cause

Based on the bug description and source code analysis, the most likely issues are:

1. **`isProgrammaticScrollRef`による`handleScroll`の完全ブロック**: `smoothScrollToBottom`が呼ばれると`isProgrammaticScrollRef`がtrueになり、300msのタイムアウトまで`handleScroll`が完全に無視される。この間にユーザーがスクロールアップしても`isNearBottomRef`がfalseに更新されない。ストリーミング中は高頻度で`smoothScrollToBottom`が呼ばれるため、300msのウィンドウが連続的に延長され、事実上`handleScroll`が永続的にブロックされる。

2. **`isNearBottomRef`の初期値問題**: `isNearBottomRef`の初期値がtrueであり、ページロード直後やセッション切り替え直後は常に自動スクロールが有効。これ自体は正しいが、上記のブロック問題と組み合わさると、一度trueになった後にfalseへ遷移する機会が失われる。

3. **スクロールイベントとrAFの競合**: `smoothScrollToBottom`はrAFで実行されるが、ユーザーのスクロールイベントも同じフレーム内で発生する可能性がある。`isProgrammaticScrollRef`のフラグ管理がタイミングに依存しており、確実な排他制御になっていない。

4. **300msタイムアウトの不適切さ**: smooth scrollの完了を300ms固定で推定しているが、実際のスクロール完了時間はコンテンツ量やブラウザの状態に依存する。この間にユーザー操作が発生した場合の考慮が不足。

## Correctness Properties

Property 1: Bug Condition - ユーザースクロールアップ中の位置維持

_For any_ input where the user has scrolled up (distance from bottom > 200px) and streaming content is being updated, the fixed scroll handling logic SHALL NOT trigger automatic scrolling, preserving the user's current scroll position.

**Validates: Requirements 2.1, 2.2, 2.3**

Property 2: Preservation - 底付近での自動スクロール継続

_For any_ input where the user is near the bottom (distance from bottom <= 200px) or a session switch occurs, the fixed scroll handling logic SHALL produce the same scrolling behavior as the original code, preserving automatic scroll-to-bottom functionality.

**Validates: Requirements 3.1, 3.2, 3.3, 3.4**

## Fix Implementation

### Changes Required

Assuming our root cause analysis is correct:

**File**: `src/components/chat/ChatView.tsx`

**Functions**: `handleScroll`, `smoothScrollToBottom`

**Specific Changes**:

1. **`isProgrammaticScrollRef`の廃止またはユーザー操作検知の分離**: プログラムスクロール中でもユーザーの明示的なスクロール操作（wheel, touch, keyboard）を検知できるようにする。`handleScroll`の抑制ロジックを削除し、代わりにユーザー操作イベント（`wheel`, `touchmove`, `keydown`）を直接リッスンして`isNearBottomRef`をfalseに設定する。

2. **ユーザー操作による即時フラグ更新**: `wheel`/`touchmove`イベントリスナーを追加し、ユーザーが上方向にスクロールした場合に即座に`isNearBottomRef = false`を設定。これにより`isProgrammaticScrollRef`の状態に関係なくユーザー意図を検知可能。

3. **`smoothScrollToBottom`のガード強化**: `smoothScrollToBottom`内の`isNearBottomRef`チェックを、rAFコールバック内（実行直前）でも再度行う。rAFのスケジュールから実行までの間にユーザーがスクロールアップした場合を検知。

4. **`isProgrammaticScrollRef`のスコープ縮小**: `isProgrammaticScrollRef`を完全に廃止するか、`handleScroll`内での使用を「scrollイベントからの`isNearBottomRef`更新」のみに限定し、ユーザー操作イベントからの更新はブロックしない。

5. **高頻度更新のスロットリング**: ストリーミング中の`smoothScrollToBottom`呼び出しを適切にスロットリングし、連続的な300msタイムアウト延長を防止。

## Testing Strategy

### Validation Approach

テスト戦略は2フェーズ: まず未修正コードでバグを再現するカウンターサンプルを表面化し、次に修正後のコードで正しい動作と既存動作の保持を検証する。

### Exploratory Bug Condition Checking

**Goal**: 修正前のコードでバグを再現し、根本原因分析を確認または反証する。反証された場合は再仮説が必要。

**Test Plan**: `shouldAutoScroll`関数の単体テストと、スクロール状態管理ロジックのシミュレーションテストを作成。未修正コードで実行し、失敗パターンを観察。

**Test Cases**:
1. **プログラムスクロール中のユーザー操作無視テスト**: `isProgrammaticScrollRef`がtrue時に`handleScroll`が呼ばれても`isNearBottomRef`が更新されないことを確認（未修正コードでバグ再現）
2. **高頻度ストリーミング更新テスト**: 50ms間隔でストリーミング更新をシミュレートし、ユーザーのスクロールアップが検知されないことを確認（未修正コードでバグ再現）
3. **300msタイムアウト連続延長テスト**: 連続的な`smoothScrollToBottom`呼び出しで`isProgrammaticScrollRef`が永続的にtrueになることを確認（未修正コードでバグ再現）

**Expected Counterexamples**:
- `isProgrammaticScrollRef`がtrue時にユーザーがスクロールアップしても`isNearBottomRef`がfalseにならない
- 原因: `handleScroll`の先頭で`isProgrammaticScrollRef`チェックによる早期リターン

### Fix Checking

**Goal**: バグ条件が成立する全入力に対し、修正後の関数が期待動作を生成することを検証。

**Pseudocode:**
```
FOR ALL input WHERE isBugCondition(input) DO
  scrollPosition_before := container.scrollTop
  triggerStreamingUpdate(input)
  scrollPosition_after := container.scrollTop
  ASSERT scrollPosition_before = scrollPosition_after
END FOR
```

### Preservation Checking

**Goal**: バグ条件が成立しない全入力に対し、修正後の関数が元の関数と同じ結果を生成することを検証。

**Pseudocode:**
```
FOR ALL input WHERE NOT isBugCondition(input) DO
  ASSERT originalScrollBehavior(input) = fixedScrollBehavior(input)
END FOR
```

**Testing Approach**: Property-based testingを推奨。理由:
- 入力ドメイン全体にわたり多数のテストケースを自動生成
- 手動ユニットテストでは見逃すエッジケースを検出
- 非バグ入力に対する動作不変を強力に保証

**Test Plan**: 未修正コードでまずマウスクリックやその他のインタラクションの動作を観察し、その動作を捕捉するproperty-based testを作成。

**Test Cases**:
1. **底付近での自動スクロール保持**: ユーザーが200px以内にいる状態でストリーミング更新 → 自動スクロール実行を確認
2. **セッション切り替え時の即時スクロール保持**: セッション切り替え後に即座に最下部へスクロールされることを確認
3. **手動スクロールダウンによる再有効化保持**: ユーザーが手動で最下部までスクロールした後、自動スクロールが再有効化されることを確認
4. **新メッセージ追加時のスクロール保持**: 底付近でメッセージ追加時にスムーズスクロールが実行されることを確認

### Unit Tests

- `shouldAutoScroll`関数の境界値テスト（200px閾値）
- ユーザー操作イベント（wheel上方向）による`isNearBottomRef`即時更新テスト
- `smoothScrollToBottom`のrAF内再チェックロジックテスト
- スロットリングロジックのタイミングテスト

### Property-Based Tests

- ランダムなスクロール位置とストリーミング状態の組み合わせで、バグ条件成立時にスクロール位置が変化しないことを検証（fast-check使用）
- ランダムなスクロール位置で底付近（<=200px）の場合に自動スクロールが実行されることを検証
- ランダムなタイミングでのユーザー操作とストリーミング更新の組み合わせで、ユーザー操作が常に検知されることを検証

### Integration Tests

- ストリーミング中にスクロールアップ → 位置維持 → 手動で最下部へ戻る → 自動スクロール再開のフルフロー
- セッション切り替え → ストリーミング開始 → スクロールアップ → 位置維持のフルフロー
- 高頻度ストリーミング更新中のユーザー操作応答性テスト
