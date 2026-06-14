<p align="center">
  <img src="assets/bow-kun-256.png" alt="Kei のマスコット bow-kun(契約書を咥えた柴犬)" width="160">
</p>

# Kei

> **Code is a 契 between humans and AI.**
> コードは人間とAIの契である。

Kei は「**AIが書き、人間が承認し、コンパイラが履行を保証する**」ことを前提に設計されたプログラミング言語です。人間の書きやすさを捨て、**検証可能性・推論の局所性・エージェントループとの親和性**に全振りしています。

- **ターゲット**: TypeScript へトランスパイル(V8 / Cloudflare Workers / Node)
- **実装**: Rust の Cargo ワークスペース。ランタイム(`@kei/runtime`)のみ TypeScript の npm パッケージ
- **ツールチェイン**: `kei` CLI と Kei MCP Server を言語仕様と同格の一級市民として扱う

> ✅ **ステータス: v0.1 実装フェーズ(M0〜M7)完了。** 言語処理(パーサ〜トランスパイラ)と MCP サーバーが動作し、`kei` CLI バイナリ(`kei_cli`)は `check` / `fmt` / `build` / `test` が使えます。`cargo test --workspace` は全件パス。残務はドッグフード実験(人間主導)。仕様は `spec/kei-spec-v0.1.md`(Draft)が正本です。

---

## 設計思想

| 原則 | 中身 |
|---|---|
| **合意書原則** | シグネチャ + 契約(`uses` / `requires` / `ensures`)だけ読めば、body を読まずに承認判断できる。レビュー単位は実装ではなく契約 |
| **曖昧さゼロ** | 暗黙の型変換なし・暗黙の import なし・演算子オーバーロードなし |
| **推論の局所性** | 1 ファイルの挙動は、そのファイルと明示 import の宣言だけで決まる。再エクスポート禁止、グローバル可変状態なし |
| **正規形唯一** | 同じ AST は常に同じテキストに整形される(`kei fmt`)。スタイル論争は仕様で殺す |
| **エラーは構造化データ** | 診断は JSON が正、散文は派生。生成→検証→修正ループの帯域を最大化する |
| **null 不在** | `Option<T>` / `Result<T, E>` のみ。例外機構なし。失敗は型に現れる |

## コード例

```kei
module contracts.withdraw

import core.money { AccountId, Money }
import infra.database as Database

enum WithdrawError {
  NotFound(AccountId)
  Overdraft { limit: Money }
}

func withdraw(account: AccountId, amount: Money) -> Result<Money, WithdrawError>
  uses Database.Read, Database.Write
  requires amount > Money.zero
  ensures result.isOk implies amount > Money.zero
{
  let current = Database.fetchBalance(account) else fail WithdrawError.NotFound(account)
  if current < amount {
    return Err(WithdrawError.Overdraft { limit: current })
  }
  Database.setBalance(account, current - amount)
  return Ok(current - amount)
}
```

- **エフェクトはケーパビリティ**: `uses` に宣言したエフェクトしか行使できない。呼び出し先から推移的に伝播し、未宣言の使用はコンパイルエラー
- **契約**: `requires` / `ensures` は v0.1 では実行時アサーションに展開される。契約式は副作用禁止(将来の静的証明を壊さないため)
- **失敗の表現**: `else fail` と `Result` / `Option`。null も例外もない

ほかのサンプルは [`examples/`](examples/)(basics / effects / contracts)にあります。

---

## リポジトリ構成

6 クレートの Cargo ワークスペース。依存は一方向のみ(逆流・循環禁止):

```text
kei_syntax ←─ kei_fmt
     ↑
kei_check  ←─ kei_emit
     ↑              ↑
     └── kei_cli ──┘
     └── kei_mcp ──┘
```

