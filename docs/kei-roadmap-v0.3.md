# Kei 開発ロードマップ v0.3 — /goal 契約書集

> 運用ルール: 各Milestoneは「人間が合意する契約」。
> 完了条件は必ず機械検証可能な形(テスト・コマンド出力)で書く。
> 評価モデルはトランスクリプトしか見えないため、/goal文には「検証コマンドの実行と結果表示」を含めること。
> 🤝 マークは /goal 投入前に人間との設計合意が必要な事項。
>
> 本ファイルは `docs/kei-roadmap-goals.md`(v0.1 の M0〜M7、提案中の M8: kei_lsp)・
> `docs/kei-roadmap-v0.2.md`(v0.2 の M10〜M13)の続編。
> **v0.3 のテーマは「射程を広げる(システム記述言語化)と、契約検証の本丸への到達」。**
> 出典はドッグフード実験(在庫管理・図書館貸出)と外部設計レビューで、すべて v0.3 ラベルの
> open issue(#25 / #45 / #26 / #44 / #35 / #24)に対応する。
> v0.1 が「曖昧さゼロの最小核」、v0.2 が「その核で露わになった穴(健全性・契約表現力)」を
> 塞いだのに対し、v0.3 は **核の外——複数エンティティ処理・契約の本丸(外部状態の事後条件)・
> 捏造不能なテスト・修正ループの帯域**——へ踏み出す。

## Milestone 番号について

`M9`(コレクション)は v0.1 ロードマップ期に **#25 の親 issue(epic)** として `kei-roadmap-goals.md`
に採番された経緯があり、README・spec(`kei-spec-v0.1.md` §10 / `kei-spec-v0.3-collections.md`)から
**「M9」として参照されている**ため、番号を維持したまま本ファイルへ移設する。v0.2 が `M10`〜`M13` を
消費したため、**新規 v0.3 マイルストーンは `M14` から続ける**(番号の飛びはこの経緯による)。

## Milestone 全体像と順序

| M | テーマ | issue | 優先度 | 主な改修クレート |
|---|---|---|---|---|
| **M9** | コレクション型 `List` 段階1(不変・opaque + コンビネータ最小集合) | #25 | medium | kei_syntax / kei_check / kei_emit |
| **M14** | エフェクト事後条件の言語拡張(案1: 論理的読み取り) | #45 | medium | kei_syntax / kei_check / spec |
| **M15** | 契約ベース PBT 生成(`kei test` generative)段階1+2 | #26 | medium | kei_check / kei_cli / spec |
| **M16** | `extern` 未宣言呼び出しの strict モード | #44 | low | kei_check / kei_cli / spec |
| **M17** | 定数恒偽契約(`requires false`)の静的検出 | #35 | low | kei_check |
| **M18** | Agent Repair Protocol — 構造化修正提案 | #24 | low | kei_check / diagnostic-schema |

順序の論拠:

- **M9(コレクション)を土台に先頭。** `List` は反復・集計・絞り込みを言語内へ取り込む基盤で、
  #25 立場B(システム記述言語化)の中核。#26(量化契約のテスト)・#45(案2 ゴースト変数)が
  将来この上に積まれるため、最優先(medium だが依存上の根)。
- **M14 → M15 は契約表現力の連続。** M14(#45)は #21 の**本丸**——`borrowBook` 自身の `ensures` に
  「在庫がちょうど 1 減る」を直接書く——で、v0.2 / M11 の `extern` を延長する。M15(#26)の生成テストは
  契約が強いほど強くなるため、契約の本丸(M14)の後に置くと PBT の射程が広がる。M15 は Kei が
  「検証層」として外部から最も分かりやすい価値(「契約を書けば、捏造不能な検証が湧く」)で、対外訴求の柱。
- **M16〜M18 は low。** M16(#44)は M11 `extern` の境界を「未宣言呼び出しの厳格化」で閉じる繰り越し分。
  M17(#35)は定数恒偽の静的検出で、健全な静的検証の最小形(M12 `verification_of` の延長)。
  M18(#24)は修正ループの帯域を上げる高度化で、issue の位置づけどおり**土台が固まった最後**に置く。

各 issue の依存(後続段階):

- **#25 段階2(量化契約 `forall`/`exists`)** は #23(M12 検証レベル報告、実装済み)が前提。M9 完了後に着手。
- **#45 案2(ゴースト/モデル変数)** は #25(コレクション・立場B)と量化契約の成熟が前提。本書では案1のみ。
- **#26 段階3(エフェクト関数の生成テスト)** はエフェクトハンドラ実装が前提、**段階4(量化契約のテスト)** は
  #25 段階2 が前提。本書(M15)は段階1+2(純粋関数 + シード注入)に絞る。

---

## M9: コレクション型 — `List` 段階導入(立場B / v0.3)【親 issue: #25】

> #25 で **立場B(システム記述言語を目指す)** を採択した。`List` を `Result` / `Option` と
> 同格の **第三の組み込みジェネリクス** として段階導入する。設計の正本は
> `spec/kei-spec-v0.3-collections.md`(Draft)、射程の宣言は `spec/kei-spec-v0.1.md` §10。
> **issue #25 は v0.3 以降のコレクション系の親 issue(epic)**。下記の段階1/2/3 を
> /goal 投入時に必要ならサブ issue へ分割する。**一度に全部入れない。**

### 段階の全体像

| 段階 | 内容 | Milestone | 前提 |
|---|---|---|---|
| **1** | `List<T>`(不変・opaque)+ 量化子なしのコンビネータ最小集合 | M9(本節) | 関数引数の機構合意(下記 🤝) |
| **2** | 量化契約 `forall` / `exists`(契約専用構文) | M9 以降 | #23 の検証レベル報告が動作(M12 実装済み) |
| **3** | `Map<K, V>` | 未定 | 段階2後に必要性を再評価 |

### 🤝 事前合意(段階1 / M9 着手前に確定する)

- **【最大の論点】コンビネータ引数に関数を渡す機構**。`map` / `filter` / `fold` / `all` / `any` は
  関数引数を取るが、v0.1 は「関数は値ではない(`KEI-E2001`)」。golden の `err_collection_*` が
  この衝突を記録している。**案2(名前付き関数参照をコンビネータ引数位置でのみ許す。第一級関数値は
  入れない)を既定線**とする(spec v0.3 §4.1)。案1(第一級関数値+ラムダ)に振るか、ここで確定する。
- **`List` リテラル構文**(`[1, 2, 3]` と空リスト `List<T>()` の要素型明示)。spec v0.3 §3.1。
- **コンビネータの最小集合の確定**: `length` / `isEmpty` / `get` / `map` / `filter` / `fold` / `all` / `any`
  の 8 つに絞る(spec v0.3 §4)。これ以外(`flatMap` / `zip` / `take` 等)は段階1に入れない。
- **量化子なしで書ける契約の範囲**: `xs.length` / `xs.isEmpty` / `xs.all(pred)` / `xs.any(pred)` の
  結果まで契約から参照可。束縛量化(`forall x in xs`)は段階1で入れない(spec v0.3 §5)。
- **`@kei/runtime` / emit の分担**: `get`(範囲外 `None`)を runtime ヘルパーにするか emit 展開にするか
  (spec v0.3 §9)。新規依存・API 追加は ARCHITECTURE.md に記録する。

### /goal(段階1 ドラフト — 🤝 合意確定後に投入)

```
/goal `List<T>` が第三の組み込みジェネリクスとして kei_syntax / kei_check / kei_emit に
入り、(1) tests/golden/check/ の err_collection_total_stock_value と
err_collection_plan_all_reorders が ok_ に移って check-clean になる(List 型解決 +
コンビネータ引数の関数参照を許可)、(2) コンビネータ 8 種(length/isEmpty/get/map/
filter/fold/all/any)の型・エフェクト検査が golden で固定される、(3) 量化子なしの
契約(xs.length / xs.isEmpty / xs.all(pred) / xs.any(pred) / result.length)が書ける
ことを golden で固定、(4) List を使う example が TS(readonly 配列+配列メソッド)へ
トランスパイルされ tsc --strict --noEmit でエラーゼロ、e2e で実行一致する。
spec/kei-spec-v0.3-collections.md の段階1記述と実装が一致(食い違ったら仕様を先に直す)。
cargo test --workspace 全件パス、clippy 警告ゼロ。結果を表示して完了とする。
```

### golden / test 設計方針

- **回帰ターゲットの移行**: 既設の `tests/golden/check/err_collection_*`(現状 `List` 未実装で `err_`)を
  段階1完了で `ok_` へ移す。これは **人間レビュー必須の golden 変更**(不変条件1)であり、段階1の受け入れ信号。
- コンビネータごとに ok / err(型不一致・エフェクト未宣言・契約での誤用)の golden を足す。
- example(在庫管理の `totalStockValue` / `planAllReorders`)を examples/ に追加し、check-clean を保つ。

### スコープ外(段階1 / M9)

- 量化契約 `forall` / `exists`(段階2。#23 の検証レベル報告が前提)
- `Map<K, V>`(段階3)
- ユーザー定義ジェネリクス(`record Box<T>` 等)の一般開放(`List` は特別扱いに留める)
- 命令ループ(`for` / `while`)・添字構文 `xs[i]`(反復はコンビネータ、要素アクセスは `get`)

---

## M14: エフェクト事後条件の言語拡張(#45 / 案1: 論理的読み取り)

> **問題(#45)**: #21 は二段構えで設計され、v0.2 / M13 で**短期分**(数量的契約を純粋ヘルパーへ
> 退避し本体が必ず経由するイディオムの spec 正式化)を完了した。しかし**本丸は未解決**——
> `borrowBook` 自身の `ensures` に「在庫がちょうど 1 減る」を**直接**書けない。純粋ヘルパー経由は
> 「言語の制約に構造を合わせた」回避策で、「本体が必ずヘルパーを経由する/読んだ値をそのまま
> 書き戻す」接続を言語が強制しない(レビュー依存)。「`kei check` が通った」と「在庫が 1 減る」の
> **乖離が残る**。
> **本命案(案1)**: 契約内での外部状態参照を、副作用でなく**純粋な観測(論理的読み取り)**として
> 特別扱いする。`ensures Database.availableOf(book) == old(Database.availableOf(book)) - 1`。
> M11 `extern` の延長として小さく入る(HANDOFF・外部レビューと収斂した第一候補)。
> **健全性の鍵は観測子の純粋性保証**。比較メモは `docs/effect-postconditions-memo.md`。

### 🤝 事前合意(着手前に確定する)

- **観測子(query)の宣言構文**: 契約から参照できる外部状態は「副作用のない観測子」に限る。
  `extern`(M11)を拡張し、読み取り専用の純粋観測子を宣言する構文を確定する
  (例: `extern query Database.availableOf(book: BookId) -> Int` のような **query 修飾**)。
  純粋性は `extern` 宣言を信頼する(`trusted`)か、検査で担保するか——健全性の根なので明示する。
- **`old()` の外部状態への拡張**: 現状 `old()` は引数のみ参照可。観測子に `old()` を効かせる意味論
  (関数進入時点での観測値を保持して事後と比較する)を定義する。実行時の「進入時スナップショット」を
  どの粒度で取るか(参照した観測子だけ評価して退避)。
- **本体との接続を言語が担保する形**: 案1 では `ensures` が直接外部状態を参照するため、検証は
  「関数進入時の観測値」と「退出時の観測値」を実行時に評価して比較する。純粋ヘルパー経由のような
  レビュー依存の接続を不要にできることを目標ケースで示す。
- **検証レベル(#23 / M12 との統合)**: 案1 はまず `runtime`(観測子を実行時に呼んで比較)で実装し、
  `contracts[].verification` が **`runtime` 以上**(理想は将来 `static`)を `kei check --json` で報告する。
- **副作用禁止規則との整合**: 契約式は副作用禁止(CLAUDE.md / spec)。観測子呼び出しが「論理的読み取り=
  純粋」として規則に反しないことを spec に明文化する。エフェクト付き関数を契約から呼ぶことは引き続き禁止。
- **スコープ**: 案2(ゴースト/モデル変数 + `modifies` 節、Dafny/Verus 式)は **入れない**。
  実装が重く、#25(コレクション)と量化(`forall`)の成熟が前提のため v0.3 以降で再評価する。

### /goal(ドラフト — 🤝 合意確定後に投入)

```
/goal #45 案1(エフェクト事後条件の論理的読み取り)を kei_syntax / kei_check に入れ、
言語処理は各クレートの責務内で完結する。(1) extern の純粋観測子(query)宣言と、契約内で
その観測子を old() 付きで参照する構文がパースされる、(2) borrowBook のシグネチャ + 契約だけで
「成功時に在庫がちょうど 1 減る」が読み取れる(純粋ヘルパーへの退避を強制されない)ことを
example + golden で固定する、(3) その契約が kei check で機械検証され、kei check --json の
contracts[].verification が runtime 以上を報告する(本体がヘルパーを経由する接続を言語が担保)、
(4) 反例(在庫を 2 減らす / 減らし忘れる版)が契約違反として検出される——静的に落ちるか、
e2e 実行で KeiContractViolation になる——ことを golden / e2e で固定、(5) 観測子は副作用禁止
規則に反しない純粋観測として spec(kei-spec-v0.2.md §4 か v0.3 章)に明文化し、
docs/effect-postconditions-memo.md の「案1 を採用」を反映する。cargo test --workspace 全件パス、
clippy 警告ゼロ。最後にテスト結果サマリと、契約だけで在庫不変条件が読める check-clean 例を表示して完了とする。
```

### golden / test 設計方針

- `tests/golden/syntax/`: `extern query` 宣言と契約内観測子参照(`old(Database.availableOf(book))`)の AST。
- `tests/golden/check/`: 観測子事後条件の ok(ちょうど 1 減る)/ err(2 減る・減らし忘れ・観測子の純粋性違反)。
- `examples/contracts/`: M13 の `borrow.kei`(純粋ヘルパー経由)に対し、**契約直書き版**を追加して check-clean を保つ。
- `tests/e2e/`: 正常版(在庫がちょうど 1 減る)と違反版(`KeiContractViolation`)の実行一致。

### スコープ外(M14)

- 案2(ゴースト/モデル変数・`modifies` 節)。#25 コレクション + 量化の成熟後に再評価。
- SMT による外部状態の静的証明(`verification` の `static` 引き上げは将来)。
- 集合に対する事後条件(`forall` での要素単位の不変条件)。#25 段階2 が前提。

---

## M15: 契約ベース PBT 生成 — `kei test` generative(#26 / 段階1+2)

> **着想(#26)**: Kei の契約は既にテストの二大要素を内包する——`requires` = 入力の生成制約、
> `ensures` = テストオラクル。テストの本質的コストは「期待値を誰が決めるか(オラクル問題)」だが、
> Kei は契約が一級市民なので**オラクルが既に書かれている**。契約を読んで property-based test を
> 生成・実行できる。
> **中核原則(捏造不能性)**: シードおよびテストは「入力」のみを供給し、「期待値」を持たない。
> オラクルは契約(`ensures`)のみが担う。AI がテストを通す唯一の方法を「実装を契約に合わせる」ことだけに
> 限定し、「テストを通すために期待値を歪める」捏造経路を**言語構造から排除**する。これは第一条
> (人間は契約を承認、AI は実装)の権限分離をテストドメインへ拡張したもの。

### 既存 `kei test`(M7)との関係

現状の `kei test`(M7)は **dev ビルド後にプロジェクトの `npm test` へ委譲する薄いラッパー**で、
ランナーの知識を持たない。本 Milestone の「契約から PBT を生成する」のは**別レイヤ**であり、
既存の委譲経路を壊さないサブコマンド/フラグの形を 🤝 で確定する(下記)。

### 段階の全体像(#26 の段階設計)

| 段階 | 内容 | Milestone | 前提 |
|---|---|---|---|
| **1** | 純粋関数の単純な契約から PBT 生成・実行(反例最小化) | M15(本節) | — |
| **2** | シード注入モード(入力のみのシードファイル、requires 適合検証) | M15(本節) | — |
| **3** | エフェクト関数の生成テスト(シードの `given` で初期外部状態) | 未定 | エフェクトハンドラ実装 |
| **4** | 量化契約のテスト(集合に対するプロパティ) | 未定 | #25 段階2(`forall`/`exists`) |

### 🤝 事前合意(段階1+2 着手前に確定する)

- **設計原則の格上げ**: 「テストのオラクルは契約(`ensures`)のみ、シードは入力のみ(捏造不能性)」を
  spec / HANDOFF に**正式原則として明記**する。テストが書けない=契約が不足のサインであり、契約強化で
  解決する、という方向も併記する。
- **サブコマンド/フラグの形**: 既存 `kei test`(npm 委譲)と衝突しない経路を確定する。候補は
  `kei test --generative` フラグ / 別サブコマンド。どちらでも「言語処理を kei_cli に持ち込まない」
  (CLAUDE.md)を守り、生成・判定ロジックは kei_check 側に置く。
- **入力ジェネレータの範囲**: `Int` / `Bool` / `String` / `record` の生成と、**単純な `requires`**
  (`available > 0` 等の範囲制約)を満たす入力の供給方法を確定する。**制約ソルバーは言語に持ち込まない**
  ——複雑な `requires`(引数間の関係・将来の量化子)は段階2のシード注入で回避する。
- **反例の最小化(shrinking)**: `ensures` 違反を**最小化された入力**付きで報告する方針を決める。
- **シードファイル文法(段階2)**: 入力のみを持ち、**期待値を構文上持てない**ことを保証する文法を定義する
  (#26 の `seeds for <fn> { case "..." { input: {...} given: {...} } }` イメージ)。`given`(初期外部状態)は
  段階3(エフェクト関数)で生きる枠で、段階2では構文として定義し検証は入力部に絞る。
- **シードの検証**: シードが供給する入力は対象関数の `requires` を満たさねばならない。`kei check` が
  シードを `requires` に照らし、無効なシード(`requires from != to` に対する `from == to` 等)を弾く。
- **検証レベル(#23 / M12)との統合**: 生成テストの結果を **`generative`** レベルとして診断
  (`kei check --json` / テスト出力)に載せる。static で証明できないものは generative、それも難しければ
  runtime、という連続的扱いの「generative」の実体を本 Milestone が与える。

### /goal(段階1+2 ドラフト — 🤝 合意確定後に投入)

```
/goal 契約から property-based test を生成・実行する経路を kei_check / kei_cli に入れ、生成・判定
ロジックは kei_check 側に置く(kei_cli は委譲のみ)。(1) 純粋関数の requires から入力を生成し、
全入力で ensures が成り立つかを検証する。decrementAvailable 相当(requires available > 0,
ensures result == old(available) - 1)が対象になる、(2) ensures 違反を最小化された反例入力付きで
報告する(わざと壊した実装で反例が出ることを golden / 統合テストで固定)、(3) シードファイル文法を
定義し、入力のみを持ち期待値を構文上持てないことをパーサで保証する、(4) シードが対象関数の requires を
満たすかを kei check が検証し、requires 違反シードを Diagnostic(span・code・fix 候補付き)で弾く、
(5) 生成テストの結果が検証レベル generative として診断に載る。設計原則「オラクルは契約のみ、
シードは入力のみ(捏造不能性)」を spec / HANDOFF に明記する。cargo test --workspace 全件パス、
clippy 警告ゼロ。最後にテスト結果サマリと、契約から生成された PBT が通る例・反例が出る例を表示して完了とする。
```

### golden / test 設計方針

- `tests/golden/check/`: シードの requires 適合(ok / requires 違反シードを弾く err)、期待値を持つシードの構文エラー。
- 生成テストの統合テスト: 正しい実装で全 pass、わざと壊した実装(`available - 2` 等)で最小化反例を報告。
- `examples/`: 契約から PBT が湧く最小例(`decrementAvailable`)を check-clean で固定。
- 検証レベル `generative` が `kei check --json` の `CheckReport` / テスト出力に載ることを golden で固定。

### スコープ外(M15 = 段階1+2)

- 段階3: エフェクト関数の生成テスト(エフェクトハンドラ + シードの `given` による初期外部状態)。エフェクトハンドラ実装が前提。
- 段階4: 量化契約のテスト(`forall`/`exists`)。#25 段階2 が前提。
- 制約ソルバー(SMT)による複雑 `requires` の入力自動生成(シード注入で回避)。

---

## M16: `extern` 未宣言呼び出しの strict モード(#44 / #22 フォローアップ)

> **問題(#44)**: M11 で `extern` 署名(外部関数の戻り型・エフェクト宣言)を実装し、宣言された境界は
> 検証下に入った(境界越しの `KEI-E3001`)。しかし移行は **opt-in 段階移行**として実装したため、
> **`extern` を宣言していない外部 namespace 呼び出しは従来どおり opaque で通る**。つまり #22 の元の
> 再現コード(`extern` を書かずに `Time.now()` を呼び `uses Clock` を忘れる)は**まだ検出されない**。
> 完全な「境界の合意書化」には、未宣言の外部呼び出しを警告/エラーにする厳格モードが要る。
> **なぜ v0.2 で見送ったか**: 一律 warning/error 化は flag-day 移行になり、外部 namespace 呼び出しを
> 別目的で多用する既設 `ok_*` 検査群が一斉に赤くなる。HANDOFF「スコープ発散が第二の死因」に従い、
> まず enforcement-when-declared を確立した(`spec/kei-spec-v0.2.md` §2.3)。

### 🤝 事前合意(着手前に確定する)

- **strict モードの導入方法**: 未宣言の外部 namespace 呼び出しを検出する仕組みを確定する。候補は
  `kei check --strict-extern` フラグ / プロジェクト設定 / モジュール単位のオプトイン宣言。
  **一括 error はせず**、段階は (a) warning →(成熟後)(b) error とする。
- **既存 `ok_*` の移行パス**: 外部 namespace 呼び出しを別目的(名前解決・エフェクト階層・契約純粋性)で
  使う既設 golden を壊さない段階導入を設計する(strict はオプトインで、既定の `kei check` は従来挙動)。
- **標準 prelude の `extern`**: `Time` / `Database` / `Audit` 等の組み込み `extern` を配布して未宣言を
  減らすか併せて検討する(配るなら prelude の置き場・読み込み経路を決める)。
- **新規 Diagnostic コード**: 未宣言呼び出しの warning/error コードを採番(M0 の `KEI-E[カテゴリ1桁][連番3桁]`
  規約に従う。境界=エフェクト系カテゴリ3 が素直)。span・code・fix 候補(`extern` 宣言の追加提案)を持たせる。

### /goal(ドラフト — 🤝 合意確定後に投入)

```
/goal extern 未宣言の外部 namespace 呼び出しを検出する strict モードを kei_check / kei_cli に入れる。
(1) strict モードの導入機構(フラグ/設定/宣言のいずれか合意した形)が定義され、既定の kei check は
従来挙動のまま、strict 時のみ未宣言呼び出しを検出する、(2) #22 の元の再現コード(extern なしで
Time.now() を呼び uses Clock 宣言漏れ)が strict モードで落ちることを golden で確認する。検出は
span・code・最低1つの fix 候補(extern 宣言追加)を持つ、(3) 既存 tests/golden/check/ok_* 検査群を
壊さない移行パス(strict はオプトイン)を golden で示す、(4) spec/kei-spec-v0.2.md §2.3 の将来欄を
strict モードの仕様で更新する。cargo test --workspace 全件パス、clippy 警告ゼロ。最後にテスト結果
サマリと、strict で #22 再現コードが落ち・既定では通る対比を表示して完了とする。
```

### golden / test 設計方針

- `tests/golden/check/`: strict 時に #22 再現コードが落ちる err、既定では同じコードが通る ok の対。
- 既設 `ok_*` 群が既定モードで不変(strict オプトインで初めて指摘される)ことを回帰で固定。
- spec `kei-spec-v0.2.md` §2.3 の将来欄が実装と一致(食い違ったら仕様を先に直す)。

### スコープ外(M16)

- 未宣言呼び出しの **一括 error 化(flag-day)**。段階導入(warning → 後で error)に留める。
- prelude `extern` の網羅整備(配るかの判断と最小セットに留め、全外部 API の署名化はしない)。

---

## M17: 定数恒偽契約(`requires false`)の静的検出(#35)

> **問題(#35)**: PR #31(M12)の検証レベル判定 `verification_of`(`crates/kei_check/src/check.rs`)は、
> 定数畳み込みで `true` になる契約のみ `static`、それ以外は `runtime`。`requires false` / `requires 1 > 2`
> のような **定数恒偽** の契約も `runtime` 扱いになり、実行時に必ず違反する。コンパイル時に「常に偽」と
> 分かるなら、静的に診断できる余地がある。
> **提案**: `const_eval` が `Some(Bool(false))` に畳める契約を、静的エラー/警告(新規コード)として
> 診断する。健全な静的検証の最小形(定数恒偽は処理系が反証済み)。

### 🤝 事前合意(着手前に確定する)

- **警告かエラーか**: `requires false`(到達不能な関数=呼べば必ず違反)はエラー寄り、`ensures false`
  (実装が必ず違反)も対象にするか。診断の severity を確定する。
- **新規 Diagnostic コード**: 定数恒偽の新規コードを採番(検証=カテゴリ系。M0 規約に従う)。
  span・code・fix 候補(契約の修正提案)を持たせる。
- **`verification_of` との接続**: 定数恒偽は「処理系が反証済み」なので、`runtime` ではなく**静的に確定**。
  M12 の `verification_of` 判定にこの分岐を足す形を決める(true → `static` / false → 静的診断 / その他 → `runtime`)。
- **スコープ**: SMT 連携は入れない。`const_eval` が定数に畳める範囲(リテラル比較・定数論理式)に限る。

### /goal(ドラフト — 🤝 合意確定後に投入)

```
/goal 定数恒偽の契約を静的に検出する分岐を kei_check の verification_of / 診断に入れる。
(1) const_eval が Some(Bool(false)) に畳める契約(requires false / requires 1 > 2 等)を新規
Diagnostic(span・code・最低1つの fix 候補)で静的に検出する、(2) 検出が true 畳み込み(static)・
定数恒偽(新規診断)・その他(runtime)の三分岐として verification_of に入り、既存の検証レベル判定を
壊さない、(3) ok(畳めない通常契約は従来どおり)/ err(定数恒偽)を golden で固定する。
cargo test --workspace 全件パス、clippy 警告ゼロ。最後にテスト結果サマリと、定数恒偽が静的に
落ちる例を表示して完了とする。
```

### golden / test 設計方針

- `tests/golden/check/`: `requires false` / `requires 1 > 2` が静的に落ちる err、畳めない通常契約が従来どおり通る ok。
- `verification_of` の三分岐(static / 定数恒偽診断 / runtime)の単体テスト。
- 既存の検証レベル golden(M12)が不変であることを回帰で固定。

### スコープ外(M17)

- SMT / 記号実行による一般の契約不能性検出(定数畳み込みで届く範囲のみ)。
- 恒真契約の冗長性警告(本 Milestone は恒偽に限る)。

---

## M18: Agent Repair Protocol — 構造化修正提案(#24)

> **背景(#24)**: 外部設計レビューで「Kei の価値は構文より『生成 → 検証 → 修正』のループ帯域にある。
> フォーマッタと JSON エラーだけでは弱く、AST diff / contract diff を公式出力にすべき。AI が
> 『エラー文を読んで勘で直す』のではなく、コンパイラが提示した構造化差分を適用する形にする」との提案。
> 現行 `Diagnostic.fixes`(テキスト編集候補)の **意味論的強化版**——契約レベル・AST レベルの構造化された
> 修正提案——にあたる。
> **位置づけ**: 方向は正しいが**土台の後**。表現力の穴(#20: Option を開く / M10 済)や契約の射程
> (#21 / M13・M14)が埋まり `Diagnostic.fixes` が充実した延長線上に進める高度化。issue の優先度どおり
> v0.3 の最後に置く。

### 🤝 事前合意(着手前に確定する)

- **構造化修正提案のスキーマ**: `ContractMissing` / `ContractWeak`(契約が弱い)等、修正提案を伴う
  診断種別と `suggested_contract` のスキーマを `spec/diagnostic-schema.md` に定義する。
  例: `{ "kind": "ContractMissing", "function": "borrowBook", "suggested_contract": { "ensures": "..." } }`。
- **現行 `fixes` との後方互換**: テキスト編集候補の `fixes` を壊さず、構造化提案を**追加フィールド**として
  載せる(既存の Diagnostic 消費側を壊さない)。
- **着手する診断種別**: 最低 1 種(`ContractMissing` 等)から始める。どの検査でどう `suggested_contract` を
  生成するか(契約の不足をどう検出し提案文を組むか)を確定する。
- **AST diff / contract diff の出力形式**: `kei check` の公式出力(`--json`)に加える差分フォーマットを決める。
- **適用 → 再検証プロトコル**: エージェントが「提案された構造化差分を適用 → 再検証」できる経路を spec 化する。

### /goal(ドラフト — 🤝 合意確定後に投入)

```
/goal 構造化修正提案(Agent Repair Protocol)を kei_check の Diagnostic / spec に入れる。
(1) 構造化修正提案のスキーマ(suggested_contract を含む診断種別)が spec/diagnostic-schema.md に
定義される、(2) 最低1種の診断(ContractMissing 等)が suggested_contract を返し、現行
Diagnostic.fixes と後方互換である(既存の fixes 消費が壊れない)、(3) エージェントが提案を適用 →
再検証してエラーが減ることを統合テストで確認する。cargo test --workspace 全件パス、clippy 警告ゼロ。
最後にテスト結果サマリと、ContractMissing → 提案適用 → 再検証でエラーが減る一連を表示して完了とする。
```

### golden / test 設計方針

- `spec/diagnostic-schema.md`: 構造化修正提案スキーマの定義(人間レビュー必須の心臓部=不変条件2)。
- `tests/golden/check/`: `ContractMissing` 等が `suggested_contract` を返す診断 JSON。
- 統合テスト: 提案適用 → 再検証でエラーが減る一連(適用前 N 件 → 適用後 < N 件)。
- 現行 `fixes` を消費する既存テストが不変であることを回帰で固定。

### スコープ外(M18)

- 提案の自動適用(コンパイラが書き換える)。提案の**出力**に留め、適用はエージェント/ツール側。
- 全診断種別への `suggested_contract` 展開(最低 1 種から始め、後続で拡張)。

---

## v0.3 全体運用メモ

- v0.3 は **射程の拡張(#25 コレクション=システム記述言語化)と契約検証の本丸**
  (#45 外部状態の事後条件 / #26 捏造不能な生成テスト)に集約され、#44 / #35 / #24 が境界・静的検証・
  修正ループの仕上げを担う。すべての出典はドッグフード実験と外部レビュー——**実際に詰まった証拠**。
- 依存と順序: **M9(コレクション)が土台**で先頭。**M14(事後条件・本丸)→ M15(生成テスト)** は
  契約表現力の連続(契約が強いほど PBT が強くなる)。**M16 / M17 / M18 は low** で、M11 境界の仕上げ・
  静的検証の最小形・修正ループの高度化。
- **後続段階の前提を守る**: #25 段階2(量化 `forall`/`exists`)・#45 案2(ゴースト変数)・#26 段階3-4
  (エフェクト関数 / 量化のテスト)は本書のスコープ外で、土台(M9 / エフェクトハンドラ / #23)成熟後に
  サブ issue 化する。**一度に全部入れない**(HANDOFF「スコープ発散が第二の死因」)。
- **golden / スキーマ変更は人間レビュー必須**: M9 の `err_collection_* → ok_` 移行、M18 の
  `diagnostic-schema.md` 拡張は M0 で心臓部とされた領域(不変条件1・2)。expected を実装都合で書き換えない。
- 各 /goal 投入前に対象 Milestone の golden ケースを人間がレビューする。**golden test こそが契約本文**。
- Milestone 完了ごとに `kei fmt` を全コードベースに適用し、spec との乖離があれば **spec を先に直す**
  (仕様が常に source of truth)。
