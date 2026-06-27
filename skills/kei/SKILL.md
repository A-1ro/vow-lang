---
name: kei
description: Kei 言語のコードを学習なしで正しく書くための取扱説明書。func/契約/エフェクト/型/失敗処理の最小十分セットと頻出エラーの直し方を一括ロードする。Kei の .kei ファイルを生成・編集するときに読む。
---

# Kei を書く

Kei は「AIが書き、人間が承認し、コンパイラが履行を保証する」前提で設計された言語。TypeScript にトランスパイルされる(V8 / Cloudflare Workers / Node)。設計は **検証可能性・推論の局所性・エージェントループとの親和性** に全振りし、人間の書きやすさは捨てている。`null` も例外も無い。失敗は型(`Option` / `Result`)に現れる。副作用は `uses`(ケーパビリティ)で宣言する。

この文書だけ読めば、文法を問い合わせずに実務コードが書ける。網羅は `spec/kei-spec-v0.1.md` に任せ、ここは頻出経路を厚くしてある。矛盾したら **spec が正**。

## 最重要: 書いたら必ず検証する

書いた `.kei` は必ず `kei check` に通し、エラーゼロになるまで直す。**エラーゼロが完成の定義**。

```text
kei check <file> --json     # Diagnostic[] (JSON) を得る → fixes を読んで直す → 再実行
```

- `--json` で構造化 Diagnostic(`severity` / `code` / `message` / `span` / `fixes`)が出る。`fixes[].edits` があれば優先して機械適用する。詳しくは §9。
- このループを回す前に推測で「直ったはず」と判断しない。`kei check` の出力だけが真。

### この文書のコードブロック規約

- **` ```kei `** … そのままで `kei check` を通る完結したプログラム(コピーの出発点)。
- **` ```text `** … 断片、または **意図的に誤った例**(✗ で示す)。`kei check` は通らない。コピー元にしない。

---

## 1. 最小テンプレート

これをコピーして改変する。module + func + 契約(`requires` / `ensures` / `old` / `result`)の最小完全形(`examples/contracts/counter.kei`)。

```kei
module contracts.counter

func increment(count: Int, step: Int) -> Int
  requires step > 0
  ensures result == old(count) + step
{
  return count + step
}
```

エフェクトと失敗(`Result` / `import` / `else fail`)まで入った実務形(`examples/contracts/withdraw.kei`)。

```kei
module contracts.withdraw

import core.money { AccountId, Money }
import infra.database as Database

enum WithdrawError {
  NotFound(AccountId)
  Overdraft { limit: Money }
}

func withdraw(account: AccountId, amount: Money) -> Result<Money, WithdrawError>
  uses Database.Read, Database.Write
  requires amount > Money.zero
  ensures result.isOk implies amount > Money.zero
{
  let current = Database.fetchBalance(account) else fail WithdrawError.NotFound(account)
  if current < amount {
    return Err(WithdrawError.Overdraft { limit: current })
  }
  Database.setBalance(account, current - amount)
  return Ok(current - amount)
}
```

---

## 2. 構文リファレンス(要点)

### func と契約節

シグネチャは `func 名(引数: 型, ...) -> 戻り値型` の後に契約節を **この順** で並べ、`{ }` で本体。契約節は省略可。

```text
func 名(p: T, ...) -> R
  uses Effect, ...          // 行使するエフェクト(無ければ純粋関数)
  requires <Bool式>         // 事前条件。複数可。呼び出し側の義務
  ensures <Bool式>          // 事後条件。複数可。実装側の義務。result と old() が使える
{
  ...
}
```

- `result` … 戻り値。**`ensures` でのみ**使える。
- `old(expr)` … 呼び出し前の値。**`ensures` でのみ**使える。
- 条件を「かつ」で並べたいときは `requires` / `ensures` を**複数行に分ける**(後述のとおり `&&` は無い)。

### 型定義(record / enum / tagged)

```kei
module ref.types

type AccountId = String tagged "AccountId"   // 幽霊型タグ。String と混同不可

record Account {
  id: AccountId
  balance: Int
}

enum FetchError {
  Timeout                       // unit バリアント
  NotFound(AccountId)           // 位置ペイロード
  Denied { reason: String }     // 名前付きフィールド
}
```

