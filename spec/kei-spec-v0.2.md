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

ただしこの寛容パスは**実行時に値が Kei の内部表現(`.ok` / `.kind`)を持つことを仮定する**。
トランスパイル(§1.5)は腕のコンストラクタからこれらの判別子を生成するため、外部値が
Option/Result/enum の Kei 表現でなければどの腕にもマッチせず、末尾の
`throw new Error("non-exhaustive match")` にサイレントに落ちる。境界を静的に検証したいなら、
その外部呼び出しに `extern` 署名(§2)を宣言してスクルティニの型を確定させる
(型が解決すれば網羅性検査が効き、末尾 throw は到達不能になる)。

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

#### strict-extern モード(v0.3 / M16 / #44)

第二段階として、**未宣言の外部呼び出しを検出する厳格モード**を `kei check --strict-extern`
として導入する(opt-in)。

- **検出対象:** import した名前空間配下の呼び出し(例 `Time.now()`)で `extern` 署名が
  **無い**もの。これらは既定では opaque で素通りし、#22 の元の再現コード(`extern` を書かずに
  `Time.now()` を呼び `uses Clock` を忘れる)を**検出できない**。strict ではこの呼び出しを
  **`KEI-E3004`(warning)** で指摘し、`extern` 宣言の追加(`fix`)を促す。
- **既定は不変:** `kei check`(フラグなし)は従来どおり opaque で通す。strict は完全な opt-in で、
  v0.1 の `ok_*` 検査群(外部呼び出しを別目的で多用)を一切壊さない。flag-day 移行はしない。
- **段階:** まず **(a) warning**(`KEI-E3004` の severity は `warning`。exit code は変えない)。
  成熟後に **(b) error** への引き上げを再評価する。HANDOFF「スコープ発散が死因」に従い、
  enforcement-when-declared(§2.2)を確立した上での次の一歩として最小に入れる。
- **prelude `extern` の配布**は本段階では行わない(`Time` / `Database` 等の組み込み署名を配るかは
  別途。strict はあくまで「宣言の不在を可視化する」辺に留める)。

## 3. 契約の検証レベルを診断報告(M12 / #23)

「契約が**書かれている**こと」と「その契約が実際に**機械検証された**こと」は別物。
AI 時代の言語では、書かれた保証と検査された保証の区別が決定的に重要——「`kei check` が
通った」が「契約が証明された」を意味するとは限らない。v0.2 は両者を出力上で区別する。

### 3.1 設計判断: 検証レベルはソース構文に書かない(確定済み)

検証レベルは **ソース構文に書き分けない。** 代わりに `kei check` の診断出力(構造化データ)に
載せる。理由は 3 つ:

1. **合意書原則の保護(§1 第一条)。** 契約は「何を保証するか」に集中させ、承認時の認知負荷を
   上げない。`requires step > 0` に検証レベルの注釈が混ざると、レビュー対象が膨らむ。
2. **検証レベルは「処理系が達成できたレベルの報告」であって書き手の選択ではない。** 同じ契約でも、
   検証器が強くなれば `runtime` から `static` に上がりうる。それは契約の意味ではなく処理系の能力。
3. **契約は不変・検証は成長。** 検証器が強化されても契約ソースを書き換えずに済む。契約は仕様、
   検証レベルはその時点の到達度。両者を分離することで、ソースの安定性と検証の進歩を両立する。

→ `trusted` / `unchecked` の明示構文は **入れない**(v0.2)。検証レベルは構文に現れない。

### 3.2 検証レベル

`kei check --json` の各契約に `verification` が付く(`spec/diagnostic-schema.md` の
`CheckReport` / `ContractInfo`)。値は 5 種(強さ: `static` > `generative` > `runtime`):

