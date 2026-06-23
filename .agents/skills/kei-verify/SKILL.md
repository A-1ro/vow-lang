---
name: kei-verify
description: Kei コンパイラ(この Rust ワークスペース)の Milestone 完了条件を一括検証する。cargo fmt --check → clippy 警告ゼロ → cargo test --workspace を順に回し、落ちた所だけ要約する。/goal の完了判定、コミット/PR 前、CI を手元で再現したいときに使う。
---

# kei-verify — Kei の完了条件を一括検証

Kei の各 Milestone / `/goal` の完了条件は「`cargo test --workspace` 全件パス・clippy 警告ゼロ・正規形維持」。CI(`.github/workflows/ci.yml`)の **fmt / clippy / test** 3 ジョブと同じものを手元で再現する。

## 実行順(速く落ちる順)
リポジトリルートで次を順に実行する。**前段が失敗しても残りも実行**し、最後にまとめて報告する(どこが壊れているか一望するため)。

1. **整形** — `cargo fmt --all -- --check`
   失敗 = 未整形の Rust。`.kei`(examples/・golden)の整形は別系統 → 下の「.kei の正規形」を見る。
2. **lint** — `cargo clippy --workspace --all-targets -- -D warnings`
   警告ゼロが必須。1 件でも警告で失敗扱い。
3. **テスト** — `cargo test --workspace`
   golden / 単体 / 統合 / e2e を含む。**e2e は Node が必要**(npm/npx を使う)。Node が無い環境では e2e 関連がスキップ/失敗しうるので、その旨を報告に明記する。

### `.kei` の正規形(examples/ や golden の .kei を触ったとき)
- `cargo run -p kei_cli --bin kei -- fmt <file> --check` で未整形を検出(`--write` で整形)。
- `cargo run -p kei_cli --bin kei -- check <file>` で意味検査(エラーありで exit 1)。

## 報告フォーマット
各ステップを `pass` / `fail` / `skip(理由)` で示し、`fail` のときだけ出力の該当箇所を抜粋する。

```
## kei-verify 結果
- fmt    : pass | fail
- clippy : pass | fail
- test   : pass | fail (Node 不在なら e2e を skip と明記)

<fail があれば、その出力の要点を貼る>
判定: 完了条件を満たす / 満たさない(残課題: ...)
```

## 失敗時の鉄則(AGENTS.md の不変条件)
- **golden test の expected を実装都合で書き換えて通さない。** `tests/golden/` 等の期待値変更は人間レビュー必須(不変条件1)。テストが落ちたら基本は実装側を直す。
- **仕様と実装が食い違ったら spec を先に直す。** `spec/` が source of truth。
- clippy はワークアラウンドの `#[allow(...)]` 乱用で黙らせない。原因を直す。

この検証はあくまで完了条件の確認。**不変条件そのものの監査(golden 改変・依存逆流・Diagnostic 要件・spec 同期)は `kei-invariant-auditor` サブエージェントの担当**。両者を併用すると `/goal` 完了の品質ゲートになる。
