# Kei リポジトリ構成 (ARCHITECTURE.md)

> このファイルはリポジトリ構成の **契約** である。
> 新しいディレクトリ・クレートの追加はこのファイルの更新を伴うこと。
> 実装がこの文書と食い違う場合、文書を直すか実装を直すかを明示的に決める。

## 全体構成

```
kei-lang/
├── Cargo.toml                # workspaceルート(membersにcrates/*を列挙)
├── rust-toolchain.toml       # ツールチェイン固定
├── ARCHITECTURE.md           # 本ファイル
├── CLAUDE.md                 # Claude Code向けプロジェクトコンテキスト
├── HANDOFF.md                # 設計判断の経緯(なぜ今こうなっているか)
├── install.sh                # `kei` CLIのビルド済みバイナリ導入スクリプト(curl|sh)
│
├── crates/
│   ├── kei_syntax/          # レキサー + パーサ + AST定義
│   ├── kei_check/           # 名前解決 + 型 + エフェクト + 契約検査 / Diagnostic型
│   ├── kei_fmt/             # 正規形フォーマッタ
│   ├── kei_emit/            # TSトランスパイラ + source map
│   ├── kei_cli/             # `kei` バイナリ(check/fmt/build/test/mcp)
│   ├── kei_mcp/             # MCPサーバー(lib: run_stdio / Server)+ kei-mcp バイナリ
│   └── kei_lsp/             # LSPサーバーバイナリ(kei-lsp)
│
├── runtime/                  # @kei/runtime (npmパッケージ, TS実装)
│   ├── package.json
│   └── src/                  # Result/Option/契約アサーション
│
├── spec/                     # 言語仕様(source of truth)
│   ├── kei-spec-v0.1.md
│   ├── kei-spec-v0.2.md      # v0.2差分章(match / extern / 検証レベル / 数量契約イディオム, M10–M13)
│   ├── kei-spec-v0.3-collections.md  # コレクション型(立場B / List 段階導入, Draft, #25)
│   ├── diagnostic-schema.md  # Diagnosticスキーマ定義(M0で確定 + v0.2 CheckReport拡張)
│   ├── grammar.md            # 文法リファレンス(kei_specツールの配信元)
│   └── errors/               # エラーコード別解説
│       └── KEI-E3042.md     # 1コード1ファイル
│
├── examples/                 # .keiサンプル集(kei_examplesの配信元)
│   ├── basics/
│   ├── effects/
│   └── contracts/
│
├── tests/
│   ├── golden/               # golden test(契約本文)
│   │   ├── syntax/           # {name}.kei + {name}.expected.json
│   │   ├── fmt/              # {name}.input.kei + {name}.expected.kei
│   │   └── check/            # {name}.kei + {name}.expected.json
│   ├── e2e/                  # トランスパイル→tsc --strict→vitest実行テスト
│   │   ├── stubs/            # import先(core.money / infra.*)のTSスタブ実装
│   │   ├── tests/            # vitestテスト(期待出力・契約違反・source map)
│   │   └── generated/        # kei_emitの出力先(git管理外、e2eテストが再生成)
│   ├── mcp/                  # MCPサーバー統合テスト
│   └── cli/                  # `kei` CLI統合テスト(実バイナリ起動でstdout/stderr/終了コード検証)
│       ├── checks/           # {name}.kei + {name}.check.txt(散文) + {name}.check.json
│       ├── fmt/              # {name}.input.kei (+ {name}.expected.kei / {name}.fmtcheck.txt / {name}.fmt.txt)
│       └── projects/         # `kei build`/`kei test`のプロジェクトfixture
│           ├── app/          # .kei + expected/(buildツリーのgolden) + package.json/tests/(kei test)
│           └── broken/       # 検査エラーを含むall-or-nothing検証用(dist は git管理外)
│
├── docs/                     # ロードマップ・設計メモ
│   ├── kei-roadmap-goals.md          # v0.1(M0–M7)+ 提案中の M8(kei_lsp)
│   ├── kei-roadmap-v0.2.md           # v0.2(M10–M13: 健全性・契約表現力)
│   ├── kei-roadmap-v0.3.md           # v0.3(M9 + M14–M18: 射程拡張・契約検証の本丸)
│   └── effect-postconditions-memo.md # エフェクト事後条件の言語拡張比較メモ(#21→#45, M14)
│
├── assets/                    # ブランド資産(README等で参照)
│   ├── bow-kun.png           # マスコット bow-kun(契約書を咥えた柴犬, 1024px 原寸)
│   ├── bow-kun-{16..512}.png # リサイズ版(16/32/48/64/128/256/512)
│   └── bow-kun.ico           # favicon(16/32/48 同梱)
│
└── .github/
    ├── dependabot.yml      # 依存自動更新(cargo / github-actions / npm)
    └── workflows/
        ├── ci.yml           # fmt / clippy / test
        └── release.yml      # v*タグでkeiバイナリをビルドしGitHub Releasesへ添付
```

## クレート責務と依存規則

依存は一方向のみ。逆流・循環はコンパイルエラー以前にレビューで弾く。

