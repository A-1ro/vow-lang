# Vow 開発ロードマップ — /goal 契約書集 v1

> 運用ルール: 各Milestoneは「人間が合意する契約」。
> 完了条件は必ず機械検証可能な形(テスト・コマンド出力)で書く。
> 評価モデルはトランスクリプトしか見えないため、/goal文には「検証コマンドの実行と結果表示」を含めること。
> 🤝 マークは /goal 投入前に人間(てるい)との設計合意が必要な事項。

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

## M6: vow_cli — `vow check` / `vow fmt`(単一ファイル CLI 基盤)

> `vow_cli` はスタブ(`main.rs` が exit 1 を返すのみ)。spec §7 の 4 サブコマンドの
> うち、純 Rust・単一ファイルで完結する check / fmt を先に固める。CLI は
> 引数解釈・ファイル IO・**Diagnostic の散文整形のみ**を持ち、言語処理は
> vow_check / vow_fmt / vow_syntax へ委譲する(ARCHITECTURE.md vow_cli 行)。

### 🤝 事前合意
- **散文 Diagnostic レンダリング形式**(`vow check` の既定出力)。rustc 風
  (`error[VOW-E3042]: <message>` + `--> file:line:col` + ソース行 + キャレット下線 +
  `= fix: <title>`)を基準サンプルでレビュー。これが人間向けの一次接触面なので
  golden snapshot を人間承認する。**散文整形は CLI に許された唯一の「ロジック」**で、
  Diagnostic の全要素(code/message/span/fix)を欠落なく描画すること。
- **`vow fmt` の挙動**: 既定は整形結果を stdout に出す(非破壊)。`--write` で上書き、
  `--check` で整形済みか検証(未整形なら exit 1 + 差分表示)。cargo fmt 流(既定 in-place)に
  するか prettier 流(既定 stdout)にするかを決定する。構文エラー入力は整形せず
  Diagnostic を返す(vow_fmt::format_source の既存挙動)。
- **終了コード規約**: `0`=成功 / `1`=診断エラー検出(check)・未整形(fmt --check) /
  `2`=使用法エラー(引数不正・ファイル不在)。全サブコマンド共通。
- **依存追加の可否**: 引数パーサ(clap 等)、CLI 統合テスト用(assert_cmd / predicates /
  tempfile)。追加するなら ARCHITECTURE.md と Cargo.toml に記録する(不変条件の手続き)。

### /goal
```
/goal `vow` バイナリの check / fmt サブコマンドが実装され、言語処理は
vow_check / vow_fmt / vow_syntax への委譲のみで(CLI は引数解釈・ファイル IO・
Diagnostic の散文整形だけを持つ)、tests/cli/ の golden test が全件パスする:
(1) `vow check <file>` が散文 Diagnostic を既定出力し `--json` で
Diagnostic[] (構造化) を出す、(2) エラーありで exit 1・なしで exit 0、
(3) `vow fmt <file>` が正規形を出力し `--check` で未整形を exit 1 で検出、
(4) 引数不正・ファイル不在で exit 2。CLI 統合テストは実バイナリを
プロセス起動して stdout / stderr / 終了コードを検証する。新設の tests/cli/ と
追加依存を ARCHITECTURE.md に反映する。cargo test --workspace 全件パス、
clippy 警告ゼロ。最後にテスト結果サマリと `vow check`・`vow fmt` の実行例出力を
表示して完了とする。
```

### golden / test 設計方針
- `vow check`: examples/ とエラー入りサンプルを入力に、`{name}.check.txt`(散文)と
  `{name}.check.json`(Diagnostic[])の両方を期待値に持つ。
- `vow fmt`: 未整形入力 → 正規形 stdout 一致 / 整形済み入力 → `--check` exit 0 /
  構文エラー入力 → 整形せず Diagnostic + exit 1。
- 散文整形が span(file:line:col)とソース行・キャレットを正しく対応づけることを検証。

