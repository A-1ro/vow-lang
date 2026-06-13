# Vow 開発ロードマップ — /goal 契約書集 v1

> 運用ルール: 各Milestoneは「人間が合意する契約」。
> 完了条件は必ず機械検証可能な形(テスト・コマンド出力)で書く。
> 評価モデルはトランスクリプトしか見えないため、/goal文には「検証コマンドの実行と結果表示」を含めること。
> 🤝 マークは /goal 投入前に人間との設計合意が必要な事項。

---

## M0: 土台

### 🤝 事前合意
- エラーコード採番ルール(`VOW-E[カテゴリ1桁][連番3桁]` 等)
- Diagnosticスキーマ最終形(spec v0.1 §6.2をベースにレビュー)
- ※ Diagnosticは全クレートの心臓部。ここだけは/goal前に人間レビュー必須。

### /goal
```
/goal Cargoワークスペース(vow_syntax, vow_check, vow_fmt, vow_emit,
vow_cli, vow_mcp の6クレート)が初期化され、共通Diagnostic型が
spec/diagnostic-schema.md の定義通りにvow_checkクレートに実装され、
serdeでのJSONシリアライズのroundtripテストを含む cargo test が全件パスし、
cargo clippy -- -D warnings がゼロ警告で、CI設定(GitHub Actions:
fmt/clippy/test)がコミットされている。最後に cargo test と cargo clippy の
出力を表示して完了とする。
```

### スコープ外
- パーサ・チェッカの実装一切

---

## M1: vow_syntax(レキサー+パーサ+AST)

### 🤝 事前合意
- v0.1文法の最終確定(spec §2)。特にbody内式の最小セット:
  let / if / return / 関数呼び出し / `else fail` / フィールドアクセス / リテラル
- golden testの入力ケース一覧(正常系15本+異常系15本を人間がレビュー)

### /goal
```
/goal tests/golden/syntax/ 配下の全golden test(正常系: .vow入力 →
期待ASTのJSONダンプ一致、異常系: .vow入力 → 期待Diagnostic JSON一致)が
パスする。パーサはエラー回復に対応し、異常系ケースで複数Diagnosticを
返せること。cargo test --workspace 全件パス、clippy警告ゼロ。
最後にテスト結果サマリを表示して完了とする。
```

### golden testケース設計方針
- 正常系: 契約節フル装備の関数 / record / enum / module+import / else fail / ネストif
- 異常系: uses節のtypo / 閉じ括弧欠落 / 予約語の識別子使用 / 複数エラー同居ファイル

### スコープ外
- 型チェック・名前解決(parse-onlyで通す)

---

## M2: vow_fmt(正規形フォーマッタ)

### 🤝 事前合意
- 正規形スタイルの確定(インデント幅、契約節の改行規則、1行の最大幅、
  import順序の正規化ルール)。サンプル3ファイルの整形例を人間が承認。

### /goal
```
/goal vow_fmtが実装され、(1) tests/golden/fmt/ の入力→期待出力ペアが
全件一致、(2) proptest によるroundtripプロパティテスト
「parse(fmt(parse(src))) == parse(src)」がexamples/配下全ファイル+
生成ケース1000件でパス、(3) fmt(fmt(x)) == fmt(x) の冪等性テストがパス
する。cargo test全件パス、clippy警告ゼロ。テスト結果を表示して完了。
```

### 備考
- roundtripテストがM1パーサのバグを炙り出す想定。M1の修正が必要になったら
  このgoal内で直してよい(golden test維持が条件)。

---

## M3: vow_check(名前解決+型+エフェクト+契約検査)

### 🤝 事前合意
- 型エラー・エフェクトエラーの主要エラーコードとメッセージ文面のレビュー
- 「uses IO 包括許可」をwarningにするか否か(spec §9 未決事項3の決着)
- 契約式の純粋性検査の仕様詳細

