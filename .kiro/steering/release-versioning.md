---
inclusion: fileMatch
fileMatchPattern: '.github/workflows/release.yml'
---

# リリースバージョニングルール

## masterマージ前のバージョンアップ手順

featureブランチからmasterへマージする前に、以下のバージョン更新を行うこと。

### 更新対象ファイル

以下の2ファイルの `version` フィールドを同じ値に揃える:

1. `src-tauri/tauri.conf.json` → `"version": "X.Y.Z"`
2. `package.json` → `"version": "X.Y.Z"`

### バージョニング規則（Semantic Versioning）

| 変更種別 | バージョン上げ方 | 例 |
|----------|-----------------|-----|
| 破壊的変更（データ形式変更、API互換性なし） | メジャー (X) | 0.1.0 → 1.0.0 |
| 新機能追加 | マイナー (Y) | 0.1.0 → 0.2.0 |
| バグ修正・軽微な改善 | パッチ (Z) | 0.1.0 → 0.1.1 |

### 注意事項

- masterへのpushでGitHub Actionsがリリースを自動作成する
- リリースタグは `v{version}`（例: `v0.2.0`）
- 同じバージョンで再pushすると既存リリースが上書きされる
- バージョンを上げ忘れると前回リリースが上書きされるため、必ずマージ前に更新すること

### チェックリスト

masterマージ前に確認:
- [ ] `src-tauri/tauri.conf.json` の version を更新した
- [ ] `package.json` の version を更新した
- [ ] 両ファイルのバージョンが一致している
- [ ] コミットメッセージに `build: バージョンをX.Y.Zに更新` を含めた
