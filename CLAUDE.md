# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

Vow は「AIが書き、人間が承認し、コンパイラが履行を保証する」ことを前提に設計されたプログラミング言語。TypeScript へトランスパイルされる(ターゲット: V8 / Cloudflare Workers / Node)。実装は Rust の Cargo ワークスペース、ランタイム(`@vow/runtime`)のみ TS の npm パッケージ。

**現状: v0.1 実装フェーズ(M0〜M7)完了。** パーサ・意味検査・フォーマッタ・TS トランスパイラ・MCP サーバーが揃い、`vow` CLI の `check` / `fmt` / `build` / `test`(vow_cli)も実装済みで、`cargo test --workspace` が全件パスする。開発は `docs/vow-roadmap-goals.md` の Milestone に沿って /goal 単位で進める。残務はドッグフード実験(人間主導)。

## Source of Truth(必読)

- `spec/vow-spec-v0.1.md` — 言語仕様。仕様と実装が食い違ったら**仕様を先に直す**
- `ARCHITECTURE.md` — リポジトリ構成の契約。ディレクトリ・クレート追加時は必ずこのファイルも更新する
- `docs/vow-roadmap-goals.md` — Milestone 別の /goal 契約書集。🤝 マークは着手前に人間との設計合意が必要

## アーキテクチャ

6クレート構成。依存は一方向のみ(逆流・循環禁止):

```text
vow_syntax ←─ vow_fmt
     ↑
vow_check  ←─ vow_emit
     ↑              ↑
     └── vow_cli ──┘
     └── vow_mcp ──┘
```

- `vow_syntax` — レキサー+パーサ+AST。型の知識を持たない
- `vow_check` — 名前解決・型・エフェクト・契約検査。**Diagnostic 型の唯一の定義元**(他クレートは独自エラー型を外部に漏らさない)
- `vow_fmt` — 正規形フォーマッタ(AST の意味的変更禁止)
- `vow_emit` — TS トランスパイラ+source map(検査の再実装禁止)
- `vow_cli` / `vow_mcp` — 言語処理ロジックを持たない。CLI は Diagnostic の散文整形、MCP は spec/・examples/ のビルド時埋め込み配信
  - `vow_mcp` は実装済み(4 ツール: vow_spec / vow_check / vow_fmt / vow_examples)。`vow_cli` は **check / fmt / build / test 実装済み**(M6・M7)。`build` はディレクトリ単位の vow_emit 委譲、`test` は dev ビルド後にプロジェクトの `npm test` を起動する薄いラッパー(ランナーの知識を持たない)

## 不変条件

1. **tests/golden/ が契約本文。** golden test の追加・変更は人間レビュー必須。実装都合で expected を書き換えない
2. コンパイラ診断は JSON(構造化 Diagnostic)が正、散文は派生。全 Diagnostic に span・code・最低1つの fix 候補を含める
3. spec/ と examples/ は vow_mcp にビルド時埋め込み(仕様更新=MCP サーバー更新)
4. `runtime/` は Rust ワークスペース外の独立 npm パッケージ
5. 正規形を常に維持する。Rust コードは `cargo fmt`、`.vow`(examples/・golden)は正規形(vow_fmt)を保つ。`.vow` の整形は `vow fmt --write <file>`(または `--check` で検証)で行える

## コマンド

- `cargo test --workspace` — 全テスト。各 Milestone の完了条件(e2e は Node が必要)
- `cargo clippy --workspace --all-targets -- -D warnings` — 警告ゼロが必須
- `cargo fmt --all -- --check` — 整形チェック(CI の fmt ジョブと同じ)
- `cargo run -p vow_mcp --bin vow-mcp` — MCP サーバー起動(stdin から改行区切り JSON-RPC を読む)
- `cargo run -p vow_emit --example transpile -- <input.vow> [output.ts]` — 単一 .vow を TS 化(検査 NG は Diagnostic を出して exit 1)。デバッグ用の最小トランスパイラ(ディレクトリ単位は `vow build`)
- `cargo run -p vow_cli --bin vow -- check <file> [--json]` — 意味検査(既定は散文 Diagnostic、`--json` で `Diagnostic[]`)。エラーありで exit 1
- `cargo run -p vow_cli --bin vow -- fmt <file> [--check | --write]` — 正規形整形(既定は stdout、`--check` で未整形を exit 1 検出、`--write` で上書き)
- `cargo run -p vow_cli --bin vow -- build <dir> [--out-dir <dir>] [--no-source-map]` — `<dir>` 配下の全 .vow を検査し、エラーゼロのとき out-dir(既定 `<dir>/dist/`)に TS + source map を `ts_path` 通り配置。1 件でもエラーなら何も書かず exit 1
- `cargo run -p vow_cli --bin vow -- test [<dir>]` — dev ビルド(契約 on)後、プロジェクトの `npm test` に委譲(Node 必須)。契約違反は `VowContractViolation` として非ゼロ終了に伝播

CI(`.github/workflows/ci.yml`)は fmt / clippy / test の 3 ジョブ。test ジョブのみ Node 22 をセットアップする(e2e が npm/npx を使うため)。

## 言語設計の要点(コード生成時に守ること)

- null・例外なし。失敗は `Option<T>` / `Result<T, E>` のみ
- エフェクトはケーパビリティ。`uses` 宣言外のエフェクト使用はコンパイルエラー、呼び出し先から推移的に伝播
- `requires` / `ensures` は v0.1 では実行時アサーション。契約式は副作用禁止(将来の静的証明を壊さないため)
- import は全て明示。ワイルドカード・再エクスポート禁止。モジュールパスはファイルパスと 1:1
