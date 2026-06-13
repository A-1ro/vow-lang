# Diagnostic スキーマ v0.1

> spec/kei-spec-v0.1.md §6.2 の「案」を M0 で確定させたもの。
> **Diagnostic 型の唯一の定義元は `kei_check` クレート**(ARCHITECTURE.md 不変条件 1)。
> 本文書と実装が食い違った場合は本文書を先に直す。

## 設計原則

1. **JSON が正、散文は派生。** コンパイラ診断は本スキーマの JSON 形式が正式な出力であり、
   CLI の人間向け表示はこれを整形した派生物にすぎない。
2. **全 Diagnostic に span・code・最低 1 つの fix 候補を含める**(CLAUDE.md 不変条件 2)。
   機械的修正が不可能な場合でも、修正の方向を示す fix(edits が空)を最低 1 つ返す。
3. 全エラーコードは `spec/errors/{code}.md` の解説ページと 1:1 対応する。

## エラーコード採番ルール

形式: `KEI-E[カテゴリ1桁][連番3桁]`(例: `KEI-E2042`)

| カテゴリ | 範囲 | 担当領域 |
|---|---|---|
| 0 | KEI-E0xxx | 字句・構文(lexer / parser) |
| 1 | KEI-E1xxx | 名前解決(未定義参照・重複定義・import) |
| 2 | KEI-E2xxx | 型検査 |
| 3 | KEI-E3xxx | エフェクト検査(uses 節) |
| 4 | KEI-E4xxx | 契約検査(requires / ensures の純粋性等) |
| 5 | KEI-E5xxx | フォーマッタ・トランスパイラ等ツール側 |

連番は領域内で一意・欠番可・再利用禁止。

## スキーマ定義

### Diagnostic(ルートオブジェクト)

| フィールド | 型 | 必須 | 説明 |
|---|---|---|---|
| `severity` | `"error" \| "warning" \| "info"` | ✓ | 深刻度 |
| `code` | string | ✓ | 採番ルール準拠のエラーコード |
| `message` | string | ✓ | 人間・エージェント双方が読む一文。英語 |
| `span` | Span | ✓ | 問題箇所 |
| `fixes` | Fix[] | ✓(最低 1 要素) | 修正候補。優先度順 |

### Span

| フィールド | 型 | 必須 | 説明 |
|---|---|---|---|
| `file` | string | ✓ | リポジトリルートからの相対パス |
| `start` | Position | ✓ | 開始位置(含む) |
| `end` | Position | ✓ | 終了位置(含まない・排他) |

### Position

| フィールド | 型 | 必須 | 説明 |
|---|---|---|---|
| `line` | u32 | ✓ | 1 始まり |
| `col` | u32 | ✓ | 1 始まり。UTF-8 ではなく Unicode スカラー値単位 |

### Fix

| フィールド | 型 | 必須 | 説明 |
|---|---|---|---|
| `title` | string | ✓ | 修正内容の一文(例: "Add 'Database.Write' to uses clause") |
| `edits` | TextEdit[] | ✓ | 機械適用可能な編集列。空配列 = 方向のみ提示 |

### TextEdit

| フィールド | 型 | 必須 | 説明 |
|---|---|---|---|
| `span` | Span | ✓ | 置換対象範囲 |
| `new_text` | string | ✓ | 置換後テキスト。挿入は start == end の span で表現 |

## JSON 例

```json
{
  "severity": "error",
  "code": "KEI-E3042",
  "message": "Effect 'Database.Write' used but not declared in 'uses' clause",
  "span": {
    "file": "transfer.kei",
    "start": { "line": 12, "col": 3 },
    "end": { "line": 12, "col": 28 }
  },
  "fixes": [
    {
      "title": "Add 'Database.Write' to uses clause",
      "edits": [
        {
          "span": {
            "file": "transfer.kei",
            "start": { "line": 3, "col": 21 },
            "end": { "line": 3, "col": 21 }
          },
          "new_text": ", Database.Write"
        }
      ]
    }
  ]
}
```

## シリアライズ規約

- フィールド名は snake_case(serde デフォルトのまま)。
- enum 値(severity)は小文字文字列。
- 未知フィールドは前方互換のため読み捨て可。出力側は本スキーマ外のフィールドを追加しない。
- `kei check --json` は Diagnostic の配列(`Diagnostic[]`)を出力する。
