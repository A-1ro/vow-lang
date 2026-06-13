# `vow` CLI

`vow` は Vow ツールチェインのコマンドラインフロントエンド(クレート [`vow_cli`](../crates/vow_cli))。
言語処理ロジックは持たず、引数解釈・ファイル IO・Diagnostic の散文整形だけを担い、
検査・整形は `vow_check` / `vow_fmt` / `vow_syntax` に委譲する。

> **ステータス: check / fmt 実装済み(M6)。** `build` / `test` は M7 で未実装(呼ぶと案内を出して終了コード 2)。

---

## インストール

要件: stable Rust(`rust-toolchain.toml` で固定)。CLI 自体に Node.js は不要。

### 1. ローカルからインストール(推奨)

リポジトリのルートで:

```bash
cargo install --path crates/vow_cli
```

- `~/.cargo/bin/vow` に `vow` バイナリが入る(`[[bin]] name = "vow"`)。
- `~/.cargo/bin` が `PATH` にあれば、以降はどこでも `vow check ...` / `vow fmt ...` で使える。
- アンインストールは `cargo uninstall vow_cli`。

### 2. インストールせず使う(開発中)

ビルドし直しが効くので、リポジトリ内での開発にはこちらが手軽:

```bash
cargo run -p vow_cli --bin vow -- check <file>
```

`--` より後ろが `vow` に渡る引数。

### 3. リリースバイナリを直接置く

`cargo install` を使わず成果物だけ欲しい場合:

```bash
cargo build --release -p vow_cli
# 生成物: target/release/vow  → PATH 上の任意のディレクトリにコピー
```

---

## サブコマンド

### `vow check <file> [--json]` — 意味検査

名前解決・型・エフェクト・契約を検査する。

| 形式 | 出力 |
|---|---|
| `vow check a.vow` | 既定。散文 Diagnostic(エラーなしなら無出力) |
| `vow check a.vow --json` | 構造化 `Diagnostic[]`(エラーなしなら `[]`) |

```bash
# エラーなし → 何も出さず exit 0
vow check examples/basics/options.vow

# 機械処理向け(span / code / fix 候補入りの JSON)
vow check examples/basics/options.vow --json   # → []
```

`--json` 出力の各要素は `spec/diagnostic-schema.md` の構造化 Diagnostic(span・code・最低 1 つの fix 候補を含む)。

### `vow fmt <file> [--check | --write]` — 正規形整形

`--check` と `--write` は排他。

| 形式 | 動作 |
|---|---|
| `vow fmt a.vow` | 既定。整形結果を **stdout** に出力(非破壊) |
| `vow fmt a.vow --check` | 整形済みか検証。未整形なら差分を出して **exit 1** |
| `vow fmt a.vow --write` | ファイルを整形結果で**上書き** |

```bash
vow fmt examples/basics/options.vow            # 整形結果を表示
vow fmt examples/basics/options.vow --check    # CI 向け(整形済みなら exit 0)
vow fmt examples/basics/options.vow --write    # その場で正規形に
```

構文エラーがあると整形せず Diagnostic を出して exit 1。

### `vow help` / `vow version`

- `vow help` / `--help` / `-h` — 使い方を表示
- `vow version` / `--version` / `-V` — バージョンを表示

---

## 終了コード

全サブコマンド共通:

| コード | 意味 |
|---|---|
| `0` | 成功(検査エラーなし / 整形済み) |
| `1` | 診断エラー検出(check)・未整形(`fmt --check`)・構文エラー(fmt) |
| `2` | 使用法エラー(引数不正・ファイル不在) |

シェルでの分岐例:

```bash
vow check src/main.vow && echo "OK" || echo "失敗 (exit $?)"
```

---

## 関連

- 単一 `.vow` の TS 化(`vow build` 実装までの暫定):
  `cargo run -p vow_emit --example transpile -- <input.vow> [output.ts]`
- エージェント向けの取説サーバーは [Vow MCP Server](../README.md#vow-mcp-server)(`vow_check` / `vow_fmt` を JSON-RPC で提供)。
- 構造化 Diagnostic の形式は [`spec/diagnostic-schema.md`](../spec/diagnostic-schema.md)。
