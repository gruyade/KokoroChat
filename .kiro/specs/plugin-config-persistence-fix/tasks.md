# Plugin Config Persistence Bug Fix Tasks

## Phase 1: Command Update
- [x] `src-tauri/src/commands/plugin.rs` の `set_plugin_config` の修正
  - 既存の `state.plugin_registry.set_plugin_config(&name, config)` の後に、`state.config_manager.get_config()` を取得。
  - `app_config.plugins.plugin_settings.insert(name.clone(), config)` で値を更新。
  - `state.config_manager.set_config(app_config)` を呼び出してファイルへ永続化する。
- [x] `enable_plugin` / `disable_plugin` の永続化対応（必要な場合）
  - 同様に `app_config.plugins.enabled_plugins` のリストを更新し、`set_config` で永続化する処理を追加する。

## Phase 2: Verification
- [x] 設定の保存テスト
  - `web_search` プラグインなどの設定をUIから保存し、`config.json` に設定が書き込まれるか確認する。
- [x] プラグイン実行テスト
  - アプリを再起動してもAPIキーが維持され、Web検索ツールが正しく動作するか検証する。
