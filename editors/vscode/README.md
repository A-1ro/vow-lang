# Kei Language — VS Code 拡張

[Kei 言語](https://github.com/A-1ro/kei-lang)(`.kei`)のシンタックスハイライトとファイルアイコンを提供する VS Code 拡張。

> Kei は「AIが書き、人間が承認し、コンパイラが履行を保証する」ことを前提に設計されたプログラミング言語(TypeScript へトランスパイル)。

## 機能

- **シンタックスハイライト** — TextMate grammar(`source.kei`)
  - キーワード: `module` / `import` / `as` / `type` / `record` / `enum` / `func` / `let` / `if` / `else` / `return` / `tagged`
  - 契約: `uses` / `requires` / `ensures` / `implies`、契約式の `result` / `old(...)`
  - 早期脱出: `else fail`
  - 組み込み型: `Int` / `Bool` / `String` / `Result` / `Option`、コンストラクタ `Some` / `None` / `Ok` / `Err`
  - 文字列(エスケープ `\n \t \r \\ \"`)・整数リテラル・行コメント `//`
- **ファイルアイコン** — `.kei` に bow-kun アイコンを表示
- **言語設定** — 括弧の自動補完・サラウンド、インデント、行コメントのトグル

## 対応バージョン

VS Code `^1.70.0`

## 開発

```sh
# 拡張をローカルで試す: editors/vscode/ を VS Code で開き F5(拡張開発ホスト)
# .kei ファイルを開くとハイライトが効く
```

文法ファイルは `syntaxes/kei.tmLanguage.json`。言語語彙の正は `spec/kei-spec-v0.1.md`。

## ライセンス

MIT
