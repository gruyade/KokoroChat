# Thought Delete Button Bugfix Design

## Overview

サイドバーの思考リスト（SidebarThoughtList.tsx）に削除ボタンが欠落しているバグの修正設計。同コンポーネントと同じサイドバー内の SidebarMemoryList.tsx には `group-hover` パターンで削除ボタンが実装済みであり、バックエンドの `delete_thought` コマンドも ThoughtView.tsx から既に利用されている。修正は SidebarMemoryList のパターンを SidebarThoughtList に適用するのみ。

## Glossary

- **Bug_Condition (C)**: サイドバー思考カードにホバーしても削除ボタンが表示されない状態
- **Property (P)**: ホバー時に削除ボタンが表示され、クリックで確認後に思考が削除される動作
- **Preservation**: 思考リストの読み込み・リアルタイム更新・レイアウト表示が変更前と同一であること
- **SidebarThoughtList**: `src/components/sidebar/SidebarThoughtList.tsx` — サイドバーで思考一覧を表示するコンポーネント
- **SidebarMemoryList**: `src/components/sidebar/SidebarMemoryList.tsx` — サイドバーで記憶一覧を表示するコンポーネント（削除ボタンのリファレンス実装）
- **delete_thought**: `src-tauri/src/commands/thought.rs` の Tauri コマンド。`{ id: String }` を受け取り思考を1件削除

## Bug Details

### Bug Condition

SidebarThoughtList.tsx の思考カードに削除ボタン要素が存在しない。ユーザーがホバーしても削除UIが表示されず、サイドバーから思考を削除する手段がない。

**Formal Specification:**
```
FUNCTION isBugCondition(input)
  INPUT: input of type UserInteraction
  OUTPUT: boolean

  RETURN input.component == "SidebarThoughtList"
         AND input.action == "hover_on_thought_card"
         AND deleteButtonElement == null
END FUNCTION
```

### Examples

- ユーザーが思考カード「今日は天気が良い」にホバー → 期待: ゴミ箱アイコン表示 / 実際: 何も表示されない
- ユーザーが思考カードを右クリックして削除を試みる → 期待: 削除手段がある / 実際: コンテキストメニューにも削除なし
- SidebarMemoryList の記憶カードにホバー → ゴミ箱アイコンが正常に表示される（参考：正常動作）
- ThoughtView で思考カードにホバー → ゴミ箱アイコンが正常に表示される（参考：正常動作）

## Expected Behavior

### Preservation Requirements

**Unchanged Behaviors:**
- 選択中キャラクターの思考一覧の読み込み・表示（`get_thoughts` 呼び出し）
- `thought:generated` イベントによるリアルタイム追加（リストの先頭に挿入、最大20件維持）
- 思考カードのレイアウト（content, context, created_at の表示順序・スタイル）
- ホバーしていない状態での見た目（削除ボタンは非表示）
- キャラクター未選択時・ローディング中・思考なし時の表示

**Scope:**
削除ボタンの追加以外のすべての既存動作は変更しない。具体的には：
- 思考の読み込みロジック（invoke, listen）
- 状態管理（useState, useEffect のフロー）
- CSS クラス・レイアウト構造（削除ボタン周辺を除く）

## Hypothesized Root Cause

実装漏れ。SidebarMemoryList を作成した際に削除ボタンパターンを含めたが、SidebarThoughtList には同パターンが適用されなかった。

1. **削除ボタン要素の欠落**: 思考カードの JSX に `<button>` + `<Trash2>` アイコンが存在しない
2. **handleDelete 関数の欠落**: 削除処理のイベントハンドラが定義されていない
3. **Trash2 アイコンの未インポート**: lucide-react から Trash2 がインポートされていない
4. **group クラスの欠落**: カード要素に `group` クラスがないため、`group-hover:opacity-100` が機能しない

## Correctness Properties

Property 1: Bug Condition - ホバー時に削除ボタンが表示される

_For any_ 思考カードに対するホバー操作において、修正後の SidebarThoughtList SHALL ゴミ箱アイコンの削除ボタンを表示し、クリック時に確認ダイアログを経て `delete_thought` コマンドを呼び出し、成功時にリストから該当思考を除去する。

**Validates: Requirements 2.1, 2.2**

Property 2: Preservation - 既存の表示・更新動作の維持

_For any_ 削除ボタン操作以外の入力（思考リスト読み込み、リアルタイム更新、ホバーなし閲覧）において、修正後の SidebarThoughtList SHALL 修正前と同一の動作を維持し、レイアウト・データフロー・イベントリスナーの挙動を保存する。

**Validates: Requirements 3.1, 3.2, 3.3**

## Fix Implementation

### Changes Required

**File**: `src/components/sidebar/SidebarThoughtList.tsx`

