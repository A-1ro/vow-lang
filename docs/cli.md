# `kei` CLI

`kei` は Kei ツールチェインのコマンドラインフロントエンド(クレート [`kei_cli`](../crates/kei_cli))。
言語処理ロジックは持たず、引数解釈・ファイル IO・Diagnostic の散文整形だけを担い、
検査・整形・トランスパイルは `kei_check` / `kei_fmt` / `kei_syntax` / `kei_emit` に委譲する。

> **ステータス: check / fmt / build / test 実装済み(M6・M7)。** `kei test` は dev ビルド後に
> プロジェクトの `npm test` を起動する薄いラッパーで、Node.js が必要(check / fmt / build 自体は不要)。

---

## インストール

用途に応じて4通り。Rust を持っていなければ 1、持っていれば 2 が手軽。

### 1. ビルド済みバイナリ(Rust 不要・推奨)

GitHub Releases に上がっている各 OS / アーキテクチャ向けバイナリを取得する。
`install.sh` がプラットフォームを判定し、最新リリースから `~/.local/bin/kei` に置く:

```bash
curl -fsSL https://raw.githubusercontent.com/A-1ro/kei-lang/main/install.sh | sh
```

- 対応: macOS(Apple Silicon / Intel)・Linux(x86_64 / aarch64)。Windows は 2 を使う。
- `~/.local/bin` が `PATH` になければスクリプトが追加方法を案内する。
- 環境変数で上書き可能:
  - `KEI_VERSION=v0.1.0` — 入れるタグを固定(既定は最新リリース)
  - `KEI_INSTALL_DIR=/usr/local/bin` — インストール先を変更
