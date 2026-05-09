# Bugfix Requirements Document

## Introduction

サイドバーの思考リスト（SidebarThoughtList.tsx）に削除ボタンが実装されていないバグ。同じサイドバー内の記憶リスト（SidebarMemoryList.tsx）には削除ボタンがホバー時に表示されるパターンで実装済みであり、メインビュー（ThoughtView.tsx）にも思考の削除機能が存在する。サイドバーの思考リストだけが削除UIを欠いており、ユーザーはサイドバーから思考を削除できない。

## Bug Analysis

### Current Behavior (Defect)

1.1 WHEN ユーザーがサイドバーの思考カードにマウスをホバーする THEN the system 削除ボタンが表示されない（ボタン要素自体が存在しない）
1.2 WHEN ユーザーがサイドバーから思考を削除しようとする THEN the system 削除手段が提供されず、操作できない

### Expected Behavior (Correct)

2.1 WHEN ユーザーがサイドバーの思考カードにマウスをホバーする THEN the system SHALL ゴミ箱アイコンの削除ボタンを表示する（SidebarMemoryListと同じホバー表示パターン）
2.2 WHEN ユーザーが思考カードの削除ボタンをクリックする THEN the system SHALL 確認ダイアログを表示し、確認後に `delete_thought` コマンドを呼び出して思考を削除し、リストから該当項目を除去する

### Unchanged Behavior (Regression Prevention)

3.1 WHEN ユーザーがサイドバーの思考リストを表示する THEN the system SHALL CONTINUE TO 選択中キャラクターの思考一覧を正しく読み込み表示する
3.2 WHEN `thought:generated` イベントが発生する THEN the system SHALL CONTINUE TO リアルタイムで新しい思考をリストの先頭に追加する
3.3 WHEN ユーザーが思考カードにホバーせずに閲覧する THEN the system SHALL CONTINUE TO 削除ボタンを非表示にし、既存のレイアウト（内容・コンテキスト・日時）を維持する
