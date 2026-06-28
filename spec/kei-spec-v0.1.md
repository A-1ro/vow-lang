# Kei 言語仕様書 v0.1 (Draft)

> Code is a kei between humans and AI.
> コードは人間とAIの合意書である。

## 0. ポジショニング

Keiは「AIが書き、人間が承認し、コンパイラが履行を保証する」ことを前提に設計されたプログラミング言語。
人間の書きやすさを捨て、**検証可能性・推論の局所性・エージェントループとの親和性**に全振りする。

- ターゲットランタイム: TypeScriptへのトランスパイル(V8 / Cloudflare Workers / Node)
- ツールチェイン: `kei` CLI + Kei MCP Server を言語仕様と同格の一級市民とする

## 1. 設計原則

1. **合意書原則** — 関数シグネチャ+契約(uses/requires/ensures)だけ読めば、人間はbodyを読まずに承認判断できる。レビューの単位は実装ではなく契約。
2. **曖昧さゼロ** — 暗黙の型変換なし、暗黙のimportなし、演算子オーバーロードなし。トークン数より推論コストを優先する。
3. **推論の局所性** — 1ファイルの挙動は、そのファイルと明示importの宣言だけで決定できる。再エクスポート禁止。グローバル可変状態は存在しない。
4. **正規形唯一** — 同じASTは常に同じテキストに整形される(`kei fmt`)。スタイル論争は仕様で殺す。diffは常に意味的差分。
5. **エラーは構造化データ** — コンパイラ診断はJSONが正、散文は派生。生成→検証→修正ループの帯域を最大化する。
6. **null不在** — `Option<T>` / `Result<T, E>` のみ。例外機構なし。失敗は型に現れる。

## 2. 構文スケッチ

### 2.1 関数と契約

```kei
func transferFunds(from: AccountId, to: AccountId, amount: Money)
  -> Result<TransferReceipt, TransferError>
  uses Database.Write, Audit.Log
  requires amount > Money.zero
  requires from != to
  ensures result.isOk implies balanceOf(from) == old(balanceOf(from)) - amount
{
  let sender = Database.fetchAccount(from) else fail TransferError.NotFound(from)

  if sender.balance < amount {
    return Err(TransferError.InsufficientFunds { needed: amount, had: sender.balance })
  }

  Database.debit(from, amount)
  Database.credit(to, amount)
  Audit.Log.record(Transfer { from, to, amount })

  return Ok(TransferReceipt { from, to, amount })
}
```

- `uses` — エフェクト宣言(権限条項)。宣言外のエフェクト使用はコンパイルエラー。
- `requires` — 事前条件。呼び出し側の義務。
- `ensures` — 事後条件。実装側の義務。`old(expr)` で呼び出し前の値を参照できる。
- `else fail` — Option/Resultの早期脱出を構文で明示。

### 2.2 型定義

```kei
type AccountId = String tagged "AccountId"   // 幽霊型タグ。String同士でも混同不可

record TransferReceipt {
  from: AccountId
  to: AccountId
  amount: Money
}

enum TransferError {
  NotFound(AccountId)
  InsufficientFunds { needed: Money, had: Money }
}
```

### 2.3 モジュール

```kei
module payments.transfer

import core.money { Money }
import infra.database as Database
```