- 手動で入れたい場合は [Releases](https://github.com/A-1ro/kei-lang/releases) から
  `kei-<target>.tar.gz`(Windows は `.zip`)を落として展開し、`kei` を PATH 上に置く。

### 2. cargo install(Rust ユーザー)

要件: stable Rust(`rust-toolchain.toml` で固定)。CLI 自体に Node.js は不要。
`~/.cargo/bin/kei` に入る(`[[bin]] name = "kei"`)。`~/.cargo/bin` が `PATH` にあれば、
以降はどこでも `kei check ...` / `kei build ...` で使える。

```bash
# クローン不要。GitHub から直接(任意のタグは --tag v0.1.0、ブランチは --branch main)
cargo install --git https://github.com/A-1ro/kei-lang.git kei_cli

# クローン済みなら、リポジトリのルートで
cargo install --path crates/kei_cli
```

アンインストールはどちらも `cargo uninstall kei_cli`。

### 3. インストールせず使う(開発中)

ビルドし直しが効くので、リポジトリ内での開発にはこちらが手軽:

```bash
cargo run -p kei_cli --bin kei -- check <file>
```

`--` より後ろが `kei` に渡る引数。

### 4. リリースバイナリを直接ビルド

`cargo install` を使わず成果物だけ欲しい場合:

```bash
cargo build --release -p kei_cli
# 生成物: target/release/kei  → PATH 上の任意のディレクトリにコピー
```

---

## サブコマンド

### `kei check <file> [--json]` — 意味検査

名前解決・型・エフェクト・契約を検査する。

| 形式 | 出力 |
|---|---|
| `kei check a.kei` | 既定。散文 Diagnostic(エラーなしなら無出力) |
| `kei check a.kei --json` | 構造化 `Diagnostic[]`(エラーなしなら `[]`) |

```bash
# エラーなし → 何も出さず exit 0
kei check examples/basics/options.kei

# 機械処理向け(span / code / fix 候補入りの JSON)
kei check examples/basics/options.kei --json   # → []
```

`--json` 出力の各要素は `spec/diagnostic-schema.md` の構造化 Diagnostic(span・code・最低 1 つの fix 候補を含む)。

### `kei fmt <file> [--check | --write]` — 正規形整形

`--check` と `--write` は排他。

| 形式 | 動作 |
|---|---|
| `kei fmt a.kei` | 既定。整形結果を **stdout** に出力(非破壊) |
| `kei fmt a.kei --check` | 整形済みか検証。未整形なら差分を出して **exit 1** |
| `kei fmt a.kei --write` | ファイルを整形結果で**上書き** |

```bash
kei fmt examples/basics/options.kei            # 整形結果を表示
kei fmt examples/basics/options.kei --check    # CI 向け(整形済みなら exit 0)
kei fmt examples/basics/options.kei --write    # その場で正規形に
```

構文エラーがあると整形せず Diagnostic を出して exit 1。

### `kei build <dir> [--out-dir <dir>] [--no-source-map]` — ディレクトリ単位のトランスパイル

`<dir>` 配下の `**/*.kei` を再帰収集して検査し、**全件クリーンなら** TS + source map を
`kei_emit` の `ts_path`(モジュール宣言由来。`module a.b` → `a/b.ts`)で出力先に 1:1 配置する。

| オプション | 動作 |
|---|---|
| (既定) | 出力先は `<dir>/dist/`。source map も書く |
| `--out-dir <dir>` | 出力先を変更 |
| `--no-source-map` | `.ts.map` を書かず、`//# sourceMappingURL=` 行も落とす |

- **all-or-nothing**: 1 ファイルでも検査エラーがあれば**何も書かず**、全 Diagnostic を
  stderr に出して exit 1(中途半端な `dist/` を残さない)。
- 生成物はファイルに書くため stdout は使わない。進捗(`wrote N module(s) ...`)も診断も stderr。
- 入力ファイルのパスは `<dir>` 相対で記録するため、出力(先頭コメント・source map の `sources`)は
  プロジェクトの配置に依存しない。

```bash
kei build examples                      # → examples/dist/ に TS + map
kei build src --out-dir build/ts        # 出力先を指定
kei build src --no-source-map           # map なし
```

### `kei test [<dir>]` — dev ビルド + テスト実行

`<dir>`(省略時はカレント)を **dev ビルド(契約 on)**して `<dir>/dist/` に出力し、続いて
プロジェクトの `npm test`(`package.json` の `test` スクリプト)に委譲する。テストランナー
(vitest など)・依存解決はプロジェクト側の責務で、`kei` はランナーの知識を持たない。

- 要件: `<dir>` に `package.json` があり、依存(`@kei/runtime` 等)がインストール済みであること。
- dev ビルドでは `requires` / `ensures` が TS に焼き込まれるため、契約違反は実行時に
  `KeiContractViolation`(`@kei/runtime`)として送出される。捕捉しないテストは失敗し、
  `npm test` の非ゼロ終了が `kei test` にそのまま伝播する。
- 子プロセスの stdout / stderr と環境変数は継承する。

```bash
kei test                 # カレントを dev ビルドして npm test
kei test examples/app    # ディレクトリ指定
```

### `kei help` / `kei version`

- `kei help` / `--help` / `-h` — 使い方を表示
- `kei version` / `--version` / `-V` — バージョンを表示

---

## 終了コード

全サブコマンド共通:

| コード | 意味 |
|---|---|
| `0` | 成功(検査エラーなし / 整形済み / ビルド成功 / テスト全件パス) |
| `1` | 診断エラー検出(check / build)・未整形(`fmt --check`)・構文エラー(fmt)・テスト失敗(test) |
| `2` | 使用法エラー(引数不正・ファイル/ディレクトリ不在) |

シェルでの分岐例:

```bash
kei check src/main.kei && echo "OK" || echo "失敗 (exit $?)"
```

---

## リリースの発行(メンテナ向け)

ビルド済みバイナリ(インストール方法 1)は `v*` タグの push で自動公開される。
`.github/workflows/release.yml` が各 OS / アーキテクチャ向けに `kei` をビルドし、
同名タグの GitHub Release に `kei-<target>.tar.gz`(Windows は `.zip`)を添付する。

```bash
# 例: v0.1.0 を公開する
cargo test --workspace                 # 緑であること
git tag v0.1.0
git push origin v0.1.0                  # → release ワークフローが起動
```

- 配布対象: `x86_64` / `aarch64` × Linux・macOS、および `x86_64` Windows。
- アセット名は `install.sh` が参照するため変更しない(target トリプル命名)。
- Release 本体はワークフローが `--generate-notes` で自動作成する。

---

## 関連

- 単一 `.kei` の TS 化(デバッグ用の最小トランスパイラ):
  `cargo run -p kei_emit --example transpile -- <input.kei> [output.ts]`(ディレクトリ単位は `kei build`)。
- エージェント向けの取説サーバーは [Kei MCP Server](../README.md#kei-mcp-server)(`kei_check` / `kei_fmt` を JSON-RPC で提供)。
- 構造化 Diagnostic の形式は [`spec/diagnostic-schema.md`](../spec/diagnostic-schema.md)。