**Specific Changes**:

1. **Trash2 アイコンのインポート追加**: `lucide-react` のインポートに `Trash2` を追加
   - `import { Lightbulb, Loader2, Trash2 } from 'lucide-react';`

2. **handleDelete 関数の追加**: SidebarMemoryList と同パターンの削除ハンドラ
   - `e.stopPropagation()` でイベント伝播を防止
   - `confirm()` で確認ダイアログ表示
   - `invoke('delete_thought', { id })` でバックエンド呼び出し
   - 成功時に `setThoughts` で該当思考をフィルタ除去

3. **カード要素に `group` クラス追加**: ホバー検出のため
   - `<div key={thought.id} className="group p-2.5 rounded-lg ...">` に変更

4. **flex レイアウトの適用**: content 部分と削除ボタンを横並びに配置
   - `<div className="flex items-start justify-between gap-1">` ラッパー追加

5. **削除ボタン要素の追加**: SidebarMemoryList と同一スタイルのボタン
   - `opacity-0 group-hover:opacity-100` でホバー時のみ表示
   - `hover:bg-destructive/10 hover:text-destructive` で削除色のフィードバック
   - `<Trash2 className="w-3 h-3" />` アイコン

## Testing Strategy

### Validation Approach

2段階のテスト戦略：まず未修正コードでバグの存在を確認し、次に修正後に正しい動作と既存動作の保存を検証する。

### Exploratory Bug Condition Checking

**Goal**: 未修正コードで削除ボタンが存在しないことを確認し、根本原因を裏付ける。

**Test Plan**: SidebarThoughtList をレンダリングし、思考カード内に削除ボタン要素が存在するか検査する。未修正コードでは失敗する。

**Test Cases**:
1. **削除ボタン存在テスト**: 思考カードをレンダリングし、Trash2 アイコンまたは「削除」title のボタンを検索（未修正コードで失敗）
2. **group-hover クラステスト**: カード要素に `group` クラスが存在するか検査（未修正コードで失敗）
3. **handleDelete 関数テスト**: 削除ボタンクリック時に `delete_thought` が呼ばれるか検証（未修正コードで失敗）

**Expected Counterexamples**:
- 削除ボタン要素が DOM に存在しない
- 原因: JSX に削除ボタンが実装されていない

### Fix Checking

**Goal**: バグ条件が成立するすべての入力に対し、修正後の関数が期待動作を生成することを検証。

**Pseudocode:**
```
FOR ALL thought IN renderedThoughts DO
  hover(thoughtCard)
  ASSERT deleteButton.isVisible()
  click(deleteButton)
  ASSERT confirmDialog.shown()
  confirmDialog.accept()
  ASSERT invoke.calledWith('delete_thought', { id: thought.id })
  ASSERT thought NOT IN currentThoughts
END FOR
```

### Preservation Checking

**Goal**: バグ条件が成立しない入力に対し、修正前後で同一の動作を維持することを検証。

**Pseudocode:**
```
FOR ALL interaction WHERE NOT isBugCondition(interaction) DO
  ASSERT SidebarThoughtList_fixed(interaction) == SidebarThoughtList_original(interaction)
END FOR
```

**Testing Approach**: Property-based testing を推奨。多様な思考データ・キャラクター状態を自動生成し、削除ボタン以外の表示が変わらないことを網羅的に検証。

**Test Plan**: 未修正コードで思考リストの表示・更新動作を観察し、修正後も同一であることを property-based test で保証。

**Test Cases**:
1. **思考リスト読み込み保存**: 任意のキャラクターIDで `get_thoughts` 呼び出し結果が正しく表示されることを検証
2. **リアルタイム更新保存**: `thought:generated` イベント発火時にリスト先頭に追加され、20件上限が維持されることを検証
3. **レイアウト保存**: content, context, created_at の表示順序・スタイルが変わらないことを検証
4. **空状態・ローディング保存**: 思考なし・読み込み中の表示が変わらないことを検証

### Unit Tests

- 削除ボタンのレンダリング確認（ホバー時に表示、非ホバー時に非表示）
- handleDelete の confirm → invoke → state 更新フロー
- 削除失敗時のエラーハンドリング（state が変わらないこと）
- e.stopPropagation() が呼ばれることの確認

### Property-Based Tests

- ランダムな思考データ配列を生成し、全カードに削除ボタンが存在することを検証
- ランダムな思考データで削除後のリスト長が1減ることを検証
- ランダムな思考データでレイアウト構造（content → context → date の順序）が保存されることを検証

### Integration Tests

- 思考生成 → サイドバー表示 → 削除 → リスト更新の一連フロー
- 複数思考の連続削除
- 削除後に `thought:generated` イベントで新規思考が正しく追加されること
