# PACT-E2004: malformed record literal

型検査: record リテラルの形が定義と一致しない。必須フィールドの欠落、
リテラル内のフィールド重複、record 型でないものへのリテラル適用
(関数呼び出し形での record 構築を含む)がこのエラーになる。

```pact
record Point {
  x: Int
  y: Int
}

func demo() -> Point {
  return Point { x: 1 }   // error: missing field(s) 'y' in 'Point' literal
}
```

## 修正

欠落フィールドを追加し、重複を取り除く。record は必ず `Name { ... }` 形で構築する。