| クレート | 役割 | 状態 |
|---|---|---|
| [`kei_syntax`](crates/kei_syntax) | レキサー + パーサ + AST(型の知識を持たない、エラー回復対応) | ✅ |
| [`kei_check`](crates/kei_check) | 名前解決・型・エフェクト・契約検査。**Diagnostic 型の唯一の定義元** | ✅ |
| [`kei_fmt`](crates/kei_fmt) | 正規形フォーマッタ(AST の意味的変更禁止) | ✅ |
| [`kei_emit`](crates/kei_emit) | TS トランスパイラ + source map(検査の再実装禁止) | ✅ |
| [`kei_mcp`](crates/kei_mcp) | MCP サーバー。spec/・examples/ をビルド時埋め込み配信 | ✅ |
| [`kei_cli`](crates/kei_cli) | `kei` バイナリ。check / fmt / build / test 実装済み | ✅ |

そのほか:

- [`runtime/`](runtime) — `@kei/runtime`(Rust ワークスペース外の独立 npm パッケージ。Result/Option/契約アサーション)
- [`spec/`](spec) — 言語仕様(source of truth)。`kei-spec-v0.1.md` / `diagnostic-schema.md` / `errors/*.md`(エラーコード 1 つにつき 1 ファイル)
- [`examples/`](examples) — `.kei` サンプル集(`kei_examples` の配信元、常に check-clean)
- [`skills/`](skills) — Claude Code 向けスキル(`kei/SKILL.md`。エージェントが Kei を書くための取説、全例 check-clean)
- [`.claude-plugin/`](.claude-plugin) — プラグイン manifest + marketplace 定義(スキルを配布可能プラグインとして公開)
- [`tests/`](tests) — `golden/`(契約本文)/ `e2e/`(トランスパイル→tsc→vitest)/ `mcp/`(MCP 統合 golden)

詳細は [`ARCHITECTURE.md`](ARCHITECTURE.md) を参照(リポジトリ構成の**契約**)。

---

## ビルドとテスト

要件: stable Rust(`rust-toolchain.toml` で固定、rustfmt + clippy 同梱)。e2e テストのみ Node.js 22 が必要。

```bash
# 全テスト(各 Milestone の完了条件)
cargo test --workspace

# 警告ゼロが必須
cargo clippy --workspace --all-targets -- -D warnings

# 整形チェック(CI の fmt ジョブと同じ)
cargo fmt --all -- --check

# ランタイム(TypeScript)をビルド
cd runtime && npm install && npm run build
```

CI(`.github/workflows/ci.yml`)は **fmt / clippy / test** の 3 ジョブ。test ジョブは Node 22 をセットアップする(e2e が npm/npx を使うため)。

テスト規模(現状): golden — syntax 18 / check 17 / fmt 8 ペア、MCP 16 ペア。e2e vitest 5 本。エラーコード解説 22 本。

---

## `kei` CLI

ツールチェインのコマンドラインフロントエンド。`check` / `fmt` / `build` / `test` の 4 サブコマンドを提供します。

```bash
# 1. ビルド済みバイナリ(Rust 不要・macOS / Linux)
curl -fsSL https://raw.githubusercontent.com/A-1ro/kei-lang/main/install.sh | sh

# 2. cargo install(Rust ユーザー・クローン不要)
cargo install --git https://github.com/A-1ro/kei-lang.git kei_cli

# 3. インストールせず実行(開発中)
cargo run -p kei_cli --bin kei -- check examples/basics/options.kei
```

```bash
kei check <file> [--json]                        # 意味検査(既定は散文、--json で Diagnostic[])
kei fmt <file> [--check | --write]               # 正規形整形(既定は stdout、--check は検証、--write は上書き)
kei build <dir> [--out-dir <dir>] [--no-source-map]  # <dir> 配下の全 .kei を検査し TS + source map を出力(既定 <dir>/dist/)
kei test [<dir>]                                 # dev ビルド(契約 on)後、プロジェクトの npm test を起動(Node 必須)
```