- importは全て明示。ワイルドカードなし。再エクスポートなし。
- モジュールパスはファイルパスと1:1対応。
- **import 境界の型解決(v0.4 / M20)**: `kei check <file>` は `module a.b.c` 宣言と入力ファイルのパスから project root を逆算し(親を `path` の段数だけ遡る)、`import a.b { X }` を `<root>/a/b.kei` まで解決する。対象モジュールが見つかれば、import した record / enum / type alias は通常のローカル型と同じ検査経路に乗り、フィールド名タイプミスは `KEI-E2002`、フィールド型誤用は `KEI-E2001`、enum match の非網羅は `KEI-E2007` で検出される。解決できない import(ファイル不在 / パース失敗 / 循環)は従来通り **opaque**(`Ty::Unknown`)として扱い、検査をブロックしない。namespace 別名 import(`import x.y as N`)は将来拡張のため M20 でも opaque のまま据え置く。
- **List リテラルと tagged 明示構築(v0.4 / M22)**: `[a, b, c]` で `List<T>` を直接構築できる(`T` は要素から推論。空 `[]` は文脈の型注釈から決まる)。`type Id = Base tagged "Id"` で導入した tagged 型は **同名コンストラクタ呼び出し** `Id(value)` で明示構築でき、`value` の型は `Base` と互換であること(不一致は `KEI-E2001`)。素の `Base → tagged` 代入は引き続き `KEI-E2005` でブロックする(構築点を常に明示する規律を保つ)。`enum`・`record` のコンストラクタはそれぞれ専用構文(`E.V(...)` / `R { ... }`)を持ち、tagged だけが「関数呼び出しの形での値構築」を持つ例外。

### 2.4 数値型と金額表現(v0.4 / #61)

v0.1 の組み込み数値型は **`Int`(i64 相当)のみ**。浮動小数・固定小数点は持たない。金額・評価額・税率の按分などはすべて `Int` で表現する。

- **金額は最小通貨単位の `Int` で持つ** ことを規約とする(円なら銭でなく円、ドルならセント)。境界(ホスト TS / 表示)で必要なら換算する。
- **`Money` / `core.money` は仕様上の架空型・架空モジュール**。`§2.1` `§2.2` `§2.3` の例で `Money` `AccountId` `core.money` が登場するが、これらは「合意書としての契約」を読みやすくするための**説明用**であり、stdlib に **実装されていない**。実プロジェクトでは次のいずれかを採る:
  - `Int`(最小通貨単位)をそのまま使う(最小コスト・推奨)。
  - 自プロジェクトの `core/money.kei` 等で `type Money = Int tagged "Money"` を自前定義する(`§2.3` の M22 構築規則に従い、`Money(0)` で値を作る。`Money.zero` のような静的メンバアクセスは Kei 構文に**無い**)。
- **固定小数点(`Decimal`)・stdlib `core.money` の実在化**は v0.5 以降で別途検討する(本仕様の射程外)。丸め規約・等価性・契約での扱いを設計する必要があり、コレクション(`§10`)とは独立に進められる。

`examples/contracts/withdraw.kei` と `examples/effects/transfer.kei` は架空 `Money` 例として残しており、e2e は `tests/e2e/stubs/core/money.ts` の差し替えで動かす。実装プロジェクトでこの形を踏襲しないこと。

### 2.5 コンビネータ引数位置限定ラムダ(v0.4 / M25 / #59)

`List<T>` のコンビネータ(`map` / `filter` / `fold` / `all` / `any`)の引数位置に限り、その場で **純粋ラムダ** `p => expr` / `(a, b) => expr` を書ける。「一度しか使わない述語/射影をその場で読める形にして、`requires` を読むだけで不変条件が把握できる」状態を目指す射(合意書原則の強化)。

```kei
func totalStockValue(products: List<Product>) -> Int
  requires products.all(p => p.quantity >= 0)
  requires products.all(p => p.unitPrice >= 0)
  ensures result >= 0
{
  return products.fold(0, (acc, p) => acc + p.quantity * p.unitPrice)
}
```

合意条件:

