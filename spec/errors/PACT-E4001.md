# PACT-E4001: effectful call in contract

契約検査: `requires` / `ensures` の契約式の中で、エフェクト(`uses`)を持つ
関数を呼び出した。契約式は副作用禁止・純粋関数の呼び出しのみ可
(spec §4 — 将来の静的証明可能性を壊さないための制約)。

```pact
func currentBalance(id: Int) -> Int
  uses Database.Read
{
  return 100
}

func withdraw(id: Int, amount: Int) -> Int
  requires currentBalance(id) >= amount   // error: call to 'currentBalance' (uses Database.Read) is not allowed in a contract
{
  return amount
}
```

## 修正

エフェクトを伴う取得は本体で行い、結果をパラメータとして受け取って契約式では
純粋な値だけを参照する。呼び出し先が実際には純粋なら `uses` を外す。