- 組み込み型は `Int`(i64)・`String`・`Bool`・`Result<T, E>`・`Option<T>` のみ。それ以外は同一ファイルの `record` / `enum` / `type` 宣言か `import` が要る。
- ユーザー定義型は **型引数を取れない**(ジェネリクスは組み込みの `Result`(2)・`Option`(1)・`List`(1)だけ)。
- **コレクション `List<T>` は v0.3 で利用可能。** 要素は不変・opaque。8 コンビネータ — `length`・`isEmpty()`・`get(i)`(→ `Option<T>`)・`map`・`filter`・`fold`・`all`・`any` — で反復・集計・絞り込みを書く。`map`/`filter`/`fold`/`all`/`any` の関数引数は **名前付き純粋関数の参照**(ラムダは無い)。契約では `length`・`isEmpty()`・`all`・`any`・`result.length` を参照できる。`List` リテラル構文はまだ無いので、`List` はパラメータや `map`/`filter` の戻りとして受け取る。配列リテラル・`Map` は未実装。詳細は `spec/kei-spec-v0.3-collections.md`、実例は `examples/collections/inventory.kei`。

```kei
module collections.demo

record Item {
  qty: Int
}

func nonNeg(item: Item) -> Bool {
  return item.qty >= 0
}

func addQty(acc: Int, item: Item) -> Int {
  return acc + item.qty
}

func totalQty(items: List<Item>) -> Int
  requires items.all(nonNeg)
  ensures result >= 0
{
  return items.fold(0, addQty)
}

func firstItem(items: List<Item>) -> Option<Item> {
  return items.get(0)
}
```

### 値の構築

```kei
module ref.build

record Point {
  x: Int
  y: Int
}

enum Shape {
  Dot
  Line { length: Int }
}

func origin() -> Point {
  return Point { x: 0, y: 0 }          // record は必ず Name { ... }
}

func shift(p: Point, dx: Int) -> Point {
  let x = p.x + dx
  let y = p.y
  return Point { x, y }                // 変数名がフィールド名と一致するなら省略形
}

func makeDot() -> Shape {
  return Shape.Dot                     // unit: E.V
}

func makeLine(n: Int) -> Shape {
  return Shape.Line { length: n }      // 名前付き: E.V { ... } / 位置なら E.V(...)
}
```

フィールド省略形(`{ x }`)は record でも enum の名前付きバリアントでも使える。変数名がフィールド名と一致するとき、`Shape.Line { length: length }` は `Shape.Line { length }` と書ける。

### 式と文

```kei
module ref.stmt

record User {
  name: String
  age: Int
}

func classify(u: User) -> String {
  let adult = u.age >= 18           // let 束縛(再代入なし)
  if adult {                        // if 条件は Bool
    return "adult"
  } else if u.age >= 13 {           // else if 連鎖
    return "teen"
  } else {
    return "child"
  }
}
```

- フィールドアクセスは `.`(`u.age`)。関数呼び出しは `f(args)`。名前空間メンバは `Database.fetch(...)` / `Audit.Log.record(...)`。
- コメントは `//` の行コメントのみ。`kei fmt` は leading / trailing / 本体内コメントを保持する(v0.4 / M19)。
- 文字列は1行・ダブルクオート。エスケープは `\n \t \r \\ \"` のみ。

### 演算子(これで全部 / v0.4)

```text
比較   ==  !=  <  >  <=  >=
算術   +  -  *  /  %         // / は 0 方向切り捨て、% は同じ商での剰余
単項   -x  !x                // - は Int、! は Bool
論理   ||  implies           // a implies b は「a ならば b」(= !a || b)
その他 =(let束縛)  ->(戻り型)  .(アクセス)
```

- **`&&` は存在しない。** 「かつ」は `requires` を複数並べるか `if` を入れ子にして表す。`&` を書くと `KEI-E0001 unexpected character`。

### module と import

```kei
module payments.transfer

import core.money { Money, AccountId }    // 名前を明示 import
import infra.database as Database         // 名前空間に別名を付けて import
```

