# PACT-E1001: undefined name

名前解決: スコープ内に存在しない値名(変数・関数・import 名)を参照した。
パラメータ・`let` 束縛・ローカル関数・import で導入した名前のみ参照できる
(暗黙の import はない — spec §1 曖昧さゼロ)。

```pact
func total(price: Int) -> Int {
  return price + prce   // error: undefined name 'prce'
}
```

## 修正

typo なら fix の提案(`Did you mean 'price'?`)を適用する。外部モジュールの
名前なら `import` で明示的に導入する。
