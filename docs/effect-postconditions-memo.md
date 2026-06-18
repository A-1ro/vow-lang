# エフェクト関数の事後条件 — 言語拡張の比較メモ(#21 / v0.3+ 送り)

> #21 の二段構えのうち **中長期分**(言語拡張)の比較検討メモ。
> v0.2 では**短期分のみ**を実装した(純粋ヘルパー経由イディオムを spec/kei-spec-v0.2.md §4 に
> 正式化、`examples/contracts/borrow.kei` を check-clean 化)。本メモは言語拡張の
> **評価軸と v0.3+ への送り判断**を記録する。実装は設計合意後。

## 問題の再掲

「`borrowBook` を呼ぶと在庫が**ちょうど 1 減る**」を関数全体の `ensures` で表したい。だが:

1. 契約式は副作用禁止(将来の静的証明を壊さないため)→ DB を読めない。
2. `old()` は引数しか参照できない → 外部状態の「呼び出し前の値」を捉えられない。

結果、**「`kei check` が通った」と「在庫が 1 減ることが保証されている」が乖離する。**
短期イディオム(§4)は数量変換を純粋ヘルパーへ退避するが、「本体が必ずヘルパーを経由する」
接続は言語が強制しない(レビュー依存)。これを言語で担保するのが中長期の課題。

## 案1: エフェクト事後条件の「論理的読み取り」

契約内での外部状態参照を、副作用ではなく**論理的読み取り(純粋な観測)**として特別扱いする。

```text
func borrowBook(book: BookId) -> Result<Int, BorrowError>
  uses Database.Read, Database.Write
  ensures result.isOk implies Database.availableOf(book) == old(Database.availableOf(book)) - 1
```

- `Database.availableOf` は契約専用の**観測子(observer)**として宣言され、契約式の中でのみ、
  状態を変えない読み取りとして許される。`old(...)` を外部状態の観測子に拡張する。
- **要件:** 観測子が純粋(冪等・無副作用)であることの保証。`extern`(M11)に
  「観測子」種別を追加し、checker が「契約内で呼べるのは観測子だけ」を強制する必要がある。

## 案2: 検証用ゴースト変数 / モデル変数(Dafny / Verus 式)

検証専用の**ゴースト状態**を導入し、実行時には消えるがコンパイル時の推論には乗る変数で
外部状態をモデル化する。

```text
ghost var stock: Map<BookId, Int>          // 検証専用。実行時には存在しない
func borrowBook(book: BookId) -> Result<Int, BorrowError>
  modifies stock
  ensures result.isOk implies stock[book] == old(stock[book]) - 1
```

- `modifies` 節で変更対象のゴースト状態を宣言し、`ensures` でその遷移を書く。
- **要件:** ゴースト変数の言語化、`modifies` 節、ゴーストと実コードの対応(refinement)の検査。
  `Map` 等のコレクション(立場B / #25)とゴーストの量化(`forall`)が前提になりがち。

## 評価軸

| 軸 | 案1(論理的読み取り) | 案2(ゴースト変数) |
|---|---|---|
| **健全性** | 観測子の純粋性保証が要(破れると契約が嘘になる) | ゴースト/実コードの対応検査が要(refinement の健全性) |
| **実装コスト** | 中(extern に観測子種別 + 契約内呼び出し規則) | 大(ゴースト言語化 + modifies + コレクション + 量化) |
| **合意書原則との整合(§1 第一条)** | 高(契約の見た目が自然な ensures に近い) | 中(ghost/modifies が増え、承認時に読む量が増える) |
| **既存機構との連続性** | 高(M11 extern の自然な延長) | 低(新しい検証サブ言語に近い) |
| **段階性** | 小さく入れられる | コレクション(#25)成熟が前提で大きい |

## 判断: 案1 を採用した(v0.3 / M14 / #45)

- **案1(論理的読み取り)を採用・実装した。** HANDOFF と外部レビューが収斂した方向であり、M11 `extern` の
  延長として小さく入った(`extern query` 観測子 + 契約内 `old()` 拡張)。合意書原則との整合も高い。
  仕様は `spec/kei-spec-v0.2.md` §4.3、実装は `kei_syntax`(`extern query` パース)/ `kei_check`
  (query 純粋性 `KEI-E3005`・契約内は query のみ `KEI-E4004`)/ `kei_emit`(`old` 機構の延長)。
- **健全性の根は観測子の純粋性。** v0.3 では `extern` 宣言を**信頼する(trusted)**——`query` が `uses` を
  持てないことを `KEI-E3005` で強制し、宣言が「純粋」と言う以上それを信じる。宣言が嘘なら契約も嘘になる。
- **案2 は引き続き見送り。** #25(コレクション・立場B)と量化契約 `forall`/`exists` の成熟が前提。
  それらが揃う段階で再評価する(現状は依存が重く、スコープ発散リスクが高い)。

## 目標ケースの定義(将来の e2e/golden の達成基準)

将来 `borrowBook` の在庫不変条件を**言語で**表現・検証できたと言える基準を、現時点で固定する:

1. **表現:** `borrowBook` の **関数シグネチャ + 契約だけ**を読んで、「成功時に在庫がちょうど 1 減る」が
   読み取れる(本体を読まずに承認できる = 合意書原則)。純粋ヘルパーへの退避を**強制されない**。
2. **検査:** その契約が `kei check` で機械検証され、`kei check --json` の `contracts[].verification` が
   `runtime` 以上(理想は `static`)を報告する。「本体がヘルパーを経由する」接続を言語が担保する。
3. **反例で落ちる:** 本体を「在庫を 2 減らす」「減らし忘れる」に書き換えると、契約違反として
   検出される(現状の §4 イディオムでは本体の接続ミスは検出されない — そこが埋まること)。
4. **e2e:** 上記を満たす `.kei` がトランスパイル→実行で「ちょうど 1 減る」を示し、違反版が
   `KeiContractViolation`(または静的エラー)で落ちる golden を持つ。

この 4 点が、§4 の現実解と将来の言語拡張を分かつ受け入れ条件だった。**v0.3 / M14(案1)で 4 点とも達成
した:** 表現は `examples/contracts/borrow_direct.kei` の `borrowBook`(契約直書き)、検査は `kei check`
が通り `verification` は `runtime`、反例は `borrowBookOffByTwo`(2 減らす)が実行時に `KeiContractViolation`、
e2e は `tests/e2e/tests/borrow_direct.test.ts`。golden は `tests/golden/check/ok_contract_observer`
(ok)/ `err_effect_query_effects`(`KEI-E3005`)/ `err_contract_nonquery`(`KEI-E4004`)。
