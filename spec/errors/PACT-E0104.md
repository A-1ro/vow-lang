# PACT-E0104: unknown contract clause

構文解析: 関数シグネチャの契約節位置に `uses` / `requires` / `ensures`
以外の語が現れた。既知の節キーワードへの編集距離が 2 以下の場合に
typo とみなして報告し、置換 fix を提示する。

```pact
func save(user: User) -> Bool
  use Database.Write   // error: unknown clause 'use'(fix: 'uses' へ置換)
{
```

## 修正

正しい節キーワード(`uses` / `requires` / `ensures`)に置き換える。