### スコープ外
- `vow build` / `vow test`(M7)
- 設定ファイル(vow.toml)・複数ファイル一括処理・ウォッチモード

---

## M7: vow_cli — `vow build` / `vow test`(プロジェクト CLI)

> M6 の基盤(引数解釈・終了コード・散文整形)の上に、ディレクトリ単位の
> トランスパイルとテスト実行を載せる。言語処理は vow_emit への委譲のみ。
> ビルドパイプラインは tests/e2e/ の手動手順(transpile → tsc → vitest)を
> `vow build` / `vow test` として正規化したもの。

### 🤝 事前合意
- **プロジェクト/ビルドモデル**: 入力は `<dir>` 以下の `**/*.vow` を再帰収集。出力先は
  既定 `<dir>/dist/`(`--out-dir` で変更)、emit の `ts_path`(モジュールパス由来)で
  1:1 配置(spec §5 のファイルパス対応)。source map は既定 on(`--no-source-map` で抑止)。
- **部分失敗の扱い**: 全ファイルを先に検査し、1 ファイルでもエラーがあれば**何も書かず**
  全 Diagnostic を出して exit 1(all-or-nothing。中途半端な dist/ を残さない)。
- **dev / release ビルド**(spec §4): `--release` で `ensures` 除去・`requires` のみ残すか。
  emit 側にビルドモード引数が要るため、M7 で対応するか v0.2 送りにするかを決定する。
- **`vow test` の実体**: v0.1 は Vow に test 構文がない。dev ビルド(契約 on)→ Node の
  テストランナー(vitest)へ委譲するラッパーとするか、v0.1 ではスコープ外にするか。
  採用する場合は Node 前提(CI の test ジョブと同じ)で、契約違反が非ゼロ終了に
  伝播することを保証する。

### /goal
```
/goal `vow` バイナリの build / test サブコマンドが実装され、言語処理は
vow_emit への委譲のみ。(1) `vow build <dir>` が <dir> 配下の全 .vow を検査し、
エラーゼロのとき out-dir に TS + source map をモジュールパス通りに書き出す、
(2) 1 ファイルでもエラーなら何も書かず全 Diagnostic を出して exit 1、
(3) 生成 TS が tsc --strict --noEmit でエラーゼロ、(4) `vow test` が dev ビルド後に
テストランナーを起動し契約 on で実行(requires 違反を検出して非ゼロ終了することを
含む)。tests/cli/projects/ のフィクスチャでビルド出力ツリーを golden 比較し、
CLI 統合テストは実バイナリをプロセス起動して終了コードと出力を検証する。
cargo test --workspace 全件パス、clippy 警告ゼロ。最後に `vow build` の出力ツリーと
`vow test` の結果を表示して完了とする。
```

### golden / test 設計方針
- `tests/cli/projects/<name>/`: 入力 .vow 群 + 期待 `dist/` ツリー(TS + .map)。
  examples/ の再利用可。出力ツリーのパスと内容を golden 比較する。
- 契約違反フィクスチャ: requires を破る入力で `vow test` が非ゼロ終了 + 構造化エラー
  (`VowContractViolation`)を出すこと(M4 の e2e 契約違反テストの CLI 版)。

### スコープ外
- パッケージマネージャ・依存解決(spec §8「やらない」)
- ウォッチモード・インクリメンタルビルド
- release 最適化(`--release` を入れない判断ならビルドモード自体を丸ごと)

---

## 全体運用メモ

- 各/goal投入前に対象Milestoneのgolden testケースを人間がレビューする。
  **golden testこそが契約本文**であり、/goal文はその履行命令にすぎない。
- /goalが2時間以上回り続けたら一度止めて、ゴールの分割を検討する。
- Milestone完了ごとに `vow fmt` を全コードベースに適用し、specとの乖離が
  あればspecを先に直す(仕様が常にsource of truth)。