- import は**全て明示**。ワイルドカード・再エクスポート禁止。
- モジュールパスはファイルパスと 1:1(`payments.transfer` ↔ `payments/transfer.kei`)。`module` のパス各セグメントに予約語(`fail` 等)は使えない(`KEI-E0102`)。
- 暗黙の import は無い。使う外部名は必ず import する。
- `as` で別名 import した名前空間配下のメンバ呼び出し(`Database.fetchBalance(...)` 等)は、既定では **opaque** 扱い。`check` は外部モジュールの実体を解決しないので、戻り型を気にせず呼べて check も通る(本体ロジックは自分で正しく組むこと)。
- **境界を検証したいなら `extern` 署名を宣言する(v0.2)。** `extern Time.now() -> Int uses Clock` のように外部関数の戻り型・エフェクトを宣言すると、その呼び出しは opaque でなくなり、戻り型が型検査に伝播し、エフェクトが呼び出し元の `uses` へ推移伝播する(宣言漏れは `KEI-E3001`)。`extern` は import の後・モジュール先頭の宣言群に置く。詳細は §3「外部境界(extern)」と `spec/kei-spec-v0.2.md` §2。

---

## 3. エフェクト(最頻出のつまずき)

エフェクトは **ケーパビリティ**。関数は `uses` に列挙したエフェクトしか行使できない。`uses` の無い関数は **純粋**。

### 推移的伝播(ここを最も間違える)

呼び出し先が使うエフェクトは、**呼び出し元も `uses` に宣言しなければならない**。下に降りていくのではなく、上に伝播する。

```kei
module fx.propagate

func writeRow(id: Int) -> Bool
  uses Database.Write
{
  return true
}

func save(id: Int) -> Bool
  uses Database.Write          // ← writeRow を呼ぶので、ここにも宣言が要る
{
  return writeRow(id)
}
```

✗ よくある誤り(`save` がエフェクト宣言を忘れている → `KEI-E3001`):

```text
func save(id: Int) -> Bool {        // uses が無い
  return writeRow(id)               // writeRow は uses Database.Write
}
```

### 階層で包んでもよい

宣言は階層の上位ノードで包含できる(粗いほど合意書の価値は下がるが合法)。

```kei
module fx.hierarchy

func writeRow(id: Int) -> Bool
  uses Database.Write
{
  return true
}

func viaDatabase(id: Int) -> Bool
  uses Database                // Database.Read と Database.Write を包含
{
  return writeRow(id)
}

func viaIO(id: Int) -> Bool
  uses IO                      // 全 IO を包含(雑だが合法)
{
  return writeRow(id)
}
```

### 標準エフェクト階層(v0.1 で使えるのはこれだけ)

| エフェクト | 意味 |
|---|---|
| `IO` | 全 IO の包括(下位すべてを含む) |
| `Network.Read` / `Network.Write` | ネットワーク入出力 |
| `File.Read` / `File.Write` | ファイル入出力 |
| `Database.Read` / `Database.Write` | DB 入出力(`Database` で両方を包含) |
| `Clock` | 現在時刻の取得 |
| `Random` | 乱数 |
| `Audit.Log` | 監査ログ |

中間ノード(`Network` / `File` / `Database`)も書ける。これ以外を書くと `KEI-E3002 unknown effect`(ユーザー定義エフェクトは v0.2 以降)。

### 外部境界(extern / v0.2)

外部呼び出し(`Database.*` / `Time.now()` 等)は既定で opaque だが、`extern` 署名を宣言すると
**戻り型とエフェクトが検査される**。境界を合意書に載せたいときに使う。

```kei
module fx.extern

import infra.time as Time

extern Time.now() -> Int uses Clock

func recordedAt() -> Int
  uses Clock                  // ← extern が uses Clock を宣言しているので、ここにも要る
{
  return Time.now()
}
```

- `extern <名前空間パス>(<引数>) [-> <戻り型>] [uses <エフェクト...>]`。import の後に置く。
- 宣言したエフェクトは呼び出し元の `uses` へ推移伝播する。書き忘れると `KEI-E3001`(↓§7)。
  ✗ 上記で `uses Clock` を消すと「`Clock` used but not declared ... required by call to 'Time.now'」。
- 戻り型は型検査に伝播する(`match` / `else fail` で開ける)。取り違えは `KEI-E2001`。
- opt-in。`extern` の無い外部呼び出しは従来どおり opaque で通る(段階移行)。
- 重複署名は `KEI-E3003`、`uses` に標準外エフェクトは `KEI-E3002`。

---

## 4. 契約

