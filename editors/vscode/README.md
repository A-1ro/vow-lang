# Kei Language — VS Code 拡張

[Kei 言語](https://github.com/A-1ro/kei-lang)(`.kei`)のシンタックスハイライト・ファイルアイコン・言語サーバー(診断/契約 Hover)を提供する VS Code 拡張。

> Kei は「AIが書き、人間が承認し、コンパイラが履行を保証する」ことを前提に設計されたプログラミング言語(TypeScript へトランスパイル)。

## 機能

- **言語サーバー(LSP)** — `kei-lsp` を起動し、`kei_check` の検査結果をエディタへ橋渡しする
  - **診断** — `.kei` を開く/編集するたびに名前解決・型・エフェクト・契約検査を走らせ、エラーを波線表示(`textDocument/publishDiagnostics`)
  - **契約 Hover** — 関数名にカーソルを合わせると `uses` / `requires` / `ensures` を含むシグネチャを表示
- **シンタックスハイライト** — TextMate grammar(`source.kei`)
  - キーワード: `module` / `import` / `as` / `type` / `record` / `enum` / `func` / `let` / `if` / `else` / `return` / `tagged`
  - 契約: `uses` / `requires` / `ensures` / `implies`、契約式の `result` / `old(...)`
  - 早期脱出: `else fail`
  - 組み込み型: `Int` / `Bool` / `String` / `Result` / `Option`、コンストラクタ `Some` / `None` / `Ok` / `Err`
  - 文字列(エスケープ `\n \t \r \\ \"`)・整数リテラル・行コメント `//`
- **ファイルアイコン** — `.kei` に bow-kun アイコンを表示
- **言語設定** — 括弧の自動補完・サラウンド、インデント、行コメントのトグル

## 対応バージョン

VS Code `^1.82.0`(`vscode-languageclient` v9 の要件)

## 設定

| 設定 | 既定 | 説明 |
| --- | --- | --- |
| `kei.server.path` | `""` | `kei-lsp` 実行ファイルのパス。空のときは `PATH` 上の `kei-lsp` を使う。リポジトリで開発中なら `${workspaceFolder}/target/debug/kei-lsp` を指定する。 |
| `kei.trace.server` | `off` | VS Code と言語サーバー間の JSON-RPC 通信トレース(`off` / `messages` / `verbose`)。 |

コマンド `Kei: Restart Language Server`(`kei.restartServer`)でサーバーを再起動できる(サーバーを再ビルドした後などに使う)。

## 開発

```sh
# 1. 言語サーバーをビルド(リポジトリルートで)
cargo build -p kei_lsp        # target/debug/kei-lsp ができる

# 2. 拡張をローカルで試す: editors/vscode/ を VS Code で開き F5(拡張開発ホスト)
#    .kei を開くとハイライト + 診断 + 契約 Hover が効く
```

`kei-lsp` が `PATH` に無いときは、設定 `kei.server.path` に `${workspaceFolder}/target/debug/kei-lsp` を指定する。

文法ファイルは `syntaxes/kei.tmLanguage.json`、LSP 配線は `extension.js`。言語語彙・診断の正は `spec/kei-spec-v0.1.md` と `kei_check`。

## ライセンス

MIT
