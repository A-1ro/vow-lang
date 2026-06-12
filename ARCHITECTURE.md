# Pact リポジトリ構成 (ARCHITECTURE.md)

> このファイルはリポジトリ構成の **契約** である。
> 新しいディレクトリ・クレートの追加はこのファイルの更新を伴うこと。
> 実装がこの文書と食い違う場合、文書を直すか実装を直すかを明示的に決める。

## 全体構成

```
pact-lang/
├── Cargo.toml                # workspaceルート(membersにcrates/*を列挙)
├── rust-toolchain.toml       # ツールチェイン固定
├── ARCHITECTURE.md           # 本ファイル
├── CLAUDE.md                 # Claude Code向けプロジェクトコンテキスト
│
├── crates/
│   ├── pact_syntax/          # レキサー + パーサ + AST定義
│   ├── pact_check/           # 名前解決 + 型 + エフェクト + 契約検査 / Diagnostic型
│   ├── pact_fmt/             # 正規形フォーマッタ
│   ├── pact_emit/            # TSトランスパイラ + source map
│   ├── pact_cli/             # `pact` バイナリ(check/fmt/build/test)
│   └── pact_mcp/             # MCPサーバーバイナリ
│
├── runtime/                  # @pact/runtime (npmパッケージ, TS実装)
│   ├── package.json
│   └── src/                  # Result/Option/契約アサーション
│
├── spec/                     # 言語仕様(source of truth)
│   ├── pact-spec-v0.1.md
│   ├── diagnostic-schema.md  # Diagnosticスキーマ定義(M0で確定)
│   ├── grammar.md            # 文法リファレンス(pact_specツールの配信元)
│   └── errors/               # エラーコード別解説
│       └── PACT-E3042.md     # 1コード1ファイル
│
├── examples/                 # .pactサンプル集(pact_examplesの配信元)
│   ├── basics/
│   ├── effects/
│   └── contracts/
│
├── tests/
│   ├── golden/               # golden test(契約本文)
│   │   ├── syntax/           # {name}.pact + {name}.expected.json
│   │   ├── fmt/              # {name}.input.pact + {name}.expected.pact
│   │   └── check/            # {name}.pact + {name}.expected.json
│   ├── e2e/                  # トランスパイル→tsc --strict→vitest実行テスト
│   │   ├── stubs/            # import先(core.money / infra.*)のTSスタブ実装
│   │   ├── tests/            # vitestテスト(期待出力・契約違反・source map)
│   │   └── generated/        # pact_emitの出力先(git管理外、e2eテストが再生成)
│   └── mcp/                  # MCPサーバー統合テスト
│
├── docs/                     # ロードマップ・設計メモ
│   └── pact-roadmap-goals.md
│
└── .github/
    └── workflows/ci.yml      # fmt / clippy / test
```

## クレート責務と依存規則

依存は一方向のみ。逆流・循環はコンパイルエラー以前にレビューで弾く。

```
pact_syntax ←─ pact_fmt
     ↑
pact_check  ←─ pact_emit
     ↑              ↑
     └── pact_cli ──┘
     └── pact_mcp ──┘
```

| クレート | 責務 | してはいけないこと |
|---|---|---|
| pact_syntax | ソース→AST。span情報保持。エラー回復 | 型の知識を持つこと |
| pact_check | 意味検査全般。**Diagnostic型の定義元** | 出力形式(散文/JSON)の整形 |
| pact_fmt | AST→正規形テキスト | ASTの意味的変更 |
| pact_emit | 検査済みAST→TS+source map | 検査の再実装 |
| pact_cli | 引数解釈・ファイルIO・Diagnosticの散文整形 | 言語処理ロジック |
| pact_mcp | MCPプロトコル・spec/とexamples/の埋め込み配信 | 言語処理ロジック |

## 設計上の不変条件

1. **Diagnosticはpact_checkが唯一の定義元。** 全クレートはこれを再利用し、独自エラー型を外部に漏らさない(内部エラー→Diagnostic変換は各クレートの境界で行う)。
2. **spec/ と examples/ はpact_mcpにビルド時埋め込み**(include_dir等)。仕様の更新=取説サーバーの更新。
3. **tests/golden/ が契約本文。** golden testの追加・変更は人間レビュー必須。実装都合でexpectedを書き換えるのは契約の一方的変更にあたる。
4. **runtime/ はRustワークスペース外。** npm独立パッケージとして管理し、pact_emitの出力が参照する。
5. CLAUDE.md には本ファイルと spec/ への参照を必ず含め、/goal実行時の前提コンテキストとする。