- `requires` … 事前条件(呼び出し側の義務)。引数の前提を書く。
- `ensures` … 事後条件(実装側の義務)。`result`(戻り値)と `old(expr)`(呼び出し前の値)が使える。
- v0.1 では契約は**実行時アサーション**にトランスパイルされる。違反は `KeiContractViolation` で即死。

### 契約式は純粋でなければならない

契約式の中で**エフェクトを持つ関数を呼んではいけない**(`KEI-E4001`)。将来の静的証明を壊さないため。純粋関数だけ呼べる。エフェクトを伴う取得は本体で行い、結果をパラメータや `let` で渡す。

```kei
module contract.pure

func nonNegative(value: Int) -> Bool {     // 純粋なので契約式から呼べる
  return value >= 0
}

func deposit(balance: Int, amount: Int) -> Int
  requires nonNegative(amount)
  requires amount > 0
  ensures result == old(balance) + amount
  ensures result >= old(balance)
{
  return balance + amount
}
```

### 数量の保存を ensures で表すイディオム

「ちょうど step だけ増える」「ちょうど amount だけ減る」を事後条件で固定する。

```kei
module contract.conserve

func increment(count: Int, step: Int) -> Int
  requires step > 0
  ensures result == old(count) + step       // ちょうど step 増える
{
  return count + step
}
```

`old()` と `result` は **`ensures` 専用**。`requires` や本体で使うと `KEI-E4002`。

### 外部状態の数量保存は純粋ヘルパーへ退避する(v0.2 / 推奨)

「在庫がちょうど 1 減る」のような**外部状態(DB 等)の数量的契約**は、関数全体の `ensures` には
書けない(契約式は副作用禁止で DB を読めず、`old()` は引数しか見られない)。数量変換を
**現在値を引数で受ける純粋ヘルパー**に切り出し、その `requires`/`ensures` で表す。本体は外部状態を
読み → ヘルパーで次の値を計算 → 書き戻す。**本体は必ずヘルパーを経由する。**

```kei
module contracts.conserve_external

func decrementAvailable(available: Int) -> Int
  requires available > 0
  ensures result == old(available) - 1
{
  return available - 1
}
```

実物は `examples/contracts/borrow.kei`。背景と限界は `spec/kei-spec-v0.2.md` §4、言語拡張の比較は
`docs/effect-postconditions-memo.md`。

---

## 5. 失敗の扱い(null も例外も無い)

- **「見つからないかもしれない」= `Option<T>`** … 成功は `Some(x)`、不在は `None()`。
- **「明確な失敗理由がある」= `Result<T, E>`** … 成功は `Ok(x)`、失敗は `Err(e)`。`E` は普通 `enum` で定義する。

```kei
module lookup.kinds

enum LookupError {
  NotFound(Int)
  Forbidden { who: Int }
}

func find(id: Int, exists: Bool) -> Option<Int> {
  if exists {
    return Some(id)
  }
  return None()
}

func load(id: Int, allowed: Bool, exists: Bool) -> Result<Int, LookupError> {
  if allowed {
    if exists {
      return Ok(id)
    }
    return Err(LookupError.NotFound(id))
  }
  return Err(LookupError.Forbidden { who: id })
}
```

### `else fail` で早期脱出

`Option` / `Result` を返す式を `let ... = expr else fail <Err>` で受けると、不在/失敗時にその関数から `<Err>` で即脱出し、成功時は中身が束縛される。

```kei
module withdraw.elsefail

import infra.database as Database

enum WithdrawError {
  NotFound(Int)
}

func balanceOf(account: Int, amount: Int) -> Result<Int, WithdrawError>
  uses Database.Read
{
  let current = Database.fetchBalance(account) else fail WithdrawError.NotFound(account)
  return Ok(current - amount)
}
```

- `else fail` は `Option` を返す式・`Result` を返す式の**どちらにも**使える(成功時は中身が束縛され、不在/失敗時に脱出する)。
- `fail` の後ろは囲む関数の戻り型に合わせた**裸のエラー値**を書く。`Result<_, E>` を返す関数なら `E`(`Err` は自動で被さる)。`fail Err(...)` と二重に包むと型不一致(`KEI-E2001`)。
- `Result` のメンバは `.isOk` / `.isErr`、`Option` のメンバは `.isSome` / `.isNone` のみ。
- `Result<T, E>` と `Option<T>` は混同不可(`KEI-E2001`)。中身を裸で返すのも型不一致 → `Ok(...)` / `Some(...)` で包む(`return Ok(x)` であって `return x` ではない)。

