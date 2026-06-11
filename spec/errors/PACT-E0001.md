# PACT-E0001: unexpected character

字句解析: Pact が認識しない文字がソース中に現れた。

```pact
let a = 1 @ 2   // error: unexpected character '@'
```

## 修正

その文字を削除するか、意図した演算子・区切り文字に置き換える。
Pact v0.1 の演算子は `== != < > <= >= + - * / = -> ! .` のみ。
