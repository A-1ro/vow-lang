# PACT-E2005: tagged type confusion

型検査: tagged 型(幽霊型タグ付きエイリアス — spec §2.2)とその基底型、
または異なる tagged 型同士を混同した。`type AccountId = String tagged "AccountId"`
の `AccountId` は `String` と構造が同じでも型として互換にならない。
これが tagged 型の存在意義であり、混同は常にエラー。

```pact
type AccountId = String tagged "AccountId"

func exists(id: AccountId) -> Bool {
  return true
}

func demo(raw: String) -> Bool {
  return exists(raw)   // error: expected 'AccountId', found 'String'
}
```

## 修正

期待される tagged 型の値を明示的に構築して渡すか、両辺の型を揃える。
