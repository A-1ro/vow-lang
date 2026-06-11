# PACT-E0004: integer literal out of range

字句解析: 整数リテラルが 64 ビット符号付き整数(`i64`)の範囲を超えた。

```pact
let n = 99999999999999999999   // error: out of range
```

## 修正

`9223372036854775807` 以下の値に収める。
