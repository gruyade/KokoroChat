# Plugin Config Persistence Bug Fix

## 1. バグの概要
ユーザーがフロントエンドの `SettingsView` でプラグイン（例：`web_search`）の設定（APIキーなど）を保存しても、プラグインの実行時に設定値が読み込まれず、「APIキー未設定」として扱われてしまう不具合。

## 2. 原因の分析
- フロントエンドから呼び出される `set_plugin_config` コマンド（`src-tauri/src/commands/plugin.rs`）が、インメモリの `PluginRegistry` (`state.plugin_registry`) に対してのみ設定値を書き込んでいる。
- 一方、`WebSearchPlugin` などは `ModelConfigManager` (`app_config.plugins.plugin_settings`) を経由してディスク上の `config.json` に永続化された設定を読み込みに行くアーキテクチャとなっている。
- 結果として、UIからの設定保存がディスク(`AppConfig`)に反映されず、プラグインの実行時に古い（あるいは空の）設定が読み込まれる状態になっている。

## 3. 修正方針 (Fix A: コマンド側での永続化)
- `src-tauri/src/commands/plugin.rs` 内の `set_plugin_config` コマンドを修正し、`PluginRegistry` の更新に加えて、`ModelConfigManager` 経由で `AppConfig` を更新・永続化する処理を追加する。
- 同じく、`enable_plugin`, `disable_plugin` などの状態変更コマンドでも、永続化のための `AppConfig` 更新が必要な場合は合わせて修正を行う。

## 4. 影響範囲
- `src-tauri/src/commands/plugin.rs`
- 既存のプラグインの動作への破壊的変更はなく、むしろ全プラグインの設定が再起動後も正しく保持されるようになる。
