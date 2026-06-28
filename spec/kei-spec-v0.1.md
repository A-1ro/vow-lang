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