- **構文**: `p => expr`(単項)/ `(a, b) => expr`(複数)。**body は単一式**。`{ ... }` ブロック禁止(M25 段階の射程)。**0 引数 `() => expr` は不許可**(`KEI-E0101`、F0)— キャプチャ禁止 + 純粋限定下では定数式しか書けず、コンビネータの arity も 0 を期待しないため、構文段階で弾く。
- **位置**: List コンビネータの引数位置のみ。`let f = (ラムダ)` / `return ラムダ` / 任意の非コンビネータ引数位置のラムダは `KEI-E2001`(関数は値ではない)を維持。受信側が `List<T>` でないコンビネータ風呼び出し(`cart.map(...)`)は専用診断「`map` is a List<T> combinator」を出す(F7)。
- **キャプチャ**: 禁止。ラムダ body 内で参照できるのは **ラムダパラメータ + トップレベル関数 + import** のみ。外側関数の `let` / parameter 参照は `KEI-E2001`(明示的に「キャプチャ不可」と診断)。特に `result`(ensures の特殊束縛)も lambda 内からは見えない(F1 / 「`result` is not accessible from lambda bodies」)。
- **エフェクト**: 純粋限定。ラムダ body 内で `uses` 付き関数を呼んだら `KEI-E3001`(外側関数の `uses` 包含があっても許さない。契約式と同じ純粋スコープ)。契約の中の lambda が effectful 呼び出しを含む場合は `KEI-E3001` + `KEI-E4001`(契約純粋性)の両方が出る(F6 / 二重診断は意図的)。
- **`old(...)` は lambda body 内で一律禁止**(N3 / [0] / `KEI-E4002`、ensures モード限定)。`old` は関数入口で 1 回評価される(emit が `kei$old$N` に bind する)のに対し、lambda body は呼び出しごとに評価される — 時相が根本的に整合しない。引数が lambda param を参照するか否かに関わらず、`old(p.qty)` も `old(Database.maxLimit())` も等しく違反。したがって `xs.all(p => p < old(maxLimit()))` のような契約は書けない。代替: lambda body をトップレベル関数に切り出して `old(...)` 値を引数で渡すか、契約から `old` を取り除いて別の不変条件で表現する。emit 側は二段防御として `collect_old_exprs` を lambda 境界で停止する。
- **TS 予約語**: lambda パラメータ名が TS 予約語(`class`, `var`, `null`, `this`, `function`, `delete`, `typeof`, `let`, `await`, `async` 等)と衝突したら `KEI-E2001`([4])。Kei 自体は予約していないが、emit 後の `(class) => ...` は `tsc` が parse 不能になるため check 段階で弾く。検出単位は v0.4 では lambda パラメータのみ(将来 let / 関数パラメータ全般に拡張可)。
- **0 引数禁止**: `() => expr` は構文段階で `KEI-E0101` ([6])。parser が `Expr::Error` sentinel を返し、下流 walker は no-op で扱う。
- **第一級関数値ではない**: ラムダは「コンビネータ引数位置の構文糖」であり、値として保存・再利用はできない(M9 / spec §10 「案 2: 第一級関数値を導入しない」を維持)。
- **ネスト可能**: `xss.fold(0, (acc, xs) => acc + xs.fold(0, (a, x) => a + x))` のように内側のコンビネータ引数位置に再度 lambda を書ける。キャプチャ禁止・純粋限定は外側にも一段ずつ独立に効く(内側 lambda から外側 lambda の param を参照することはできない)。

合意書原則への影響: 述語/射影が `requires` の上に直書きされることで、`products.all(p => p.quantity >= 0)` のように **その関数が前提とする不変条件** をその場で読める。トップレベルに `hasNonNegativeQuantity` のような使い捨て関数を散布する必要が無くなる(命名の汚染を避けつつ、契約は依然として静的に解析可能)。

## 3. エフェクトシステム(v0.1の範囲)

### 3.1 意味論

- エフェクトは**ケーパビリティ**として扱う。関数は `uses` に列挙したエフェクトのみ行使できる。
- エフェクトは推移的に伝播する。`f` が `g`(uses Database.Write)を呼ぶなら、`f` も `Database.Write` を宣言しなければならない。
- `uses` なしの関数は**純粋**。コンパイラはメモ化・並べ替え・テスト時のプロパティ検査を自由に行える。

### 3.2 標準エフェクト階層(初期セット)

```
IO
├── Network.{Read, Write}
├── File.{Read, Write}
├── Database.{Read, Write}
├── Clock          // 現在時刻の取得
├── Random         // 乱数
└── Audit.Log
```

- エフェクトは階層を持つ。`uses IO` は全IOの包括許可(雑だが合法)。細かいほど合意書として価値が高い。
- ユーザー定義エフェクトを `effect` 宣言で追加できる(v0.2で詳細化)。

