# Kei 開発ロードマップ v0.4 — /goal 契約書集

> 運用ルール: 各 Milestone は「人間が合意する契約」。
> 完了条件は必ず機械検証可能な形(テスト・コマンド出力)で書く。
> 本ファイルは v0.4 ラベルの open issue(#54〜#62)を、ドッグフードで露出した
> 「承認者が読めること」「import 境界でも検査が続くこと」「実行可能な例を言語内に置けること」
> という観点で並べ直したもの。

## Milestone 全体像と順序

| M | テーマ | issue | 優先度 | 状態 | 主な改修クレート |
|---|---|---|---|---|---|
| **M19** | `kei fmt` のコメント保持(lossless formatting) | #54 | high | ✅ 実装済み | kei_syntax / kei_fmt |
| **M20** | import 境界の型定義解決(record/enum/tagged) | #55 | high | ✅ 実装済み | kei_check / kei_cli |
| **M21** | 論理和 `\|\|` と剰余 `%` | #58 | medium | ✅ 実装済み | kei_syntax / kei_check / kei_fmt / kei_emit |
| **M22** | List リテラル + tagged 明示構築 | #57 | medium | 未着手 | kei_syntax / kei_check / kei_emit |
| **M23** | List / record 引数の generative 検証 | #60 | medium | 未着手 | kei_check / kei_cli |
| **M24** | 外部状態事後条件の再検証ケース拡充 | #56 | high | v0.3 実装済み・追試待ち | kei_emit / e2e |
| **M25** | コンビネータ引数位置限定ラムダ | #59 | low | 設計待ち | kei_syntax / kei_check / kei_emit |
| **M26** | 金額表現(`Money` 実在化 or 最小通貨単位の明文化) | #61 | low | 未着手 | spec / examples |
| **M27** | 単項演算子表の整合(`-x` / `!x`) | #62 | low | ✅ 実装済み | spec / skill / golden |

順序の論拠:

- **M19/M20 は承認と健全性の高優先ギャップ。** コメントが fmt で消えると合意書としてのソースが弱くなり、import した record/enum が opaque のままだとモジュール分割した瞬間に型検査の価値が落ちる。
- **M21/M22/M23 は v0.4 のドッグフード性を上げる中核。** `||`/`%` は契約表現力の小さな穴を塞ぐ。List リテラルと tagged 構築は自己完結した fixture を可能にし、その上で List/record PBT が集計・計画へ届く。
- **M24 は v0.3 M14 の再検証。** `extern query` と runtime old キャプチャは実装済みだが、#56 が求める「本体がズレたら契約違反になる」反例を在庫ドメインでも追加して確信度を上げる。
- **M25〜M27 は ergonomics/文書整合。** ラムダはスコープ膨張のため設計合意を先行する。Money と単項演算子は、エージェントが取説だけ読んで正しく書けることを守るための整合タスク。

## M21: 論理和 `||` と剰余 `%`(#58)

### 完了条件

- `a == 0 || a >= minLot` が本体・`requires`・`ensures` で書け、`kei check` を通る。
- `amount % caseSize == 0` が書け、TS 出力は Kei の `Int` 除算(`Math.trunc`)と同じ商で定義した剰余 `a - Math.trunc(a / b) * b` になる。
- spec / skill の演算子表に `||` / `%` / 単項 `-` / 単項 `!` の優先順位と型を明記する。
- syntax / check / fmt / emit の golden または単体テストで固定し、`cargo test --workspace` が通る。

### スコープ外

- `&&`。`requires` の複数行と `if` で代替できるため、必要性が出た時点で別 issue に切る。
- 0 除算の静的診断。既存の `/` と同じくランタイム/生成検証側の trap として扱う。

## M27: 単項演算子表の整合(#62)

### 完了条件

- `-x` と `!x` が正式サポートであることを spec / skill に明記する。
- `-x` は `Int` または `Int` 基底の tagged 型、`!x` は `Bool` に限る。
- unary は postfix より弱く、二項 `*`/`/`/`%` より強く結合することを formatter と契約式表記で保つ。

## 後続 /goal ドラフト

```text
/goal M19: `kei fmt` が leading / trailing / body コメントを保持し、コメント付き正規形の
golden round-trip を追加する。cargo fmt --all -- --check、cargo clippy --workspace --all-targets -- -D warnings、
cargo test --workspace を通して結果を表示する。
```

```text
/goal M20: 単一ファイル check でも import 先の module path を解決し、import した record の
存在しないフィールドを KEI-E2002、フィールド型誤用を KEI-E2001、enum match の不適合を
既存診断で検出する。解決不能 import の扱いを spec に明記し、golden を追加する。
```

```text
/goal M22: `[a, b, c]` と空 List の明示構築、tagged 型の明示コンストラクタを入れ、
examples/collections に自己完結した在庫 fixture を置けるようにする。素の base→tagged 代入は
引き続き KEI-E2005 として固定する。
```
