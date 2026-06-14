# Changelog

## 0.2.1

- **修正** — 設定 `kei.server.path` の `${workspaceFolder}` が展開されず言語サーバーの起動に失敗していた問題を修正(`extension.js` が変数を自前で解決するようにした)。README / 設定説明どおり `${workspaceFolder}/target/debug/kei-lsp` で起動できる

## 0.2.0

- **言語サーバー(LSP)統合** — `kei-lsp` を起動し、`.kei` のリアルタイム診断(名前解決・型・エフェクト・契約検査)と契約(`uses` / `requires` / `ensures`)の Hover を提供
- 設定 `kei.server.path` / `kei.trace.server`、コマンド `Kei: Restart Language Server` を追加
- 依存 `vscode-languageclient` ^9 を追加し、対応 VS Code を `^1.82.0` に更新

## 0.1.0

初版。

- `.kei` のシンタックスハイライト(TextMate grammar `source.kei`)
- `.kei` ファイルアイコン(言語アイコン方式、bow-kun)
- 言語設定: 行コメント `//`、括弧の自動補完・サラウンド、インデント
