# Kei 開発ロードマップ v0.2 — /goal 契約書集

> 運用ルール: 各Milestoneは「人間が合意する契約」。
> 完了条件は必ず機械検証可能な形(テスト・コマンド出力)で書く。
> 評価モデルはトランスクリプトしか見えないため、/goal文には「検証コマンドの実行と結果表示」を含めること。
> 🤝 マークは /goal 投入前に人間との設計合意が必要な事項。
>
> 本ファイルは `docs/kei-roadmap-goals.md`(v0.1 の M0〜M7、提案中の M8/M9)の続編。
> **v0.2 のテーマは「健全性(soundness)と契約表現力の強化」。** 出典は図書館貸出システムの
> ドッグフード実験(書き手AI本人のフィードバック)と外部設計レビュー(Issues #20–#23)。
> v0.1 が「曖昧さゼロの最小核」を作ったのに対し、v0.2 は **その核で露わになった穴**
> ——純粋文脈での失敗処理・外部境界の未検証・契約と検証の混同——を塞ぐ。

## Milestone 全体像と順序

| M | テーマ | issue | 優先度 | 主な改修クレート |
|---|---|---|---|---|
| **M10** | `match` 式 + Option の純粋分解 | #20 | P1 / high | kei_syntax / kei_check / kei_emit / kei_fmt |
| **M11** | 外部境界の検証 — `extern` 署名 | #22 | medium | kei_syntax / kei_check |
| **M12** | 契約の検証レベルを診断報告 | #23 | medium | kei_check / diagnostic-schema / kei_cli |
| **M13** | エフェクト関数の事後条件(短期分) | #21 | high | spec / docs / examples 主体 |

順序の論拠:

- **M10 を先頭に。** `match` は型システムの基礎機能で他に依存せず、最優先(P1)。Option/Result/enum を一律に網羅分解でき、後続 Milestone のコード表現力も底上げする。
- **M11 → M12 は連続。** `extern`(外部境界の型・エフェクト署名)は検証レベル報告(M12)の前提。M12 の `trusted`/`unchecked` の構文明示は #22 の `extern` と統合検討される(両 issue が相互参照)。
- **M13 は設計主体で最後。** 最も重い(外部状態の事後条件)。issue の二段構えに従い、**v0.2 では短期分(イディオムの spec 正式化+言語拡張の比較メモ+目標ケース定義)に絞り**、本格的な言語拡張は v0.3+ の設計合意後に送る。

---

## M10: `match` 式 + Option の純粋分解(#20)

> **問題(#20)**: `Option<T>` の中身を取り出す唯一の手段が `else fail`(= Result 文脈専用)で、
> Result を返さない純粋関数の内部で Option を開けない。本来 `Option` で表現したい API
> (`isOverdue -> Option<Bool>`)が、言語都合で `Result` に歪められた。
> **本命案**: まず `match` 式(網羅性をコンパイラが強制)を入れる。Result/enum にも一律に効き、
> 「曖昧さゼロ」と整合する。コンビネータ(`map`/`unwrapOr`/`andThen`)は糖衣として後追い。

### 🤝 事前合意

- **`match` の構文確定**: `match <式> { <パターン> => <式>, ... }`。パターンは少なくとも
  `Some(x)` / `None` / `Ok(x)` / `Err(e)` / enum バリアント(`E.V` / `E.V(x)` / `E.V { f }`)を被覆する。
  腕の区切り(`,` か改行か)、腕の本体が式のみか文ブロックを許すかを確定する。
- **式か文か**: issue の例は `let r = match ... { ... }`(式)。`match` を **式**として定め、
  各腕の式の型を一致させて全体の型を決める方針でよいか。
- **網羅性検査の厳格度**: 全バリアント未被覆をエラーにする。**ワイルドカード `_` を入れるか**が
  最大の論点——入れると網羅漏れを隠せてしまい「暗黙なし」と緊張する。**既定線は `_` を入れない**
  (全ケース明示を強制)。入れるなら到達不能・冗長腕の検査もセットにする。
- **コンビネータの扱い**: `map` / `unwrapOr` / `andThen` は **M10 のスコープ外**(将来の糖衣)とし、
  M10 は `match` のみに集中してよいか(issue の「match を先に」方針に沿う)。
- **パターンの深さ**: ネストパターン(`Some(Ok(x))`)・ガード(`Some(x) if ...`)は v0.2 では入れず、
  1 段のパターンに限る、で確定するか。
- **新規エラーコード**: 網羅性違反・腕の型不一致・到達不能腕の `KEI-E[2|...]xxx` を採番(M0 の
  `KEI-E[カテゴリ1桁][連番3桁]` 規約に従う。網羅性は型系=カテゴリ2 が素直)。

### /goal

```
/goal `match` 式が kei_syntax / kei_check / kei_emit / kei_fmt に入り、言語処理は各クレートの
責務内で完結する。(1) `Option<T>` / `Result<T, E>` / ユーザー enum を網羅分解できる
(`match e { Some(x) => ..., None => ... }`)、(2) 網羅性検査: 全バリアント未被覆(例: None 腕の
欠落、enum の一部バリアント欠落)が新規 Diagnostic でコンパイルエラーになり、span・code・
最低1つの fix 候補を持つ、(3) 各腕の式の型が一致し match 式全体の型が確定する(不一致は
KEI-E2001)、束縛変数(`Some(loan)` の `loan`)は当該腕のスコープに限定される、(4) Result を
返さない純粋関数の内部で Option を開けること——#20 の `isOverdue -> Option<Bool>` が自然に
書けること——を golden で固定、(5) `match` が TS に展開され `tsc --strict --noEmit` でエラーゼロ、
e2e で実行一致、(6) `match` の正規形を kei_fmt が固定し冪等性・roundtrip テストがパスする。
spec/kei-spec-v0.1.md(または v0.2 章)・skills/kei/SKILL.md・examples/ に `match` と
「Option を純粋文脈で開く」イディオムを追記する。cargo test --workspace 全件パス、
clippy 警告ゼロ。最後にテスト結果サマリと `match` を使った check-clean 例を表示して完了とする。
```

### golden / test 設計方針

- `tests/golden/syntax/`: `match` の正常系 AST、異常系(腕の欠落・区切り誤り・閉じ括弧欠落)。
- `tests/golden/check/`: 網羅性 ok / err(None 欠落・enum 一部欠落)、腕の型不一致、束縛変数スコープ、
  純粋文脈での Option 分解(`isOverdue` 相当)。
- `tests/golden/fmt/`: `match` の正規形・冪等・roundtrip。
- `examples/`: `isOverdue`(`Option<Bool>` を `match` で組む)を追加し check-clean を保つ。
- `tests/e2e/`: `match` を含む `.kei` のトランスパイル→実行一致。

### スコープ外

- Option / Result コンビネータ(`map` / `unwrapOr` / `andThen`)——糖衣として将来送り
- ネストパターン・パターンガード・`match` 文(文ブロック腕)
- ワイルドカード `_`(🤝 で「入れない」と決めた場合。入れる判断なら本 Milestone に含める)

---

## M11: 外部境界の検証 — `extern` 署名(#22)

> **問題(#22)**: 外部 namespace 呼び出し(`Database.*` / `Time.now()`)が opaque で、戻り型も
> エフェクトも検査対象外。`Time.now()` を呼びながら `uses Clock` を宣言し忘れても検出されない。
> 「曖昧さゼロ・暗黙なし」を掲げる言語の **境界部分でだけ合意書の担保が外れている**。
> **本命案**: 外部関数に戻り型とエフェクトを宣言する `extern` 署名を必須化し、checker が照合する。
> 宣言コストは増えるが「書くのは AI」なので第一条と矛盾しない。

### 🤝 事前合意

- **`extern` 構文の確定**: `extern Time.now() -> Int uses Clock` /
  `extern Database.fetchBalance(account: AccountId) -> Option<Money> uses Database.Read`。
  どこに書くか(モジュール先頭の宣言群 / 専用の宣言ファイル / `import` への注釈)を決める。
- **既存 opaque 運用からの移行**(最重要): 現状 `import infra.database as Database` 配下の呼び出しは
  opaque で check を通る(SKILL.md §2 に明記)。`extern` 必須化は **既存 examples / golden を破壊する**。
  → 移行戦略を決める: (a) `extern` 未宣言の外部呼び出しを当面 **warning**(段階移行)、(b) 即 **error**
  (一括移行)。既存の golden 変更は人間レビュー必須(不変条件1)。
- **標準ライブラリの `extern` 提供元**: `Time` / `Database` / `Audit` 等の標準名前空間の署名を
  組み込み prelude で配るか、プロジェクトごとにユーザー宣言させるか。
- **`unsafe` 境界マーク**(#22 案2)を併設するか: `extern` が間に合わない境界の暫定明示。
  M12 の `trusted` / `unchecked` と統合検討(#23 の「例外: trusted/unchecked のみ構文明示」と接続)。
- **`extern` の型に使える範囲**: 組み込み型 + ユーザー定義型 + `Option` / `Result`。
- **新規エラーコード**: `extern` と実 `uses` の不一致・未宣言外部呼び出しの `KEI-E3xxx`(エフェクト系)を採番。

### /goal

```
/goal 外部関数の戻り型とエフェクトを宣言する `extern` 署名が kei_syntax / kei_check に入り、
言語処理は両クレートの責務内で完結する(kei_emit は extern 呼び出しを対応する TS 呼び出し/import
へ素直に写すのみ)。(1) `extern Time.now() -> Int uses Clock` 形式の署名をパース・登録、
(2) 外部 namespace 呼び出しが extern 署名と照合され、戻り型が型検査に伝播する、(3) extern が
宣言したエフェクトが呼び出し元の uses へ推移的に伝播し、宣言漏れが **境界越しで KEI-E3001 として
落ちる**(#22 の「`uses Clock` 書き忘れ」再現コードが正しくエラーになる)、(4) extern 署名と実際の
uses の不一致・未宣言の外部呼び出しを検出し、全 Diagnostic に span・code・最低1つの fix 候補を持つ。
spec のエフェクト章・import 章に外部境界の扱いを追記し、skills/kei/SKILL.md の「外部呼び出しは
opaque で通る」記述(§2)を更新する。既存 examples/ の外部呼び出しに extern を付与して check-clean を
維持する(golden の変更は人間レビュー必須)。cargo test --workspace 全件パス、clippy 警告ゼロ。
最後にテスト結果サマリと #22 再現コードが落ちる様子を表示して完了とする。
```

### golden / test 設計方針

- `tests/golden/check/`: `extern` あり ok / `Clock` 宣言漏れ err(#22 再現)/ extern と uses の不一致 err /
  extern 戻り型と使用箇所の型不一致 err。
- 既存 `examples/`(`withdraw.kei` / `transfer.kei` 等が `Database.*` / `Audit.Log` を使用)への `extern` 付与。
  これに伴う既存 golden の expected 変更は **人間レビュー必須**(不変条件1)。
- 移行戦略が warning 段階なら、warning を出す golden も用意。

### スコープ外

- TS 型定義(`.d.ts`)からの `extern` 自動生成
- 標準 prelude の `extern` を網羅完備すること(必要最小の標準署名から始める)
- 外部関数の契約(`requires`/`ensures`)宣言(エフェクト・型に限定。契約境界は将来)

---

## M12: 契約の検証レベルを診断報告(#23)

> **問題(#23)**: 「契約が**書かれている**こと」と「その契約が実際に**機械検証された**こと」は別物。
> AI 時代の言語では、書かれた保証と検査された保証の区別が決定的に重要。
> **設計判断(確定済み・#23 本文)**: 検証レベルを **ソース構文に書き分けない**。代わりに
> **`kei check` の診断出力(構造化データ)に載せる**。理由は (1) 合意書原則の保護(契約は「何を
> 保証するか」に集中させ、承認時の認知負荷を上げない)、(2) 検証レベルは「処理系が達成できた
> レベルの報告」であって書き手の選択ではない、(3) **契約は不変・検証は成長**の分離(検証器が
> 強化されても契約ソースを書き換えずに済む)。

### 🤝 事前合意

- **検証レベルの分類確定**: `static`(コンパイル時/将来 SMT で検証済み)/ `runtime`(実行時
  アサーションへ展開、v0.1 既定)/ `trusted`(外部システム・人間レビュー・テストで保証、検証器の
  管轄外)/ `unchecked`(明示的に未検証)。各定義と判定基準を固める。
- **v0.2 時点の判定ロジックの射程**: v0.1 は全契約が runtime アサーション。**どこまで static に
  上げるか**を決める。既定線は「v0.2 では大半 `runtime` 固定とし、自明な定数境界(例:
  `requires step > 0` の純粋・定数評価可能なもの)を `static` 判定する最小実装に留める」。
  本格的な static 検証(SMT)は v1.0 送り。
- **`trusted` / `unchecked` の構文明示を入れるか**: #23 の「例外」と #22 の `extern` を統合し、
  「ここは検証外」を構文で可視化するか。入れるなら構文・対象(契約単位 / 関数単位 / 境界単位)を確定。
- **Diagnostic スキーマ拡張**(慎重さ最大): 各契約に `{ kind, expr, verification }` を付与する形を
  `spec/diagnostic-schema.md` に追加する。**Diagnostic は M0 で「全クレートの心臓部・人間レビュー
  必須」とされた**。スキーマ変更は serde roundtrip テストと人間承認をセットにする。
- **散文出力での見せ方**: `--json` が正、散文は派生(不変条件2)。散文に検証レベルを欠落なく要約する形。

### /goal

```
/goal `kei check --json` が各契約(requires / ensures)に達成検証レベル
(static / runtime / trusted / unchecked)を付与する。言語処理は kei_check に閉じ、CLI は
出力整形のみ。(1) spec/diagnostic-schema.md に契約の検証レベルフィールド
(`{ kind, expr, verification }`)を定義し、serde シリアライズの roundtrip テストを追加
(スキーマ拡張は人間レビュー必須)、(2) kei_check が各契約の達成レベルを判定して構造化出力へ
載せる、(3) 検証レベルはソース構文に現れない(`trusted` / `unchecked` の明示構文を入れる判断を
した場合のみ例外)、(4) `static` に上がるケースと `runtime` 止まりのケースの判定を golden で固定、
(5) 散文出力(既定)にも検証レベルの要約を欠落なく描画する。spec の契約章に「契約は不変・検証は
成長」の分離原則を明記する。cargo test --workspace 全件パス、clippy 警告ゼロ。
最後に契約付き関数の `kei check --json` 出力(検証レベル付き)を表示して完了とする。
```

### golden / test 設計方針

- `tests/golden/check/`: 契約付き関数の `--json` に `verification` が乗る / `static` 判定ケース /
  `runtime` 止まりケース / (構文明示を入れた場合)`trusted` / `unchecked`。
- `spec/diagnostic-schema.md` のスキーマ roundtrip テスト(JSON 往復)。
- `tests/mcp/`: `kei_check` ツール応答に検証レベルが反映されること(MCP 経由の契約)。

### スコープ外

- SMT ソルバ統合・本格的な静的契約検証(v1.0 送り)
- 検証レベルをソース構文として恒常的に書き分けること(設計判断で**不採用**。例外は
  `trusted` / `unchecked` の明示のみ)

---

## M13: エフェクト関数の事後条件(#21 — 短期分)

> **問題(#21)**: 「`borrowBook` を呼ぶと在庫がちょうど1減る」を関数全体の `ensures` で表現したい。
> しかし (1) 契約式は副作用禁止で DB を読めない、(2) `old()` は引数しか参照できず外部状態に効かない、
> ため書けない。回避策(数量的契約を純粋ヘルパーに退避させ本体が経由)は妥当だが「言語の制約に
> 構造を合わせた」のであって素直ではない。結果、**「`kei check` が通った」と「在庫が1減ることが
> 保証されている」が乖離する**——書かれていない契約は検査されない。
> **二段構え(#21 本文)**: 短期=現実解(純粋ヘルパー経由)を spec に正式化、中長期=言語拡張
> (案1 エフェクト事後条件の論理的読み取り / 案2 ゴースト変数)を比較検討。
> **v0.2 では短期分に絞る**(言語拡張の実装は v0.3+ の設計合意後)。

### 🤝 事前合意

- **v0.2 のスコープ確定**: v0.2 は短期分(イディオム明文化 + 比較メモ + 目標ケース定義)に
  留め、言語拡張本体(案1/案2)は実装しない、で合意するか。issue の二段構えに沿う既定線。
- **推奨イディオムの正式形**: 「外部状態の数量的契約は純粋ヘルパーに切り出し、本体が必ず経由する」
  パターンの正式な書き方(`decrementAvailable: available > 0 ⊢ result == old(available) - 1` を
  本体が経由)。spec のどの章に置くか。
- **言語拡張の評価軸**: 案1(`ensures Database.balanceOf(a) == old(Database.balanceOf(a)) - 1`、
  契約内 DB 参照を「論理的読み取り」として特別扱い)vs 案2(検証用ゴースト/モデル変数、Dafny/Verus 式)。
  健全性・実装コスト・合意書原則との整合で比較する軸を決める。
- **目標ケースの定義**: `borrowBook` の在庫不変条件を将来どの形で表現・検証するかの達成基準
  (将来の e2e/golden の目標)を文章で固定する。

### /goal

```
/goal #21 の二段構えのうち **短期分** を完了する。(1) spec(v0.2 章 or 契約章)に「外部状態の
数量的契約は純粋ヘルパーへ切り出し、本体が必ず経由する」推奨パターンを **正式イディオム** として
明記する、(2) examples/ に在庫不変条件(`borrowBook` 相当: 純粋ヘルパー `decrementAvailable` を
経由して「ちょうど1減る」を保証)の check-clean な example を追加し golden で固定する、(3) docs/ に
言語拡張の比較メモ(案1=エフェクト事後条件の論理的読み取り / 案2=ゴースト変数)を残し、評価軸と
v0.3+ への送り判断を記録する、(4) `borrowBook` の在庫不変条件を将来表現・検証するための目標ケース
(e2e/golden の達成基準)を定義として記述する。cargo test --workspace 全件パス、clippy 警告ゼロ。
最後に追加した example の check-clean と比較メモの所在を表示して完了とする。
```

### golden / test 設計方針

- `examples/`: 純粋ヘルパー経由で数量保存を保証する `borrowBook` 相当を追加、check-clean を golden 化。
- `docs/`: 言語拡張比較メモ(案1 vs 案2)。これは設計ドキュメントでテスト対象ではないが、
  目標ケースの記述は将来の Milestone の受け入れ条件として参照される。

### スコープ外

- エフェクト事後条件・ゴースト変数の **言語実装本体**(v0.3+ で案を確定してから)
- SMT による外部状態の静的検証
- `old()` の外部状態への拡張(言語拡張側の論点)

---

## v0.2 全体運用メモ

- v0.2 は **健全性(#22 の境界検証)と契約表現力(#20 純粋分解 / #21 事後条件 / #23 検証レベル)** の
  強化に集約される。すべての出典がドッグフード実験と外部レビュー——**実際に詰まった証拠**に基づく。
- 依存と順序: **M10(match)は独立先行**。**M11(extern)→ M12(検証レベル)は境界・診断で連続**
  (`trusted`/`unchecked` の構文明示で接続)。**M13 は設計主体**で短期分のみ。
- **M11 と M12 は既存契約面に触れる**: M11 は既存 examples/golden の外部呼び出し移行を伴い、
  M12 は Diagnostic スキーマを拡張する(M0 で人間レビュー必須とされた心臓部)。
  どちらも golden / スキーマ変更は **人間レビュー必須**(不変条件1・2)。
- 各 /goal 投入前に対象 Milestone の golden ケースを人間がレビューする。**golden test こそが契約本文**。
- Milestone 完了ごとに `kei fmt` を全コードベースに適用し、spec との乖離があれば **spec を先に直す**
  (仕様が常に source of truth)。