| 値 | 意味 |
|---|---|
| `static` | コンパイル時に成立が判定済み |
| `generative` | 契約から生成した property-based test で反例ゼロ(v0.3 / M15 / #26) |
| `runtime` | 実行時アサーションへ展開(v0.1 既定) |
| `trusted` | 外部システム・人間レビュー・テストで保証(検証器の管轄外) |
| `unchecked` | 明示的に未検証 |

### 3.3 v0.2 の判定ロジック(最小実装)

- **大半は `runtime`。** v0.1 同様、契約は実行時アサーションへ展開される。
- **自明な定数畳み込みで真になる契約は `static`。** 変数を含まない純粋式が `true` に畳めるもの
  (`requires true` / `requires 1 > 0` / `ensures 2 + 2 == 4`)は、コンパイル時に成立が確定するため
  `static`。これは健全な静的検証の最小形(定数恒真は処理系が証明済み)。
- **定数畳み込みで偽になる契約は静的エラー(`KEI-E4003`、v0.3 / M17 / #35)。** 変数を含まない純粋式が
  `false` に畳めるもの(`requires false` / `requires 1 > 2` / `requires "a" == "b"`)は、処理系が
  **反証済み**で実行すれば必ず違反する。`runtime` アサーションへ落とさず、コンパイル時に `KEI-E4003`
  (常に偽の契約)で弾く。畳める範囲は Int / Bool / String リテラルの算術・比較・論理式。
  判定は検証レベルと同じ定数畳み込みを共有し、`AlwaysTrue → static` / `AlwaysFalse → KEI-E4003` /
  `Unknown → runtime` の三分岐に揃える(片方だけ分岐が増える乖離を防ぐ)。
- `trusted` / `unchecked` は v0.2 では**産出しない**(将来 extern 契約境界や明示的検証除外の
  導入時に使う、前方互換のための予約値)。
- **本格的な static 検証(SMT ソルバ連携)は v1.0 送り。** `step > 0`(引数依存)などは現状 `runtime`。

### 3.4 散文出力

`--json` が正、散文は派生(§1 第五条)。既定の `kei check`(散文)は、診断の後に契約の検証レベルを
要約する `verification:` ブロックを欠落なく描く:

```text
verification:
  increment requires step > 0  [runtime]
  increment requires true  [static]
  increment ensures result == old(count) + step  [runtime]
```

## 4. 外部状態の数量的契約: 純粋ヘルパー経由(M13 / #21 短期分)

「`borrowBook` を呼ぶと在庫が**ちょうど 1 減る**」を関数全体の `ensures` で書きたい。
しかし v0.1/v0.2 の制約上それは直接書けない:

1. **契約式は副作用禁止**(§4 / 将来の静的証明を壊さないため)——DB を読めない。
2. **`old()` は引数しか参照できない**——外部状態(DB の在庫数)の「呼び出し前の値」を
   `old()` で捉えられない。

放置すると、**「`kei check` が通った」と「在庫が 1 減ることが保証されている」が乖離する**
(書かれていない契約は検査されない)。v0.2 はこのギャップを**正式イディオム**で塞ぐ。

### 4.1 正式イディオム: 数量保存を純粋ヘルパーへ退避する

> **外部状態の数量的契約は、純粋ヘルパー関数へ切り出し、本体は必ずそれを経由する。**

数量の変換規則(「ちょうど 1 減る」)を、現在値を**引数で受け取る純粋関数**に閉じ込め、その
`requires` / `ensures` で表す。エフェクトを伴う本体は、外部状態を読み、純粋ヘルパーで次の値を
計算し、外部状態へ書き戻す——必ずヘルパーを経由する。

```kei
func decrementAvailable(available: Int) -> Int
  requires available > 0
  ensures result == old(available) - 1     // 「ちょうど 1 減る」をここで保証
{
  return available - 1
}

func borrowBook(book: BookId) -> Result<Int, BorrowError>
  uses Database.Read, Database.Write
{
  let available = Database.fetchAvailable(book) else fail BorrowError.NotFound(book)
  if available <= 0 {
    return Err(BorrowError.OutOfStock(book))
  }
  let next = decrementAvailable(available)   // ← 数量変換は必ずヘルパー経由
  Database.setAvailable(book, next)
  return Ok(next)
}
```

完全な check-clean 例は `examples/contracts/borrow.kei`(e2e `tests/e2e/tests/borrow.test.ts`)。

### 4.2 このイディオムの担保と限界

- **担保できること:** 数量変換そのもの(`next == available - 1`)は純粋ヘルパーの契約として
  検査される(v0.2 では runtime アサーション)。
- **担保できないこと(構造的限界):** 「本体が**必ず**ヘルパーを経由する」「`fetchAvailable` で
  読んだ値を**そのまま** `setAvailable` で書く」という**接続**は、言語が強制しない。本体の
  正しさはレビュー(合意書)に依存する。これは「言語の制約に構造を合わせた」回避策であって、
  外部状態の事後条件を素直に書けているわけではない。
- → 素直な表現(`ensures Database.availableOf(book) == old(...) - 1`)を可能にする言語拡張は
  **v0.3 / §4.3(案1)で実装した**。比較検討は `docs/effect-postconditions-memo.md`。

### 4.3 案1: 外部状態の事後条件を直接書く(`extern query` 観測子 / v0.3 / M14 / #45)

§4.2 の限界(本体とヘルパーの接続がレビュー依存)を、`borrowBook` 自身の `ensures` に
外部状態の遷移を**直接**書けるようにして閉じる。中核は **純粋観測子(query)** の導入。

#### 構文: `extern query`

```text
extern query Database.availableOf(book: BookId) -> Int          // 純粋観測子(論理的読み取り)
extern Database.setAvailable(book: BookId, count: Int) uses Database.Write
```

- `extern query <パス>(...) -> <型>` は、**副作用のない読み取り専用の観測子**を宣言する。
  `query` は `extern` の直後にだけ置ける文脈依存キーワード(`extern query.foo()` のように
  名前空間名としての `query` は従来どおり使える)。
- **query は `uses` を持てない**(純粋であることが定義。`uses` を付けると `KEI-E3005`)。
  純粋性は **`extern` 宣言を信頼する(trusted)** ——宣言が嘘なら契約も嘘になる(健全性の根)。

#### 契約での参照と `old()`

```text
func borrowBook(book: BookId) -> Int
  uses Database.Write
  requires Database.availableOf(book) > 0
  ensures Database.availableOf(book) == old(Database.availableOf(book)) - 1
{ ... }
```

- 契約式の中から呼べる外部関数は **query 観測子だけ**。非 query の `extern` を契約から呼ぶと
  `KEI-E4004`(副作用の有無に関わらず。契約は状態を**観測**するもので、作用させるものではない)。
- `old(...)` を**観測子の呼び出しに拡張**する。`old(Database.availableOf(book))` は関数進入時点の
  観測値を、`ensures` 評価時(退出時)の観測値と比較する。実装は関数進入時に観測子を一度評価して
  退避し(`const kei$old$i = ...`)、退出時に再評価して比較する(M11 emit の `old` 機構の自然な延長)。
- **副作用禁止規則との整合:** 観測子呼び出しは「論理的読み取り=純粋」なので、契約式の副作用禁止
  (§1 / 第二条)に**反しない**。エフェクト付き関数(非 query extern・`uses` 付きローカル関数)を
  契約から呼ぶことは引き続き禁止(`KEI-E4001` / `KEI-E4004`)。

#### 担保されること(§4.2 との差)

- `borrowBook` の**シグネチャ + 契約だけ**を読めば「成功時に在庫がちょうど 1 減る」が分かる。
  純粋ヘルパーへの退避を強制されない。
- 本体が実際に在庫を 1 減らさなければ(2 減らす / 減らし忘れる)、退出時の観測値が `old - 1` に
  ならず、`ensures` 違反として実行時に必ず露見する(`examples/contracts/borrow_direct.kei` +
  `tests/e2e/tests/borrow_direct.test.ts` の反例 `borrowBookOffByTwo` が `KeiContractViolation`)。
- **検証レベル:** 案1 はまず `runtime`(観測子を実行時に呼んで比較)。`kei check --json` の
  `contracts[].verification` は `runtime` を報告する(将来 `static` への引き上げは SMT 連携で)。

#### スコープ外(案2 は入れない)

ゴースト/モデル変数 + `modifies` 節(Dafny / Verus 式)は入れない。実装が重く、#25(コレクション)と
量化(`forall`)の成熟が前提のため v0.3 以降で再評価する(`docs/effect-postconditions-memo.md`)。

## 5. 契約ベース PBT 生成(`kei check --generative` / v0.3 / M15 / #26)

Kei の契約はテストの二大要素を内包する: `requires` = 入力の生成制約、`ensures` = テストオラクル。
テストの本質的コストは「期待値を誰が決めるか(オラクル問題)」だが、Kei は契約が一級市民なので
**オラクルが既に書かれている**。`kei check --generative` は契約を読んで property-based test を
生成・実行する(段階1)。シードで入力を補える(段階2)。

### 5.1 中核原則: 捏造不能性(オラクルは契約のみ・シードは入力のみ)

> **テストのオラクルは契約(`ensures`)だけが担う。生成器もシードも「入力」しか供給せず、
> 「期待値」を持たない。**

AI がテストを通す方法を「実装を契約に合わせる」ことだけに限定し、「テストを通すために期待値を
歪める」**捏造経路を言語構造から排除**する。これは第一条(人間は契約を承認、AI は実装)の権限分離を
テストドメインへ拡張したもの。シードファイル文法には期待値を書く構文が**存在しない**——`expected` /
`output` / `result` を書くと文法エラー(`KEI-E4006`)。テストが書けない=契約が不足のサインであり、
**契約を強化して**解決する(テストを水増ししない)。

### 5.2 段階1: 純粋関数の生成テスト

- **対象:** 純粋関数(`uses` なし)で `ensures` を持ち、パラメータがスカラ(`Int` / `Bool` / `String`)の
  もの。評価器が扱えない構文(`match` / record / `Option` / `Result` / 外部呼び出し)を含む関数は
  静かに対象外(`generative` には上がらず `runtime` のまま)。**制約ソルバ(SMT)は持ち込まない**——
  複雑な `requires` は段階2のシードで補う。
- **生成:** 各パラメータ型の決定的な候補集合(再現性のため乱数を使わない)から入力を組み、`requires` を
  満たすものだけを検査対象にする。例: `decrementAvailable(available: Int) requires available > 0` は
  正の候補だけが残る。
- **判定:** 各入力で関数を評価し、`result` と `old(param)`(純粋関数では進入時=入力値)を束縛して
  全 `ensures` を評価する。全入力で反例ゼロなら、その `ensures` を **`generative`** に格上げする。
- **反例の最小化:** `ensures` 違反は**最小化された入力**付きで `KEI-E4005` として報告する
  (`Int` は絶対値、`String` は長さ最小)。例: `return available - 2` は `available = 1` で落ちる。
- 生成・判定ロジックは `kei_check`(`pbt` モジュール)に置き、`kei_cli` は委譲のみ(CLAUDE.md)。

### 5.3 段階2: シード注入

人手で edge case の**入力**を与えるシードファイル(`<stem>.seeds`)を `--generative` 時に自動検証する。

```text
seeds for decrementAvailable {
  input { available: 1 }
  input { available: 0 }   # requires available > 0 を満たさない → KEI-E4007 で弾かれる
}
```

- **文法は入力のみ。** `input { <field>: <literal> }` だけで、期待値を書く構文は無い(§5.1)。
- **requires 適合検証:** シード入力を対象関数の `requires` に照らし、満たさないシード(無効なシード)を
  `KEI-E4007` で弾く。名前・個数・型の不一致も同コードで報告する。
- `given`(初期外部状態)は段階3(エフェクト関数の生成テスト)の枠で、本段階では導入しない。

### 5.4 スコープ外(段階3・4)

- 段階3: エフェクト関数の生成テスト(エフェクトハンドラ + シードの `given`)。エフェクトハンドラ実装が前提。
- 段階4: 量化契約(`forall`/`exists`)のテスト。#25 段階2 が前提。
