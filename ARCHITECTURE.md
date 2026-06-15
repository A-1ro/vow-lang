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
├── install.sh                # `kei` CLIのビルド済みバイナリ導入スクリプト(curl|sh)
│
├── crates/
│   ├── kei_syntax/          # レキサー + パーサ + AST定義
│   ├── kei_check/           # 名前解決 + 型 + エフェクト + 契約検査 / Diagnostic型
│   ├── kei_fmt/             # 正規形フォーマッタ
│   ├── kei_emit/            # TSトランスパイラ + source map
│   ├── kei_cli/             # `kei` バイナリ(check/fmt/build/test)
│   ├── kei_mcp/             # MCPサーバーバイナリ
│   └── kei_lsp/             # LSPサーバーバイナリ(kei-lsp)
│
├── runtime/                  # @kei/runtime (npmパッケージ, TS実装)
│   ├── package.json
│   └── src/                  # Result/Option/契約アサーション
│
├── spec/                     # 言語仕様(source of truth)
│   ├── kei-spec-v0.1.md
│   ├── kei-spec-v0.3-collections.md  # コレクション型(立場B / List 段階導入, Draft, #25)
│   ├── diagnostic-schema.md  # Diagnosticスキーマ定義(M0で確定)
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
│   └── kei-roadmap-goals.md
│
├── assets/                    # ブランド資産(README等で参照)
│   ├── bow-kun.png           # マスコット bow-kun(契約書を咥えた柴犬, 1024px 原寸)
│   ├── bow-kun-{16..512}.png # リサイズ版(16/32/48/64/128/256/512)
│   └── bow-kun.ico           # favicon(16/32/48 同梱)
│
└── .github/
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
     ├── kei_cli ──┘
     ├── kei_mcp
     └── kei_lsp   (→ kei_check / kei_syntax / kei_fmt)
```

| クレート | 責務 | してはいけないこと |
|---|---|---|
| kei_syntax | ソース→AST。span情報保持。エラー回復 | 型の知識を持つこと |
| kei_check | 意味検査全般。**Diagnostic型の定義元** | 出力形式(散文/JSON)の整形 |
| kei_fmt | AST→正規形テキスト | ASTの意味的変更 |
| kei_emit | 検査済みAST→TS+source map | 検査の再実装 |
| kei_cli | 引数解釈・ファイルIO・Diagnosticの散文整形・ディレクトリ走査・テストランナー起動 | 言語処理ロジック(検査/整形/トランスパイルは委譲)。`kei test`はランナーの知識を持たず`npm test`へ委譲 |
| kei_mcp | MCPプロトコル・spec/とexamples/の埋め込み配信 | 言語処理ロジック |
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