---
inclusion: manual
---

# プロジェクト固有: CI・リリース設定

このファイルはマージワークフロー（merge-workflow.md）から参照される、
プロジェクト固有のCI設定・バージョニング・リリース情報。

---

## CIチェックコマンド

### バックエンド（Rust / Tauri）

```bash
# フォーマットチェック
cd src-tauri && cargo fmt --check

# 静的解析（警告をエラーとして扱う）
cargo clippy -- -D warnings

# テスト実行（環境変数テストの競合回避のためシリアル実行）
cargo test --lib -- --test-threads=1
```

### フロントエンド（TypeScript/React）

```bash
# ESLint
pnpm lint

# 型チェック
pnpm type-check

# テスト
pnpm test
```

---

## よくある失敗パターンと対処

| 失敗 | 原因 | 対処 |
|------|------|------|
| `cargo fmt --check` | フォーマット未適用 | `cargo fmt` を実行 |
| `cargo clippy` dead_code | 未使用フィールド | `#[allow(dead_code)]` または削除 |
| `cargo clippy` too_many_arguments | 引数8個以上 | `#[allow(clippy::too_many_arguments)]` |
| `cargo clippy` type_complexity | 複雑な型 | `#[allow(clippy::type_complexity)]` または型エイリアス定義 |
| `cargo clippy` vec_init_then_push | Vec::new()後に即push | `vec![...]` マクロに書き換え |
| `pnpm lint` no-undef | ブラウザAPI未定義 | `eslint.config.js` の globals に追加 |
| `cargo test` 環境変数テスト失敗 | 並列実行での競合 | `--test-threads=1` で実行 |
| `pnpm tauri build` tauri not found | @tauri-apps/cli未インストール | `pnpm add -D @tauri-apps/cli` |
| アイコンファイル不在 | tauri.conf.jsonが存在しないファイルを参照 | bundle.icon を実在ファイルのみに修正 |

---

## バージョン更新対象ファイル

以下の2ファイルの `version` フィールドを同じ値に揃える:

1. `src-tauri/tauri.conf.json` → `"version": "X.Y.Z"`
2. `package.json` → `"version": "X.Y.Z"`

### 注意事項

- masterへのpushでGitHub Actionsがリリースを自動作成する
- リリースタグは `v{version}`（例: `v0.2.0`）
- 同じバージョンで再pushすると既存リリースが上書きされる
- バージョンを上げ忘れると前回リリースが上書きされるため、必ずマージ前に更新すること

---

## リリースノートのフォーマット

GitHub Actionsの `.github/workflows/release.yml` の `body` セクションに反映する。

```markdown
## KokoroChat vX.Y.Z

### 変更点

#### 新機能
- [feat コミットから抽出]

#### バグ修正
- [fix コミットから抽出]

#### 改善
- [refactor/perf コミットから抽出]

### ダウンロード

Windows用インストーラー (.exe) をダウンロードして実行してください。

### ドキュメント

`USER_GUIDE.md` — アプリの使い方ガイド（Google Gemini無料API設定手順を含む）
```

---

## CIチェックリスト

- [ ] `cargo fmt --check` 通過
- [ ] `cargo clippy -- -D warnings` 通過
- [ ] `cargo test --lib -- --test-threads=1` 全通過
- [ ] `pnpm lint` 通過
- [ ] `pnpm type-check` 通過
- [ ] `pnpm test` 全通過
- [ ] `src-tauri/tauri.conf.json` の version 更新済み
- [ ] `package.json` の version 更新済み
- [ ] 両ファイルのバージョン一致
- [ ] リリースノート更新済み
- [ ] マージコミットメッセージに変更点含む