### `match` で中身を取り出す(純粋文脈でも使える / v0.2)

`else fail` は **Result を返す関数**でしか使えない(失敗時に `Err` で脱出するため)。
**純粋関数の内部で Option / Result / enum を開く**なら `match` 式を使う。`match` は **式**で、
各腕の式の型が一致し、その型が全体の型になる。

```kei
module match.basics

func isOverdue(daysLeft: Option<Int>) -> Option<Bool> {
  return match daysLeft {
    Some(d) => Some(d < 0)
    None => None()
  }
}
```

- パターンは 1 段:`Option` は `Some(x)` / `None`、`Result` は `Ok(x)` / `Err(e)`、
  enum は `Enum.V` / `Enum.V(a, b)` / `Enum.V { a, b }`(**構築形と対称**。`Color.Red` のように enum 名を付ける)。
- 名前付きフィールドは**全フィールドを列挙**する(`Rect { w, h }`)。束縛変数は**その腕の中だけ**で有効。
- **網羅性は必須。** 全バリアントの腕を書く(`_` ワイルドカードは無い)。漏れると `KEI-E2007`。
  これにより enum にバリアントを足すと既存の `match` が必ずエラーになり、追従漏れを防げる。
- 腕の区切りは改行(`kei fmt` が正規化)。腕の本体は式のみ(文ブロック不可)。

```kei
module match.enum

enum Light {
  Red
  Yellow
  Green
}

func canGo(light: Light) -> Bool {
  return match light {
    Light.Red => false
    Light.Yellow => false
    Light.Green => true
  }
}
```

`match` のエラー: `KEI-E2007`(網羅漏れ)/ `KEI-E2008`(到達不能な重複腕)/
`KEI-E2009`(パターンが型に不適合 — 型と違うコンストラクタ族・存在しないバリアント・束縛形違い)/
`KEI-E2001`(腕の式の型不一致)。詳細は `spec/errors/<code>.md` と `spec/kei-spec-v0.2.md` §1。

---

## 6. イディオム集(実ファイルから)

すべて check-clean が保証された実物。詳細は各ファイルを開く。

- **record と純粋関数** — `examples/basics/records.kei`(`Point`、`Point { x: ..., y: ... }` 構築、フィールドアクセス)
- **Option の分岐** — `examples/basics/options.kei`(`Some` / `None()` の返し分け)
- **enum と tagged 型** — `examples/basics/enums.kei`(unit / 位置 / 名前付きバリアント、`type X = String tagged "X"`)
- **契約と数量保存** — `examples/contracts/counter.kei`(`requires` / `ensures` / `old` / `result`)
- **match で網羅分解** — `examples/basics/matching.kei`(Option / Result / enum を `match` で開く。純粋文脈で Option を分解)
- **外部境界 + 数量保存** — `examples/contracts/borrow.kei`(`extern` で境界検証、純粋ヘルパー `decrementAvailable` で「ちょうど 1 減る」を担保)
- **エフェクト + Result + else fail** — `examples/contracts/withdraw.kei`、`examples/effects/transfer.kei`(`uses` 複数、`Audit.Log.record(...)`、`Err(... { ... })` 構築)

`transfer.kei` の本体は「取得 → 早期脱出 → 検査 → 副作用 → 監査 → `Ok` 返却」という実務の定型:

```kei
module effects.transfer

import core.money { AccountId, Money }
import infra.audit as Audit
import infra.database as Database

record TransferReceipt {
  from: AccountId
  to: AccountId
  amount: Money
}

enum TransferError {
  NotFound(AccountId)
  InsufficientFunds { needed: Money, had: Money }
}

func transferFunds(from: AccountId, to: AccountId, amount: Money) -> Result<TransferReceipt, TransferError>
  uses Database.Read, Database.Write, Audit.Log
  requires amount > Money.zero
  requires from != to
{
  let sender = Database.fetchAccount(from) else fail TransferError.NotFound(from)
  if sender.balance < amount {
    return Err(TransferError.InsufficientFunds { needed: amount, had: sender.balance })
  }
  Database.debit(from, amount)
  Database.credit(to, amount)
  let receipt = TransferReceipt { from, to, amount }
  Audit.Log.record(receipt)
  return Ok(receipt)
}
```