```
kei_syntax ←─ kei_fmt
     ↑
kei_check  ←─ kei_emit
     ↑              ↑
     ├── kei_cli ──┘   (→ kei_mcp も: `kei mcp` でサーバー起動を委譲)
     ├── kei_mcp
     └── kei_lsp   (→ kei_check / kei_syntax / kei_fmt)
```

`kei_cli → kei_mcp` は一方向の辺で循環しない(kei_mcp は kei_cli を知らない)。`kei mcp` は
`kei_mcp::run_stdio` を呼ぶだけで、MCP プロトコル処理は kei_mcp が一手に担う(配布物を `kei`
1 バイナリに統合するための辺。経緯は HANDOFF.md)。

| クレート | 責務 | してはいけないこと |
|---|---|---|
| kei_syntax | ソース→AST。span情報保持。エラー回復 | 型の知識を持つこと |
| kei_check | 意味検査全般。**Diagnostic型の定義元** | 出力形式(散文/JSON)の整形 |
| kei_fmt | AST→正規形テキスト | ASTの意味的変更 |
| kei_emit | 検査済みAST→TS+source map | 検査の再実装 |
| kei_cli | 引数解釈・ファイルIO・Diagnosticの散文整形・ディレクトリ走査・テストランナー起動・`kei mcp`でのMCPサーバー起動 | 言語処理ロジック(検査/整形/トランスパイルは委譲)。`kei test`はランナーの知識を持たず`npm test`へ委譲。`kei mcp`はプロトコル処理を持たず`kei_mcp::run_stdio`へ委譲 |
| kei_mcp | MCPプロトコル・spec/とexamples/の埋め込み配信。stdio起動は`run_stdio`が単一エントリ(`kei-mcp`バイナリと`kei mcp`が共有) | 言語処理ロジック |
| kei_lsp | LSPプロトコル変換。kei_checkのDiagnostic→LSP Diagnostic、AST→Hover(契約表示)へ写す。同期stdioループ | 言語処理ロジック(検査/整形/パースは委譲)。kei_check/kei_syntax/kei_fmtへ一方向依存のみ |

### 外部依存の追加記録(M6 事前合意の手続き)

- `kei_cli`: `serde_json`(`--json` の `Diagnostic[]` 直列化に使用。構造化出力は serde が正)。
  CLI 統合テスト(`tests/cli.rs`)は std の `std::process::Command` で実バイナリを起動するため
  追加クレートは要らず、`serde_json` を dev-dependency にも置いて `--json` 出力を構造比較する。
  引数解釈は手書き(clap 等は不採用)、整形検証は `CARGO_TARGET_TMPDIR` の一時ファイルで行う。
- `kei build` / `kei test`(M7): 新規依存なし。`build` は `kei_emit::emit_module` をディレクトリ走査で
  回し、`ts_path` 通りに書き出す(all-or-nothing)。`test` は `<dir>` を dev ビルドして `std::process::Command`
  で `npm test` を起動するだけ(Node はプロジェクト側の前提。CI の test ジョブと同じ)。
  `kei build` の出力ツリーは `tests/cli/projects/*/expected/` で golden 比較、`kei test` の契約 on 実行
  (`requires` 違反 → `KeiContractViolation` → 非ゼロ終了)は Node 在席時のみ走る統合テストで検証する。
- `kei mcp`: `kei_cli` に `kei_mcp`(path 依存)を追加。MCP サーバーの起動を `kei_mcp::run_stdio`
  へ委譲するだけで、新規の外部クレートは増えない。これにより配布物が `kei` 1 バイナリで完結し
  (`install.sh` / `cargo install kei_cli`)、利用者は `kei mcp` で取説サーバーを起動できる。
  `kei-mcp` バイナリは開発・後方互換のため残す(両者は同じ `run_stdio` を共有)。
- `kei_lsp`(M8): `lsp-server`(同期 stdio JSON-RPC スキャフォルド)+ `lsp-types`(LSP 型)+ `serde` /
  `serde_json`。tower-lsp(tokio/async)は採用しない — 本サーバーの実処理は同期関数
  `kei_check::check_module` の呼び出しだけで、非同期ランタイムを持ち込む理由がない(kei_mcp が
  serde_json だけで JSON-RPC を手回ししているのと同じ「最小依存・薄いアダプタ」方針)。lsp-server は
  rust-analyzer と同系統の同期スキャフォルドで、kei_mcp の `Server::handle` と同じく
  「リクエスト Value → レスポンス」のディスパッチに落ちる。言語処理は一切再実装せず、Diagnostic と
  AST を LSP に翻訳する境界に徹する。

## 設計上の不変条件

1. **Diagnosticはkei_checkが唯一の定義元。** 全クレートはこれを再利用し、独自エラー型を外部に漏らさない(内部エラー→Diagnostic変換は各クレートの境界で行う)。
2. **spec/ と examples/ はkei_mcpにビルド時埋め込み**(include_dir等)。仕様の更新=取説サーバーの更新。
3. **tests/golden/ が契約本文。** golden testの追加・変更は人間レビュー必須。実装都合でexpectedを書き換えるのは契約の一方的変更にあたる。
4. **runtime/ はRustワークスペース外。** npm独立パッケージとして管理し、kei_emitの出力が参照する。
5. CLAUDE.md には本ファイルと spec/ への参照を必ず含め、/goal実行時の前提コンテキストとする。