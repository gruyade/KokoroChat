---
inclusion: manual
---

# masterマージ前のCIチェック手順

masterへマージする前に、以下のチェックをすべてローカルで実行し、全通過を確認すること。

## バックエンド（Rust）

```bash
# 1. フォーマットチェック
cd src-tauri
cargo fmt --check

# 2. 静的解析（警告をエラーとして扱う）
cargo clippy -- -D warnings

# 3. テスト実行（環境変数テストの競合回避のためシリアル実行）
cargo test --lib -- --test-threads=1
```

## フロントエンド（TypeScript/React）

```bash
# 4. ESLint
pnpm lint

# 5. 型チェック
pnpm type-check

# 6. テスト
pnpm test
```

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

## チェックリスト

masterマージ前に確認:
- [ ] `cargo fmt --check` 通過
- [ ] `cargo clippy -- -D warnings` 通過
- [ ] `cargo test --lib -- --test-threads=1` 全通過
- [ ] `pnpm lint` 通過
- [ ] `pnpm type-check` 通過
- [ ] `pnpm test` 全通過
- [ ] バージョン更新済み（release-versioning.md参照）