### 3.3 エフェクトハンドラ(将来構想・v0.1では実装しない)

テスト時に `Database.Write` をインメモリ実装に差し替える仕組み。v0.1ではDI的なモジュール差し替えで代替。

## 4. 契約の実行モデル

- v0.1では `requires` / `ensures` は**実行時アサーション**としてトランスパイルされる。
  - dev/testビルド: 全契約をチェック。違反は構造化エラーで即死。
  - releaseビルド: `requires` のみ残す(公開APIの防御)。`ensures` は除去可能。
- 静的証明(SMTソルバ連携)はv1.0以降のロードマップ。設計上、契約式は副作用禁止・全関数呼び出しは純粋関数のみ、という制約を最初から課しておく(将来の証明可能性を壊さないため)。

## 5. トランスパイル戦略

| Kei | TypeScript |
|---|---|
| `record` | readonly object型 + 同名ファクトリ関数 |
| `enum` | `kind` 判別のtagged union + 同名コンストラクタ集(tupleペイロードは `values`、名前付きフィールドは `fields`) |
| `Result/Option` | 専用ランタイム(`@kei/runtime`)。内部判別子 `ok` を共有し、`else fail` は両者を同じ形で分岐できる |
| `uses` | 型レベル検査はKei側で完結。TS出力にはdocコメントとして残す |
| `requires/ensures` | アサーション挿入。違反は構造化エラー `KeiContractViolation`(clause/func/condition/file/line/col)を送出。`ensures` は本体をIIFEに包み戻り値(`result`)を検査、`old(expr)` は関数先頭でキャプチャ |
| tagged型 | branded type(`__keiTag`)+ 同名コンストラクタ関数 |
| `module` / `import` | モジュールパスとファイルパスの1:1対応のまま相対import(`import { X } from "../core/money"` / `import * as Database from "../infra/database"`) |
| `Int` の除算 | `Math.trunc(a / b)`(0方向への切り捨て) |
| `Int` の剰余 | `a - Math.trunc(a / b) * b`(`/` と同じ 0 方向の商で定義) |
| `implies` | `!(a) \|\| b` |

### 5.1 演算子

| 優先順位(強→弱) | 演算子 | 結合 | 型 |
|---|---|---|---|
| postfix | `.` / `()` | 左 | フィールドアクセス・呼び出し |
| unary | `-x` / `!x` | 右 | `-`: `Int`、`!`: `Bool` |
| multiplicative | `*` / `/` / `%` | 左 | `Int` |
| additive | `+` / `-` | 左 | `Int` |
| comparison | `==` / `!=` / `<` / `>` / `<=` / `>=` | 左 | 比較結果は `Bool` |
| logical or | `\|\|` | 左 | `Bool` |
| implication | `implies` | 右 | `Bool` |

`&&` は v0.4 では導入しない。「かつ」は `requires` を複数行に分けるか、`if` で構造化する。

- **非同期の扱い(M4で決着)**: v0.1の出力は全関数同期。非同期は v0.2 以降で `uses Async` としてエフェクト統合を再検討する(§9参照)。
- 出力TSは人間が読める品質を保つ(デバッグ時のsource of truthはKeiだが、スタックトレースの追跡可能性を確保)。
- source map(v3)対応。`sources` は .kei ファイル、`sourcesContent` 埋め込み。契約違反のエラー位置は .kei 側の行番号に解決される。

## 6. Kei MCP Server

エージェントが新言語を「学習なしで」書けるようにするための取扱説明書サーバー。

### 6.1 ツール定義

| tool | input | output | 役割 |
|---|---|---|---|
| `kei_spec` | topic: string | 構造化された仕様セクション | 文法・標準ライブラリの即引きリファレンス |
| `kei_check` | source: string | Diagnostic[] (JSON) | 構文+型+エフェクト+契約の静的検査。修正候補つき |
| `kei_fmt` | source: string | 正規形source | 整形。エージェントは生成後必ず通す |
| `kei_examples` | query: string | コード例[] | イディオム検索(「Resultの連鎖」「エフェクト宣言の書き方」等) |
| `kei_transpile` | source: string | TS source + source map | 動作確認用 |

