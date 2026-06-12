# PACT-E2002: unknown field

型検査: record に存在しないフィールドへのアクセス、または record リテラルに
定義外のフィールドを書いた。`Result` は `isOk` / `isErr`、`Option` は
`isSome` / `isNone` のみを組み込みメンバーとして持つ。

```pact
record Point {
  x: Int
  y: Int
}

func demo(p: Point) -> Int {
  return p.z   // error: no field 'z' on record 'Point'
}
```

## 修正

typo なら fix の提案を適用する。新しいフィールドが必要なら record 定義に追加する。