終了コードは `0` 成功(検査エラーなし / 整形済み / ビルド成功 / テスト全件パス)/ `1` 診断エラー(check・build)・未整形(fmt --check)・構文エラー・テスト失敗(test)/ `2` 使用法エラー。
インストールの他の方法やサブコマンドの詳細は [`docs/cli.md`](docs/cli.md) を参照。

---

## Claude Code プラグイン(Kei skill)

[Claude Code](https://claude.com/claude-code) のエージェントに Kei を「学習なしで」書かせるためのスキルを、配布可能なプラグインとして同梱しています。文法・契約・エフェクト・型・失敗処理の最小十分セットと頻出エラーの直し方を**一括ロード**し、書いたコードは `kei check` の検証ループで仕上げます。

```text
# Claude Code 上で実行
/plugin marketplace add A-1ro/kei-lang
/plugin install kei@kei-lang
```

インストール後、`.kei` を書く場面でスキル(起動名 `kei`)が自動的に効きます。スキル本体は [`skills/kei/SKILL.md`](skills/kei/SKILL.md)、配布用の manifest は [`.claude-plugin/`](.claude-plugin)(`plugin.json` + `marketplace.json`)。

- **スキル(本セクション)** — 静的な知識をプロンプトに一括ロード(ラウンドトリップなし)。Claude Code でのコード生成はこちらが最短経路。
- **MCP サーバー(次セクション)** — 同じ取説を JSON-RPC で動的に引くサーバー。任意のエージェント・即引きリファレンス用途。

両者は併存します。スキルが「書く前に読み切る取説」、MCP が「必要なときに引くサーバー」、`kei check` が「書いた後の検証」という役割分担です。

---

## Kei MCP Server

エージェントが Kei を「学習なしで」書けるようにするための取扱説明書サーバー。stdio トランスポート上の JSON-RPC 2.0(改行区切り)で動きます。

```bash
cargo run -p kei_mcp --bin kei-mcp
```

提供ツール(spec §6.1 準拠):

| tool | input | 役割 |
|---|---|---|
| `kei_spec` | `topic` | 仕様セクション・エラーコード解説の即引き(空 topic で索引) |
| `kei_check` | `source` | 構文+型+エフェクト+契約の静的検査 → `Diagnostic[]`(JSON、修正候補つき) |
| `kei_fmt` | `source` | 正規形整形。構文エラー時は整形せず `Diagnostic[]` を返す |
| `kei_examples` | `query` | イディオム検索(パス・本文を部分一致)。空 query で一覧 |

**spec/ と examples/ はビルド時に埋め込まれます**(`include_str!`)。仕様を更新して再ビルドすれば応答も変わる——「仕様の更新 = 取説サーバーの更新」が保証されます(`crates/kei_mcp/tests/embedding.rs` で検証)。

最小の動作確認(エラーコード KEI-E3001 の解説を引く):

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"kei_spec","arguments":{"topic":"KEI-E3001"}}}' \
  | cargo run -q -p kei_mcp --bin kei-mcp
```

---

## ドキュメント

- [`spec/kei-spec-v0.1.md`](spec/kei-spec-v0.1.md) — 言語仕様(source of truth。仕様と実装が食い違ったら**仕様を先に直す**)
- [`spec/diagnostic-schema.md`](spec/diagnostic-schema.md) — 構造化 Diagnostic スキーマとエラーコード採番ルール
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — リポジトリ構成の契約とクレート責務・依存規則
- [`docs/cli.md`](docs/cli.md) — `kei` CLI のインストールとサブコマンド(check / fmt / build / test)の使い方
- [`skills/kei/SKILL.md`](skills/kei/SKILL.md) — Claude Code 向けの Kei 取説スキル(配布可能プラグインとして同梱)
- [`docs/kei-roadmap-goals.md`](docs/kei-roadmap-goals.md) — Milestone 別の /goal 契約書集
- [`CLAUDE.md`](CLAUDE.md) — Claude Code 向けの作業ガイド

## ライセンス

MIT