---

## 7. よくあるエラーと直し方(頻度順)

各エントリは「症状 → 原因 → 直し方」。コードは ✗(誤)→ ✓(正)。詳細解説は `spec/errors/<code>.md`。

### KEI-E3001 effect used but not declared(最頻出)

呼び出し先のエフェクトを呼び出し元が `uses` に宣言していない。**推移的伝播の宣言漏れ**。

✗:
```text
func save(id: Int) -> Bool {           // uses が無い
  return writeRow(id)                  // writeRow は uses Database.Write
}
```
✓: 呼び出し元に不足エフェクトを足す(`fixes[].edits` がそのまま使える)。
```kei
module e3001.fix

func writeRow(id: Int) -> Bool
  uses Database.Write
{
  return true
}

func save(id: Int) -> Bool
  uses Database.Write
{
  return writeRow(id)
}
```

### KEI-E3002 unknown effect

`uses` に標準階層に無いエフェクトを書いた(多くは typo)。

✗: `uses Database.Wirte` → ✓: `uses Database.Write`。使えるのは §3 の表のものだけ。

### KEI-E2001 type mismatch

式の型が期待型と合わない。暗黙変換は無い。`if` 条件が `Bool` でない、`return` が戻り型と違う、引数の型/個数違い、`Result`/`Option` の混同、中身を裸で返す、などを含む。

✗:
```text
func loadUser(known: Bool) -> Result<Int, String> {
  return 3                 // Int を返している。期待は Result<Int, String>
}
```
✓: `Ok(...)` / `Some(...)` で包む(機械適用 fix あり)。
```kei
module e2001.fix

func loadUser(known: Bool) -> Result<Int, String> {
  return Ok(3)
}
```

### KEI-E1001 undefined name / KEI-E1002 undefined type

スコープに無い値名(変数・関数・import 名)/ 定義も import もされていない型名を参照した。暗黙 import は無い。

- 直し方: typo なら fix の `Did you mean ...?` を適用。外部のものなら `import` する。新しい型なら `record` / `enum` / `type` で宣言する。組み込み型は `Int` / `String` / `Bool` / `Result` / `Option` のみ。

### KEI-E4001 effectful call in contract

`requires` / `ensures` の中でエフェクト付き関数を呼んだ。

✗:
```text
func withdraw(id: Int, amount: Int) -> Int
  requires currentBalance(id) >= amount   // currentBalance は uses Database.Read
{
  return amount
}
```
✓: 取得は本体で行い、値をパラメータで受けて契約式は純粋な値だけ参照する。
```kei
module e4001.fix

func withdraw(balance: Int, amount: Int) -> Int
  requires balance >= amount
{
  return amount
}
```

### KEI-E4002 contract-only construct misused

`old(...)` / `result` を `ensures` 以外(`requires` や本体)で使った。

✗: `requires old(balance) >= 0` → ✓: その条件を `ensures` に移すか、`requires` ではパラメータの現在値(`balance >= 0`)を直接参照する。

### KEI-E2003 / KEI-E2004 バリアント・record リテラルの形違い

- enum: unit は `E.V`、位置は `E.V(...)`、名前付きは `E.V { ... }`。`E { ... }` や `E(...)`(enum 名だけの構築)は不可。
- record: 必ず `Name { ... }`。必須フィールド欠落・フィールド重複・関数呼び出し形での構築(`Point(1, 2)`)は不可。

✗: `return Point { x: 1 }`(`y` 欠落)→ ✓: `return Point { x: 1, y: 2 }`。

### KEI-E2005 tagged type confusion

tagged 型と基底型、または別の tagged 型を混同した。`type AccountId = String tagged "AccountId"` の `AccountId` は `String` と互換でない(それが存在意義)。

✗: `String` をそのまま `AccountId` 引数に渡す → ✓: 期待される tagged 型の値を構築して渡すか、両辺の型を揃える。

### KEI-E0102 reserved keyword as identifier

予約語を識別子に使った。予約語: `module import as type record enum func uses requires ensures let if else fail return tagged true false implies match extern`。

✗: `let type = 1` → ✓: 別名にする(`let kind = 1`)。`.` の後ろのメンバ名は同綴りでも可(`Audit.Log.record`)。

### 構文エラー(KEI-E0001 / E0101 / E0103 ほか)

