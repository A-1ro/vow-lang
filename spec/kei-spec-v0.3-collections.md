# Kei 言語仕様書 v0.3 — コレクション型(立場B / Draft)

> Code is a kei between humans and AI.
> この文書は [issue #25](https://github.com/A-1ro/kei-lang/issues/25) の中核成果物。
> 本体仕様 `kei-spec-v0.1.md` の **§10「射程の宣言(立場B)」** から参照される拡張ドラフト。

## 0. ステータス

- **段階1 実装完了(v0.3 / M9)。** `List<T>` は `Result` / `Option` と並ぶ第三の組み込み
  ジェネリクスとして kei_syntax(型として既存文法でパース)/ kei_check(型・エフェクト・
  契約)/ kei_emit(`readonly T[]` + 配列メソッド)に入った。コンビネータ 8 種、案2(名前付き
  関数参照)、量化子なし契約を `tests/golden/check/` と `examples/collections/inventory.kei` /
  `tests/e2e/tests/inventory.test.ts` で固定。
- **リテラル構文(`[1, 2, 3]` / 空 `List<T>()`)は段階1フォローアップへ繰り越し(§3.1)。** 段階1の
  受け入れケース(複数エンティティ処理の検証 = `totalStockValue` / `planAllReorders`)は List を
  **引数で受け取りコンビネータで変換する**形で完結し、リテラルを必要としない。リテラル(特に空リストの
  要素型推論と `<` の曖昧性)は独立した文法判断のため切り出す。
- **立場B を採択した(#25 の決定)。** Kei は「1 エンティティ分の純粋コア DSL」から **システム記述言語** へ射程を広げる。`List` を `Result` / `Option` と同格の **第三の組み込みジェネリクス** として段階導入する。
- 実装は `docs/kei-roadmap-v0.3.md` の **M9(段階1)** 以降で /goal 単位に進める。本 issue #25 は v0.3 以降のコレクション系の **親 issue(epic)**。

## 1. 立場の決定: なぜ B か

#25 は「機能要望」ではなく **Kei の射程の宣言を迫る** issue だった。在庫管理のドッグフードで、

- 「発注点を下回った全商品の補充計画」(`planAllReorders`)
- 「在庫総額の集計」(`totalStockValue`)

という **複数エンティティ処理** がいずれも言語内で書けず、ホスト TS の `products.filter(...).map(...)` に追い出された。そこは `uses` も契約も効かない検証の外側であり、**システムの骨格(反復・集計)が常に検証外に出る**。

### 採択(立場B)

`List` を入れ、反復・集計・絞り込みを言語内に取り込む。そこにも `uses` と契約が効くようにする。これにより Kei は初めて「システムを書く」と言える。

### 退けた選択肢(立場A)

「純粋コア DSL に留まる(コレクションを持たない)」も思想として一貫していたが、**合意書(第一条)の及ぶ範囲が原理的に 1 エンティティに固定される** 弱点を許容できないと判断した。立場Aを退けたことをここに明記する。

### 立場B の代償(承知のうえで引き受ける)

- 契約に量化(`forall` / `exists`)が要る → **段階2に隔離**(下記 §6)。
- ジェネリクスの取り回し・コンビネータ標準ライブラリ・静的検証の難所(量化子)を引き込む。

これらを **一度に入れない**。#21・#23 と同じ「土台が先」の二段構えで刻む(§2)。

## 2. 段階設計(1 → 2 → 3)

| 段階 | 内容 | 想定 | 前提 |
|---|---|---|---|
| **1** | `List<T>` を不変・opaque な列として導入(**量化子なし**)。純粋コンビネータの最小集合。 | v0.3 | (本文書 §3〜§5) |
| **2** | 集合に対する量化契約(`forall` / `exists`) | v0.3 以降 | #23 の検証レベル報告が動いていること |
| **3** | `Map<K, V>`(必要性を段階2後に再評価) | 未定 | 量化と等価性の設計 |

**一度に全部入れない。** 段階1は量化子を持ち込まず、`all` / `any` で要素述語を表す(§5)。量化子(束縛変数 `forall x in xs`)は段階2まで導入しない。

## 3. 段階1: `List<T>` の設計原則

1. **第三の組み込みジェネリクスとして特別扱い。** `Result`(型引数2)・`Option`(型引数1)の前例を踏襲し、`List`(型引数1)を組み込みに加える。**ユーザー定義のジェネリクス一般化はしない**(`record Box<T>` 等は本 issue の射程外。スコープ膨張を防ぐ)。
2. **不変(immutable)。** `List<T>` に破壊的操作は無い。すべてのコンビネータは新しい `List` か値を返す。既存の `let`(再代入なし)・無副作用スタイルと整合する。
3. **opaque。** 内部表現は隠蔽。要素アクセスは `get(i) -> Option<T>`(範囲外は `None`)のみで、添字構文 `xs[i]` は導入しない(null 不在・失敗は型に出す原則の維持)。
4. **命令ループを入れない。** `for` / `while` は導入しない。反復はコンビネータで表現する。これは量化の追跡しやすさ(段階2の静的検証)と、契約が副作用を持たない前提を守るため。

### 3.1 リテラル(段階1フォローアップへ繰り越し)

```text
let xs = [1, 2, 3]          // List<Int>(未実装 — フォローアップ)
let empty = List<Int>()     // 空リスト(未実装 — フォローアップ)
```

> **段階1(M9)ではリテラルを実装しない。** 段階1の受け入れケースは List を関数の引数で受け取り
> コンビネータ(`map` / `filter` / `fold` …)で新しい List/値へ変換する形で完結し、ソース内で
> List を**構築**する必要がないため。リテラル構文には独立した文法判断が要る:
> - `[1, 2, 3]` は要素型を先頭要素から推論できるが、空リストは周辺文脈依存。
> - 空リストの明示形 `List<Int>()` は式位置で `<` を型引数と比較演算子のどちらに取るかの
>   曖昧性(generics-vs-less-than)を持ち込む。
>
> これらは段階1の核(型・コンビネータ・契約)と切り離せるため、リテラルは別の /goal で
> 文法を確定する。それまで List は引数・コンビネータ結果としてのみ現れる。

## 4. 段階1: コンビネータ API(最小集合)

`Result` / `Option` のメンバ(`.isOk` / `.isSome` …)と同じ作法。プロパティ風(引数なし)とメソッド風(引数あり)を混在させる。

> **`isEmpty` だけはメソッド形 `xs.isEmpty()`(引数なし)。** 当初プロパティ設計だったが、emit が
> `xs.isEmpty` を `xs.length === 0` へ**書き換える**ため、レコードが `isEmpty` フィールドを持つと
> フィールドアクセス `bag.isEmpty` を誤写しうる(emit は型情報を再計算しないので名前で判別する)。
> 呼び出し形にすると、フィールドアクセス(`bag.isEmpty`)と構文的に区別でき、かつレコードは
> 呼べるフィールドを持てない(`bag.isEmpty()` は検査が弾く)ので衝突が原理的に消える。`length` は
> 実在の配列プロパティへ素直に写る(書き換え不要)のでプロパティのまま。`isOk`/`isSome` も実在の
> ランタイムプロパティへ写るので衝突しない——書き換えが要る `isEmpty` だけが例外。

| コンビネータ | 形 | シグネチャ(概念) | 説明 |
|---|---|---|---|
| `length` | プロパティ | `List<T>.length -> Int` | 要素数 |
| `isEmpty` | メソッド | `List<T>.isEmpty() -> Bool` | 空か(呼び出し形。理由は後述) |
| `get` | メソッド | `List<T>.get(index: Int) -> Option<T>` | 範囲外は `None`(添字で死なない) |
| `map` | メソッド | `List<T>.map(f: (T) -> U) -> List<U>` | 各要素を変換 |
| `filter` | メソッド | `List<T>.filter(pred: (T) -> Bool) -> List<T>` | 述語が真の要素だけ残す |
| `fold` | メソッド | `List<T>.fold(init: U, f: (U, T) -> U) -> U` | 左畳み込み |
| `all` | メソッド | `List<T>.all(pred: (T) -> Bool) -> Bool` | 全要素が述語を満たすか(空なら真) |
| `any` | メソッド | `List<T>.any(pred: (T) -> Bool) -> Bool` | いずれかの要素が述語を満たすか(空なら偽) |

これ以上は段階1に入れない(`reduce` / `flatMap` / `zip` / `take` 等は必要性が出てから別途)。

### 4.1 ⚠️ 段階1の設計依存(🤝 着手前合意・最大の論点)

`map` / `filter` / `fold` / `all` / `any` は **関数を引数に取る**。ところが v0.1 は「**関数は値ではない**(`KEI-E2001 functions are not values in v0.1`)」。両者は衝突する。golden の `err_collection_*`(§7)が、まさにこの 2 つの障壁(`List` 未定義 + 関数が値でない)を記録している。

段階1の /goal を投入する前に、コンビネータ引数に関数を渡す機構を決める:

- **案1: 第一級関数値(関数型 `(T) -> U` + ラムダ)を全面導入。** 強力だが、関数値・クロージャ・型推論が一気に増え、合意書原則(承認時に読む量)への負荷も大きい。
- **案2(推奨): 名前付き関数参照を「コンビネータ引数位置でのみ」許す。** 第一級関数値は導入せず、`xs.map(toPlan)` のように **既存のトップレベル/同一モジュールの純粋関数名** だけを引数に書ける限定形にする。`KEI-E2001` の緩和をこの位置に閉じ込め、`let f = toPlan`(関数を値に束縛)は引き続き禁止のまま。スコープ膨張を最小化でき、立場B の「ジェネリクスを一般開放しない」方針とも整合する。

> **段階1は案2を採用した(M9 実装で確定)。** コンビネータの関数引数位置に書けるのは、
> トップレベル/同一モジュールの**名前付き関数**のみ(`xs.map(toReorderPlan)`)。第一級関数値・
> ラムダは導入せず、`let f = toReorderPlan`(関数の値束縛)は引き続き `KEI-E2001` で禁止のまま。
> 関数参照でない式(リテラル・スコープ変数・呼び出し)を引数位置に書くと `KEI-E2001`。
> エフェクト付き関数を渡すと、その `uses` は呼び出し元へ推移伝播する(本体では `KEI-E3001`、
> 契約式では副作用禁止により `KEI-E4001`。既存ルールで処理でき新ルールは不要。§8.1)。

## 5. 段階1: 量化子なしで書ける契約の範囲

段階1の契約式から **参照してよい** List 由来の項:

- `xs.length`(`Int`)、`xs.isEmpty()`(`Bool`)
- `xs.all(pred)` / `xs.any(pred)` の結果(`Bool`)。`pred` は名前付き純粋関数。
- `result.length` / `old(xs).length`(`ensures` 内、§4 と既存の `result` / `old` 規則のまま)

書ける契約の例(段階1の範囲):

```text
func totalStockValue(products: List<Product>) -> Int
{
  return products.fold(0, addProductValue)
}

func planAllReorders(products: List<Product>, targetLevel: Int) -> List<ReorderPlan>
  requires products.all(hasNonNegativeQuantity)   // 要素述語は all + 名前付き純粋関数で表す
  ensures result.length <= products.length         // 長さの比較は量化子不要
{
  return products.filter(needsReorder).map(toReorderPlan)
}
```

ここで `hasNonNegativeQuantity(p: Product) -> Bool` は純粋関数。**`all` / `any` が段階1における「量化子の代わり」** である。束縛変数を導入する量化(`forall p in products: p.quantity >= 0`)は段階1では **入れない**(§6)。

> `xs.all(pred)` で十分表せるものを束縛量化で書く必要はない。段階2は「`all`/`any` では素直に書けない量化(複数コレクションにまたがる関係、`exists` の証拠など)」のために導入する。

### 5.1 等値比較はスカラー限定(KEI-E2010)

`==` / `!=` は**スカラー限定**(`Int` / `String` / `Bool` と、それらを基底にする
tagged 型)。`List<T>`・`Option<T>`・`Result<T, E>`・レコード・enum など合成型への
`==` は **コンパイルエラー**(KEI-E2010)。トランスパイル先では `===`(参照等価)に
なり構造等価にならないため、`ensures result == xs.get(0)` のような自然な契約が
非空リストでは**常に偽**になってしまう(サイレントな不一致を避けて静的に弾く)。

要素を取り出して契約に使いたいときは、`xs.get(i)` の戻り(`Option<T>`)を直接
`==` で比べず、長さや述語(`all` / `any`)、あるいはスカラー化したフィールドで
表す。構造等価そのものの言語サポートは将来課題(導入時にこの制限を緩める)。

## 6. 段階2(将来): 量化契約 `forall` / `exists`

```text
func planAllReorders(products: List<Product>, targetLevel: Int) -> List<ReorderPlan>
  requires forall p in products: p.quantity >= 0
  ensures result.length <= products.length
{ ... }
```

- `forall` / `exists` は v0.1 では予約語ではない(`requires forall ...` は現状 **構文エラー** になる)。段階2で **契約専用の束縛量化構文** として文法に追加する。
- 量化子は静的検証(SMT)の難所。**#23「達成された検証レベルを診断で報告する」仕組みが先に動いていることを前提**にする。最初は `runtime` 検証レベル(実行時に全要素チェック)で出し、`static` への引き上げは検証器の成長に委ねる。
- **合意書原則(第一条)への配慮**: 量化契約は人間が承認時に読む量を増やす。語彙は最小に絞り、**ネストを避ける**指針を spec に置く。

## 7. 段階3(将来): `Map<K, V>`

段階2の後に必要性を再評価する。在庫の「商品ID → 在庫数」表現で欲しくなるが、**量化と等価性(キーの同値性)の設計が重い**ため、段階2の量化が固まるまで着手しない。

## 8. 検証思想との整合

### 8.1 純粋性とエフェクト伝播

コンビネータに渡す関数が純粋なら契約からも呼べる。エフェクト付き関数を `map` するなら、その `uses` は呼び出し元へ推移伝播する(既存のエフェクト規則で処理でき、新ルールは不要)。

### 8.2 合意書原則の負荷

`List` 自体はシグネチャの可読性を大きく損なわない。負荷が出るのは段階2の量化契約であり、そこを §6 の指針で抑える。

### 8.3 ジェネリクスを一般開放しない

`List` はあくまで `Result` / `Option` と並ぶ **特別扱い** に留める。ユーザー定義型パラメータ(`record Box<T>` 等)は本 issue の射程外。これによりスコープ膨張(型推論・分散・境界)を防ぐ。

## 9. トランスパイル(段階1 / 実装済み)

| Kei | TypeScript |
|---|---|
| `List<T>` | `readonly T[]`(不変配列。要素がさらに List なら `readonly (readonly U[])[]`) |
| `xs.length` / `xs.isEmpty()` | `xs.length` / `(xs.length === 0)` |
| `xs.get(i)` | `keiListGet(xs, i)`(範囲外 `None` を返す `@kei/runtime` ヘルパー) |
| `xs.map(f)` / `xs.filter(p)` / `xs.fold(z, f)` | `xs.map(f)` / `xs.filter(p)` / `xs.reduce(f, z)`(`fold` は引数順が逆) |
| `xs.all(p)` / `xs.any(p)` | `xs.every(p)` / `xs.some(p)` |

> **`get` はランタイムヘルパー方式を採用した。** `@kei/runtime` に `keiListGet<T>(xs, i): Option<T>`
> を追加(emit 展開だと添字式を 2 回評価する懸念があるため)。
>
> **衝突の回避は型情報で健全に行う(構文ヒューリスティックではない)。** メソッド呼び出しの書き換え
> (`get` / `fold` / `all` / `any` / `isEmpty`)は、**検査器が「その呼び出し位置が List レシーバ上の
> 操作だ」と確定した位置(Call span)だけ**に適用する。検査器が `kei_check::list_op_spans` で List 操作の
> 呼び出し位置集合を返し、emit はそれを唯一の根拠にする(`kei_check::contract_expr_text` を委譲するのと
> 同じく「検査の再実装をしない」境界)。これにより、レシーバの型を構文だけで推測しないので、
> 外部呼び出しの連鎖 `Database.reader().get(id)`・`let r = Database.fetch(); r.get(0)`(opaque 値)・
> 同名メソッドの外部呼び出し `extern Database.get(id)` をいずれも誤写しない。
> `isEmpty` を**メソッド形 `xs.isEmpty()`** にしているのも補強で、レコードの `isEmpty` フィールドアクセス
> `bag.isEmpty`(書き換えない)と構文的に区別できる(§4 の注)。

## 10. ドッグフードの目標ケースの落とし所

#25 が固定を求めた 2 ケースは、立場B の段階設計で次のように落ちた(`tests/golden/check/` で固定):

| ケース | 立場B での扱い | 段階 | golden(M9 完了後) |
|---|---|---|---|
| `totalStockValue` | **書ける**(量化子不要、`fold`) | 段階1 | `ok_collection_total_stock_value`(check-clean) |
| `planAllReorders` | **書ける**(要素述語は `all`、長さは `result.length`) | 段階1 | `ok_collection_plan_all_reorders`(check-clean) |

両 golden は M9 完了で `err_`(未実装ゆえコンパイルしない)から **`ok_` へ移した**(人間レビュー必須の
golden 変更=段階1 の受け入れ信号)。立場A なら両者は恒久的に「意図的に書けない」に落ちたはずだが、
立場B では「段階1で書けるようになった」。実行可能な統合例は `examples/collections/inventory.kei`
(+ `tests/e2e/tests/inventory.test.ts`)で、TS(`readonly T[]` + 配列メソッド)へトランスパイルされ
`tsc --strict --noEmit` がエラーゼロ・vitest が実行一致する。

---

関連: #20(match / Option を開く)、#21(エフェクト関数の事後条件)、#23(検証レベルを診断で報告)、#24(Agent Repair Protocol)
