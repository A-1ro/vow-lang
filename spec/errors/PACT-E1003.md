# PACT-E1003: duplicate definition

名前解決: 同じスコープで同じ名前が二度定義された。モジュールレベルの
item 名(type / record / enum / func)は単一の名前空間を共有する。record の
フィールド・enum のバリアント・関数パラメータ・同一ブロック内の `let` も
それぞれ重複できない。最初の定義のみが有効として扱われる。

```pact
func sum(a: Int, a: Int) -> Int {   // error: duplicate parameter 'a' in 'sum'
  return a
}
```

## 修正

どちらかを改名するか、不要な方を削除する。
