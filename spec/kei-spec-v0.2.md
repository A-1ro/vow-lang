# Kei 言語仕様書 v0.2 — 健全性と契約表現力(Draft)

> v0.1(`kei-spec-v0.1.md`)への差分章。v0.2 のテーマは
> **健全性(soundness)と契約表現力の強化**。出典はドッグフード実験と外部レビュー
> (issues #20–#23)。v0.1 と矛盾する箇所は本章を新しい正とする。
> ロードマップは `docs/kei-roadmap-v0.2.md`(M10–M13)。

## 1. `match` 式 — 網羅分解(M10 / #20)

`Option<T>` / `Result<T, E>` / ユーザー enum の中身を、**純粋文脈でも**取り出すための
式。v0.1 では Option の中身を開く手段が `else fail`(= Result 文脈専用)しか無く、
Result を返さない純粋関数の内部で Option を開けなかった。`match` はこの穴を塞ぐ。

### 1.1 構文

```text
match <スクルティニ式> {
  <パターン> => <腕の式>
  <パターン> => <腕の式>
  ...
}
```

- `match` は **式**。各腕の式の型が一致し、その型が `match` 式全体の型になる。
- 腕の区切りは **改行**(正規形)。カンマ区切りも受理するが `kei fmt` は改行に正規化する。
- 腕の本体は **式**(文ブロックではない)。`let r = match x { ... }` のように使う。
- スクルティニは `if` 条件と同じく record リテラル禁止文脈(`match foo { ... }` の
  `foo {` をリテラルと解釈しない)。

### 1.2 パターン(1 段)

| 対象 | パターン |
|---|---|
| `Option<T>` | `Some(x)`(`x: T` を束縛) / `None` |
| `Result<T, E>` | `Ok(x)`(`x: T`) / `Err(e)`(`e: E`) |
| enum unit バリアント | `Enum.V` |
| enum 位置ペイロード | `Enum.V(a, b)`(各値を束縛) |
| enum 名前付きフィールド | `Enum.V { a, b }`(**全フィールド**を同名で束縛) |

- enum パターンは構築形と対称に **`Enum.Variant`** の 2 段形で書く(`Color.Red`)。
- 名前付きフィールドパターンは全フィールドの列挙を要求する(暗黙の無視なし)。
- ネストパターン(`Some(Ok(x))`)・ガード(`Some(x) if ...`)・ワイルドカード `_` は
  v0.2 では**入れない**(段階導入)。
- 束縛変数は**その腕の中だけ**で有効(腕をまたいで参照すると `KEI-E1001`)。

### 1.3 網羅性

コンパイラは全バリアントの被覆を強制する。**ワイルドカード `_` は無い**ので、
網羅漏れを隠せない。これは「暗黙なし」(§1 第二条)と整合し、enum にバリアントを
足したとき既存の `match` が必ずコンパイルエラーになる(追従漏れの防止)。

| コード | 条件 |
|---|---|
| `KEI-E2007` | 網羅性違反(不足バリアントを列挙) |
| `KEI-E2008` | 到達不能腕(同じコンストラクタの重複) |
| `KEI-E2009` | パターン不適合(型と異なるコンストラクタ族・存在しないバリアント・束縛形/個数違い) |
| `KEI-E2001` | 腕の式の型不一致 |

import 由来などスクルティニの型が解決できない場合(`_` 相当の opaque)、網羅性検査は
行わない(寛容)。腕の式は通常どおり型検査される。

### 1.4 純粋文脈で Option を開く(#20 の本命)

`isOverdue` のように本来 `Option` で表したい API が、v0.1 では言語都合で `Result` に
歪められていた。`match` でこれが自然に書ける:

```kei
func isOverdue(daysLeft: Option<Int>) -> Option<Bool> {
  return match daysLeft {
    Some(d) => Some(d < 0)
    None => None()
  }
}
```

### 1.5 トランスパイル

`match` は即時実行アロー関数(IIFE)に展開する。各腕は判別子で分岐する `if` ガードに
落ちる(`Option`/`Result` は内部判別子 `.ok`、enum は `.kind`)。束縛は腕の冒頭で
`const`。網羅性をチェッカが保証するため末尾の `throw` は到達不能(opaque な import 値の
防御)。

| Kei | TypeScript |
|---|---|
| `match` 式 | `(() => { const m = <scrut>; if (<判別>) { <束縛> return <腕> } ... })()` |
| `Some(x)` / `Ok(x)` 腕 | `if (m.ok) { const x = m.value; ... }` |
| `Err(e)` 腕 | `if (!m.ok) { const e = m.error; ... }` |
| `None` 腕 | `if (!m.ok) { ... }` |
| `Enum.V(a, b)` 腕 | `if (m.kind === "V") { const a = m.values[0]; const b = m.values[1]; ... }` |
| `Enum.V { f }` 腕 | `if (m.kind === "V") { const f = m.fields.f; ... }` |

## 2. `extern` 署名 — 外部境界の検証(M11 / #22)

v0.1 では `import infra.database as Database` 配下の呼び出し(`Database.fetchBalance(...)`
/ `Time.now()`)が **opaque** で、戻り型もエフェクトも検査対象外だった。`Time.now()` を
呼びながら `uses Clock` を宣言し忘れても検出されない——「曖昧さゼロ・暗黙なし」を掲げる
言語の**境界部分でだけ合意書の担保が外れていた**。`extern` 署名はこの穴を塞ぐ。

### 2.1 構文

```kei
import infra.time as Time
import infra.database as Database

extern Time.now() -> Int uses Clock
extern Database.fetchBalance(account: AccountId) -> Option<Money> uses Database.Read
extern Database.setBalance(account: AccountId, balance: Money) uses Database.Write
extern Audit.Log.record(entry: TransferReceipt) uses Audit.Log
```

- `extern <名前空間パス>(<パラメータ>) [-> <戻り型>] [uses <エフェクト...>]`。
- モジュール先頭の宣言群(import の後)に置く。本体は持たない。
- パスは import した名前空間配下のメンバー(`Time.now` / `Audit.Log.record`)。
- 戻り型・`uses` は省略可(省略時はそれぞれ Unit / エフェクトなし)。
- `extern` は **検査専用**。TS には何も出力しない(呼び出し側は従来どおり対応する
  TS 呼び出し/import に素直に写る)。

### 2.2 意味論

`extern` 署名が宣言された外部呼び出しは、もはや opaque ではない:

1. **戻り型が型検査に伝播する。** `Database.fetchBalance(account)` は `Option<Money>` を返す
   値として扱われ、`match` で分解したり `else fail` で開いたりできる。型を取り違えると
   `KEI-E2001`。
2. **エフェクトが呼び出し元の `uses` へ推移伝播する。** ローカル関数呼び出しと同じ規則。
   宣言漏れは**境界越しで `KEI-E3001`** として落ちる(#22 の「`uses Clock` 書き忘れ」が
   正しくエラーになる)。
3. **引数の個数・型を照合する**(`KEI-E2001`)。
4. `uses` に書けるのは標準エフェクト階層のノードのみ(`KEI-E3002`)。同じ外部パスへの
   重複宣言は `KEI-E3003`。

### 2.3 移行戦略(段階移行 / gradual)

v0.2 は **opt-in**。`extern` を宣言した呼び出しだけが照合される。署名の無い外部呼び出しは
**従来どおり opaque**(check を通る)で、既存コードを壊さない。これは #22 の事前合意の
(a)「段階移行」に対応する第一段階で、境界を 1 つずつ合意書に載せていける。

> **未宣言呼び出しの扱いの将来:** 「extern 未宣言の外部呼び出し」を warning/error として
> 検出する厳格モードは v0.3+ の段階で導入する(v0.1 の `ok_*` 検査群が外部呼び出しを
> 別目的で多用しており、一括 flag-day 移行は時期尚早。HANDOFF の「スコープ発散が死因」と
> #22 の P3 位置づけに従い、まず enforcement-when-declared を確立する)。