### 6.2 Diagnostic スキーマ

確定版の正式な定義は `spec/diagnostic-schema.md` を参照(M0 で確定)。以下は例。

```json
{
  "severity": "error",
  "code": "KEI-E3042",
  "message": "Effect 'Database.Write' used but not declared in 'uses' clause",
  "span": { "file": "transfer.kei", "start": {"line": 12, "col": 3}, "end": {"line": 12, "col": 28} },
  "fixes": [
    { "title": "Add 'Database.Write' to uses clause", "edits": [ ... ] }
  ]
}
```

- 全エラーコードに `kei_spec` で引ける解説ページを対応させる。エラー→仕様→修正のループを閉じる。

## 7. CLI

```
kei check <file>     # 静的検査(JSON出力: --json)
kei fmt <file>       # 正規形整形
kei build <dir>      # TSへトランスパイル
kei test             # 契約チェック有効でテスト実行
```

## 8. v0.1 スコープ(MVP)

**やる:**
- パーサ + 型チェッカ + エフェクトチェッカ(関数・record・enum・Result/Option・モジュール)
- 契約の実行時アサーション化
- TSトランスパイラ
- kei fmt(正規形)
- MCPサーバー(kei_spec / kei_check / kei_fmt)

**やらない(v0.2以降):**
- ジェネリクスの完全版(v0.1は標準型のみ組み込みジェネリクス)
- ユーザー定義エフェクト / エフェクトハンドラ
- 静的証明
- パッケージマネージャ

## 9. 未決事項

1. ~~非同期の扱い~~ — **決着(M4)**: v0.1は全関数同期でトランスパイルする。エフェクトと統合する設計(`uses Async`)はv0.2以降で再検討
2. 契約式の言語 — 本体と同一文法か、契約専用のサブセットか(コレクションの量化契約 `forall` / `exists` は契約専用構文として段階2で追加。§10 / `kei-spec-v0.3-collections.md`)
3. `uses IO` のような包括許可をlintで警告すべきか(合意書の解像度問題)
4. ~~実装言語~~ — **決着(M0)**: Rust(Cargoワークスペース)。ランタイムのみTS

## 10. 射程の宣言(立場B / v0.3 以降)

> [issue #25](https://github.com/A-1ro/kei-lang/issues/25) の決定。Kei が「何になろうとしているか」の宣言。

v0.1 の組み込み型は `Int` / `String` / `Bool` / `Result` / `Option` のみで、**複数の値をまとめて扱う手段(配列・リスト・マップ)が言語内に存在しない**。結果として反復・集計・絞り込みは書けず、現状の Kei は「**1 エンティティ分の純粋コアを書く DSL**」に留まっている。

これに対し、Kei は **立場B(システム記述言語を目指す)** を採る:

- `List` を `Result` / `Option` と同格の **第三の組み込みジェネリクス** として段階導入する。反復・集計・絞り込みを言語内に取り込み、そこにも `uses` と契約を効かせる。
- 退けた選択肢(立場A=純粋コア DSL に留まる)は思想として一貫していたが、**合意書(§1 第一条)の及ぶ範囲が原理的に 1 エンティティに固定される**弱点を許容しない。
- **一度に入れない。** 段階1(`List` + 量化子なしのコンビネータ契約)→ 段階2(量化契約 `forall`/`exists`)→ 段階3(`Map`)の二段構え。ユーザー定義ジェネリクスの一般開放は **しない**(`List` は特別扱いに留める)。

詳細設計(段階1のコンビネータ API、量化子なしで書ける契約の範囲、設計依存)は **`spec/kei-spec-v0.3-collections.md`(Draft)** が正本。ロードマップ上の刻みは `docs/kei-roadmap-v0.3.md` の **M9** 以降。issue #25 は v0.3 以降のコレクション系の親 issue。
