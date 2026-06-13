# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

Pact は「AIが書き、人間が承認し、コンパイラが履行を保証する」ことを前提に設計されたプログラミング言語。TypeScript へトランスパイルされる(ターゲット: V8 / Cloudflare Workers / Node)。実装は Rust の Cargo ワークスペース、ランタイム(`@pact/runtime`)のみ TS の npm パッケージ。

**現状: v0.1 実装フェーズ(M0〜M5)完了。** パーサ・意味検査・フォーマッタ・TS トランスパイラ・MCP サーバーが揃い、`cargo test --workspace` が全件パスする。開発は `docs/pact-roadmap-goals.md` の Milestone に沿って /goal 単位で進める。残務は `pact_cli`(`pact` バイナリのサブコマンド)が未実装でスタブのままな点と、ドッグフード実験(人間主導)。

## Source of Truth(必読)

- `spec/pact-spec-v0.1.md` — 言語仕様。仕様と実装が食い違ったら**仕様を先に直す**
- `ARCHITECTURE.md` — リポジトリ構成の契約。ディレクトリ・クレート追加時は必ずこのファイルも更新する
- `docs/pact-roadmap-goals.md` — Milestone 別の /goal 契約書集。🤝 マークは着手前に人間との設計合意が必要

## アーキテクチャ

6クレート構成。依存は一方向のみ(逆流・循環禁止):

```text
pact_syntax ←─ pact_fmt
     ↑
pact_check  ←─ pact_emit
     ↑              ↑
     └── pact_cli ──┘
     └── pact_mcp ──┘
```

- `pact_syntax` — レキサー+パーサ+AST。型の知識を持たない
- `pact_check` — 名前解決・型・エフェクト・契約検査。**Diagnostic 型の唯一の定義元**(他クレートは独自エラー型を外部に漏らさない)
- `pact_fmt` — 正規形フォーマッタ(AST の意味的変更禁止)
- `pact_emit` — TS トランスパイラ+source map(検査の再実装禁止)
- `pact_cli` / `pact_mcp` — 言語処理ロジックを持たない。CLI は Diagnostic の散文整形、MCP は spec/・examples/ のビルド時埋め込み配信
  - `pact_mcp` は実装済み(4 ツール: pact_spec / pact_check / pact_fmt / pact_examples)。`pact_cli` は**まだスタブ**(M0〜M5 のスコープ外)

## 不変条件

1. **tests/golden/ が契約本文。** golden test の追加・変更は人間レビュー必須。実装都合で expected を書き換えない
2. コンパイラ診断は JSON(構造化 Diagnostic)が正、散文は派生。全 Diagnostic に span・code・最低1つの fix 候補を含める
3. spec/ と examples/ は pact_mcp にビルド時埋め込み(仕様更新=MCP サーバー更新)
4. `runtime/` は Rust ワークスペース外の独立 npm パッケージ
5. 正規形を常に維持する。Rust コードは `cargo fmt`、`.pact`(examples/・golden)は正規形(pact_fmt)を保つ。`pact fmt` バイナリは pact_cli 実装後に置き換える

## コマンド

- `cargo test --workspace` — 全テスト。各 Milestone の完了条件(e2e は Node が必要)
- `cargo clippy --workspace --all-targets -- -D warnings` — 警告ゼロが必須
- `cargo fmt --all -- --check` — 整形チェック(CI の fmt ジョブと同じ)
- `cargo run -p pact_mcp --bin pact-mcp` — MCP サーバー起動(stdin から改行区切り JSON-RPC を読む)
- `cargo run -p pact_emit --example transpile -- <input.pact> [output.ts]` — 単一 .pact を TS 化(検査 NG は Diagnostic を出して exit 1)。`pact build` 実装までの簡易トランスパイラ
- `pact check <file>` / `pact fmt <file>` / `pact build <dir>` / `pact test` — `pact` CLI。**未実装(pact_cli はスタブ)**

CI(`.github/workflows/ci.yml`)は fmt / clippy / test の 3 ジョブ。test ジョブのみ Node 22 をセットアップする(e2e が npm/npx を使うため)。

## 言語設計の要点(コード生成時に守ること)

- null・例外なし。失敗は `Option<T>` / `Result<T, E>` のみ
- エフェクトはケーパビリティ。`uses` 宣言外のエフェクト使用はコンパイルエラー、呼び出し先から推移的に伝播
- `requires` / `ensures` は v0.1 では実行時アサーション。契約式は副作用禁止(将来の静的証明を壊さないため)
- import は全て明示。ワイルドカード・再エクスポート禁止。モジュールパスはファイルパスと 1:1