- `KEI-E0001 unexpected character` … `&` `@` など未対応文字。§2 の演算子表以外は使わない。
- `KEI-E0101 unexpected token` … 区切り(`,` `)` `:` など)の欠落。`expected ...` に従って補う。
- `KEI-E0103 unclosed delimiter` … `{` `(` の閉じ忘れ。span は開き位置を指す。
- `KEI-E0104 unknown contract clause` … 契約節に `uses`/`requires`/`ensures` 以外の語(例: `use`)。正しい節キーワードに直す。

その他のコード(`KEI-E0002`/`E0003`/`E0004` 字句、`KEI-E1003` 重複定義、`KEI-E1004` import 衝突、`KEI-E2002` 不明フィールド、`KEI-E2006` 型引数の個数)は `spec/errors/<code>.md` を参照。

---

## 8. 検証ループの回し方

1. `kei check <file> --json` を実行し、`CheckReport`(`{ "diagnostics": Diagnostic[], "contracts": ContractInfo[] }`)を得る。
   - `diagnostics` … 従来の Diagnostic 配列(これを読んで直す)。
   - `contracts` … 各 requires / ensures の**達成検証レベル**(v0.2 / M12)。`{ func, kind, expr, verification, span }`。
     `verification` は `static`(コンパイル時に成立確定)/ `runtime`(実行時アサーション。大半はこれ)/ `trusted` / `unchecked`。
     **「契約が書かれた」と「機械検証された」は別物**で、これはその到達度の報告。検証レベルはソース構文には書かない(`spec/kei-spec-v0.2.md` §3)。
2. 各 Diagnostic を読む。構造は:

   | フィールド | 内容 |
   |---|---|
   | `severity` | `"error"` / `"warning"` / `"info"` |
   | `code` | `KEI-Exxxx`。解説は `spec/errors/<code>.md` |
   | `message` | 一文の説明(英語) |
   | `span` | `file` と `start`/`end`(`line`・`col` は 1 始まり) |
   | `fixes` | 修正候補(優先度順)。各 `fix` は `title` と `edits`(`TextEdit[]`) |

3. `fixes[].edits` が**非空**なら、その `span` を `new_text` で置換すれば機械適用できる(挿入は `start == end` の span)。空配列なら方向だけ示すので `title` に従って手で直す。
4. 直して 1 に戻る。**error が 0 件になるまで繰り返す**。それが完成。

実際の出力例(`KEI-E3001`、推移的伝播の宣言漏れ。`fixes[].edits` が `uses` 節への `, Database.Write` 挿入を機械適用可能な形で持つ):

```json
[
  {
    "severity": "error",
    "code": "KEI-E3001",
    "message": "effect 'Database.Write' used but not declared in 'uses' clause of 'audit' (required by call to 'writeRow')",
    "span": {
      "file": "x.kei",
      "start": { "line": 16, "col": 10 },
      "end": { "line": 16, "col": 22 }
    },
    "fixes": [
      {
        "title": "Add 'Database.Write' to uses clause",
        "edits": [
          {
            "span": {
              "file": "x.kei",
              "start": { "line": 14, "col": 13 },
              "end": { "line": 14, "col": 13 }
            },
            "new_text": ", Database.Write"
          }
        ]
      }
    ]
  }
]
```

`--json` を付けない既定出力は同じ情報の散文(`error[KEI-E3001]: ...` と `--> file:line:col`、`= fix: ...`)。契約があれば末尾に `verification:` ブロック(`<func> <kind> <expr> [<level>]`)も出る。機械処理は `--json`、目視は既定で。

### 整形

正規形は `kei fmt <file>`(既定 stdout、`--check` で未整形を検出、`--write` で上書き)。生成後に `--write` で整えておくとレビュー diff が意味的差分だけになる。

---

## 参照

- `spec/kei-spec-v0.1.md` — 言語仕様(source of truth)
- `spec/kei-spec-v0.2.md` — v0.2 差分章(`match` / `extern` / 検証レベル / 数量契約イディオム)
- `spec/kei-spec-v0.3-collections.md` — v0.3 コレクション(`List<T>` 段階1)
- `spec/diagnostic-schema.md` — Diagnostic の確定スキーマ
- `spec/errors/<code>.md` — 各エラーコードの解説
- `examples/` — check-clean な実例(basics / contracts / effects)