### /goal
```
/goal tests/golden/check/ 配下の全golden testがパスする。カバー範囲:
(1) 名前解決(未定義参照・重複定義・import解決)、(2) 型チェック
(record/enum/Result/Option/tagged型の混同検出)、(3) エフェクト検査
(uses未宣言の使用検出・呼び出し先からの推移的伝播・階層包含判定)、
(4) 契約式の純粋性検査(契約内での副作用関数呼び出しをエラー化)。
全Diagnosticにspan・code・最低1つのfix候補が含まれることをテストで
検証する。cargo test全件パス、clippy警告ゼロ。結果を表示して完了。
```

### 備考
- ここが最大のMilestone。/goalが長時間化したら(1)〜(4)で分割投入に切り替え可。

---

## M4: vow_emit(TSトランスパイラ+ランタイム)

### 🤝 事前合意
- 非同期の扱いの決着(spec §9 未決事項1)← M4着手前に必ず決める
- @vow/runtime のAPI表面(Result/Optionのメソッド名)
- 出力TSのスタイル(人間可読性の基準サンプルを承認)

### 合意の記録(M4実施時)
- **非同期**: v0.1 は全関数同期出力。`uses Async` 統合は v0.2 で再検討(spec §5/§9 に反映済み)
- **@vow/runtime API**: `Result<T,E> = Ok<T> | Err<E>`(`isOk`/`isErr`/`value`/`error`)、
  `Option<T> = Some<T> | None`(`isSome`/`isNone`/`value`)。両者は内部判別子 `ok` を共有し
  `else fail` の展開が単一形になる。契約違反は `VowContractViolation`
  (clause/func/condition/file/line/col、`toJSON()` あり)
- **出力TSスタイル**: 基準サンプルは tests/e2e/generated/contracts/withdraw.ts(e2e 実行で再生成)。
  契約は docコメント + 実行時アサーション、enum は `kind` 判別 tagged union

### /goal
```
/goal examples/配下の全.vowファイルがTSにトランスパイルされ、
(1) 出力TSが tsc --strict --noEmit でエラーゼロ、(2) tests/e2e/ の
実行テスト(トランスパイル→vitest実行→期待出力一致)が全件パス、
(3) requires違反が実行時に構造化エラーを投げることのテストがパス、
(4) source mapが生成されエラー位置が.vow側の行番号に解決される
テストがパスする。cargo test全件パス。結果を表示して完了。
```

---

## M5: vow_mcp + ドッグフード実験

### 🤝 事前合意
- vow_spec の応答フォーマット(仕様セクションの構造化粒度)
- ドッグフード実験のお題設定(下記)

### /goal
```
/goal MCPサーバー(vow_spec / vow_check / vow_fmt / vow_examples)が
stdioトランスポートで起動し、tests/mcp/ の統合テスト(各ツールの
リクエスト→レスポンスのgolden test)が全件パスする。spec/ と examples/
の内容がビルド時にサーバーへ埋め込まれ、spec更新→再ビルドで応答が
変わることをテストで検証する。cargo test全件パス。結果を表示して完了。
```

### ドッグフード実験(v0.1完了の儀式・人間が主導)
- 新規セッションのClaudeに「Vow MCPサーバーのみ」を渡し、Vowの事前知識
  ゼロの状態で課題(例: 在庫管理の関数3本を契約つきで書く)を依頼
- 成功基準: vow_check エラーゼロのコードに3往復以内に到達するか
- 結果はそのままZenn記事のコアコンテンツになる

---

## 全体運用メモ

- 各/goal投入前に対象Milestoneのgolden testケースを人間がレビューする。
  **golden testこそが契約本文**であり、/goal文はその履行命令にすぎない。
- /goalが2時間以上回り続けたら一度止めて、ゴールの分割を検討する。
- Milestone完了ごとに `vow fmt` を全コードベースに適用し、specとの乖離が
  あればspecを先に直す(仕様が常にsource of truth)。