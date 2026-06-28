---
name: kei-dogfood
description: Kei プラグイン / MCP サーバーだけを頼りに、Kei 文法を一切教えず Sonnet サブエージェント(初見ユーザー想定)に .kei を書かせて、取説(SKILL.md / spec / examples)と Diagnostic が「初見で使えるか」を実証する。完了後、ハマりどころ・不足情報を構造化フィードバックとして回収する。「ドッグフード」「初見テスト」「kei MCP の取説テスト」と言われたら呼ぶ。
---

# kei-dogfood — Kei プラグイン / MCP のドッグフード検証

Kei は「AI が書き、人間が承認し、コンパイラが履行を保証する」前提で設計した言語。
このスキルは **その前提が成立しているか** を、Kei を知らない Sonnet サブエージェントに
MCP ツールだけで `.kei` を書かせて検証する。詰まった箇所(取説の穴・Diagnostic の不親切・
spec の漏れ)を構造化して回収し、次の改善に回す。

「自分で書いたら通った」は反証になっていない — 自分は実装と spec を覚えている。
**初見の AI が plugin / MCP だけで詰まずに書けるか** が合格基準。

## いつ使う / 使わない

**使う**:
- 新しい spec 節 / Diagnostic / MCP ツール出力を追加した後の自己検証
- v0.X リリース前に「取説だけで書けるか」をチェック
- 既知の摩擦(例: 「契約式は副作用禁止が伝わらない」)を改善した後の効果測定

**使わない**:
- 機能の正しさのテスト(`cargo test` の責任 — 取説検証ではない)
- v0.X 射程外のお題(書けない範囲を渡すとフィードバックがノイズになる)

---

## 必須条件(3 つ)

このスキルは以下 3 条件を **すべて満たさないと回さない**。1 つでも欠けるとドッグフードの
結果が無意味になる。

### 条件 1: 委譲前に plugin(SKILL.md 配布)と MCP server を両方繋ぎ込む

ドッグフードでサブエージェントが Kei を学ぶ「正当な推論経路」は 2 つ:

- **Kei plugin の `skills/kei/SKILL.md`** — plugin が enable されていればサブエージェントの
  スキル一覧に自動で載り、`/kei` 相当のトリガで(または description マッチで)読み込まれる。
  これが**取説の主経路**。ドッグフードの目的はこの経路の有効性を測ること。
- **Kei MCP の 4 ツール** — `kei_spec`(仕様検索)/ `kei_examples`(実例)/ `kei_check`(検査)
  / `kei_fmt`(整形)。SKILL.md からさらに深い情報を引く・実行検証する**対話経路**。

両方繋がっていないとドッグフードは成立しない(SKILL.md だけだと検査できない、MCP だけだと
取説の入口が無くサブエージェントが「何があるのか」を知らないまま走る)。委譲前に Opus 側で
**両方の疎通を確認**する。

#### A. MCP server の接続(必須)

確認: 現セッションの system-reminder の deferred tools 一覧に `mcp__kei-mcp__kei_check` /
`mcp__kei-mcp__kei_spec` / `mcp__kei-mcp__kei_fmt` / `mcp__kei-mcp__kei_examples` が並んで
いるか目視 → `mcp__kei-mcp__kei_spec` の schema を ToolSearch でロードし、Opus 側で 1 回叩いて
索引(topic 空)が返ることを確認。サブエージェントは parent と同じ MCP プールに接続するので、
Opus 側で叩けるならサブエージェント側でも叩ける。

繋がっていなければ、`kei` バイナリ(`cargo install --path crates/kei_cli` でビルド済み環境に
入れるか、release Assets から入手)を PATH に置き、`~/.claude.json` または対象プロジェクトの
`.mcp.json` に以下を追加:

```json
"mcpServers": {
  "kei-mcp": {
    "type": "stdio",
    "command": "kei",
    "args": ["mcp"],
    "env": {}
  }
}
```

開発中(リポジトリ作業ツリーから最新を見たい)は `command` を `cargo` に、`args` を
`["run", "-p", "kei_cli", "--bin", "kei", "--", "mcp"]` にする。設定追加後は Claude Code を
再起動して MCP server の再接続を促す。

#### B. Plugin(skills/kei/SKILL.md 配布)の有効化(必須)

確認: `~/.claude/settings.json` の `enabledPlugins` に `kei@kei-lang: true` が入っているか
(marketplace 名はインストール時の登録による。`~/.claude/installed_plugins.json` で実状を確認)。

未登録なら以下を順に:

1. **Marketplace 追加**: `~/.claude/settings.json` の `extraKnownMarketplaces` に追加:
   ```json
   "extraKnownMarketplaces": {
     "kei-lang": { "source": { "source": "github", "repo": "A-1ro/kei-lang" } }
   }
   ```
2. **Plugin 有効化**: 同じく `enabledPlugins` に `"kei@kei-lang": true`
3. Claude Code を再起動 → 新セッションのスキル一覧に `kei` が並ぶことを目視

ローカル開発版を使いたい(マージ前の SKILL.md を試したい)場合は `source` を
`{ "source": "directory", "path": "<absolute path to kei-lang>" }` にする。

### 条件 2: Opus が書く委譲プロンプト本文に Kei 文法を一切書かない

ドッグフードは「**取説経由で AI が書けるか**」の検証。条件 1 で繋いだ取説(SKILL.md / MCP の
4 ツール)からサブエージェントが文法を引くのは想定通り・**歓迎**。禁止しているのは、その手前で
**Opus が prompt 本文に Kei の語彙(`func` / `requires` / `Result` / `Option` / `uses` 等)を
直接埋め込むこと**。Opus がプロンプトに書いた瞬間、サブエージェントは取説を引かずに済んで
しまい、ドッグフードが汚染される(取説経由の摩擦が観測できなくなる)。

許されるのは:
- 「Kei は TypeScript にトランスパイルされる言語」程度の存在確認
- お題のドメイン要件(自然言語のみ。コード例・型名・キーワードゼロ)
- 完了条件(`kei_check` がエラーゼロを返すこと)
- フィードバックフォーマット

「取説の入口を案内する一文」は不要(plugin が enable されていればスキル一覧から
サブエージェントが自分で `kei` を呼ぶ。**Opus がそれすら言わない** 方が初見再現度が高い)。

NG 例(Opus prompt 本文):
- 「`requires` で前提を書いてください」 → 構文を教えている
- 「`Result<T, E>` を返してください」 → 型を教えている
- 「`uses Database.Read` でエフェクトを宣言します」 → エフェクトモデルを教えている
- 「サンプル: \`func foo(x: Int) -> Int { ... }\`」 → コード例を埋め込んでいる

OK 例:
- 「在庫から N 個引いて、引けたかどうかと残数を返す関数を 1 つ書いてください」
- 「金額が 0 より大、かつ上限を超えないことを検査する関数を書いてください」

### 条件 3: 実証後にサブエージェントから構造化フィードバックを必ず回収する

サブエージェントの最終応答に以下 6 項目を含めるよう **プロンプトで明示的に要求** する。
自由記述だけだとノイズになる。

1. **書いた .kei コード**(コードブロック)
2. **`kei_check` の最終出力**(エラーゼロなら `[]`、断念なら最後の Diagnostic)
3. **ツール呼び出し回数**: kei_spec / kei_examples / kei_check / kei_fmt それぞれ何回
4. **詰まった点**(順不同):
   - 取説に**あったが見つけにくかった**項目
   - 取説に**なくて推測で書いた**項目
   - Diagnostic は出たが**意味が分からなかった**コード
   - fix を適用したら**別の Diagnostic が連鎖**した
5. **助かった点**(良かった UX。改善のヒント源)
6. **総評**: 1〜10 で「初見で書きやすかったか」+ 一言

事実ベースで。誇張も遠慮も要らない、と前置きする。

---

## 手順

### 1. お題の確定(Opus 担当)

ユーザーから渡されればそれを使う。未指定なら Opus が 1〜2 個提案して合意を取る。
選ぶときの規約:

- Kei v0.X 射程内で、普通の業務で書きそうな小さな関数 1 つ
- ドメイン要件 1〜3 文。例: 「在庫から N 個引く / 金額を検証する / リストの合計と件数を返す」
- **コード例・使うべき機能名(`Result`・`requires` 等)・予想される型シグネチャを書かない**
- 完了は `kei_check` エラーゼロで機械判定

### 2. 委譲(Agent ツール)

- `subagent_type: claude`
- `model: sonnet`
- 下記テンプレートを self-contained に渡す

#### プロンプトテンプレート(厳密モード・基本形)

サブエージェントが**自力で** skill/MCP を発見して取説を引くまでが検証対象。Opus からは
ツール名や skill 名すら案内しない。これでハマるなら、それは「初見ユーザーが Kei に到達できない」
という valuable signal。

```text
あなたはこのプロジェクトで Kei という言語を使います。Kei は TypeScript に
トランスパイルされる言語です。このプロジェクトには Kei 用の取説と検証ツールが
セッションに繋がっています — どう使うかは自分で見つけてください。

**このプロンプトには意図的に Kei の中身(文法・型・キーワード)を書いていません。**

## お題

<ドメイン要件のみ。1〜3 文の自然言語。コード例・型名・キーワードゼロ>

## 完了条件

書いた .kei が Kei の意味検査(セッションに繋がっている検証ツール)でエラーゼロを返すこと。
断念しても構いません — その場合は「どこで詰まったか」が成果物です。

## 最終応答に必ず含めるもの(構造化フィードバック)

1. 書いた .kei コード(コードブロック)
2. 意味検査の最終出力(エラーゼロなら `[]`、断念なら最後の Diagnostic)
3. 使った取説 / ツールごとの呼び出し回数(skill 読み込み・MCP 各ツール・Read など全部)
4. 詰まった点(順不同):
   - 取説にあったが見つけにくかった項目
   - 取説になくて推測で書いた項目
   - Diagnostic は出たが意味が分からなかったコード
   - fix を適用したら別の Diagnostic が連鎖したケース
   - そもそも取説 / 検証ツールに辿り着くまでに迷ったか
5. 助かった点
6. 総評: 1〜10 で「初見で書きやすかったか」+ 一言

事実ベースで。誇張も遠慮も不要です。
```

#### ヒントモード(2 周目以降の比較用)

厳密モードで完全に詰まった、または「取説到達不可能性」が既に何度も観測されていて
今回は別の摩擦を測りたい場合のみ、以下 1 行を追加する(これ以上は足さない):

> このプロジェクトには Kei plugin のスキル(`kei`)と Kei MCP の 4 ツールが繋がっています。

厳密モードと**同じお題で**両方走らせると、ヒントの有無で発見コストの差分が測れる
(=取説への入口のコスト評価)。

### 3. フィードバックの振り分け(Opus 担当)

受け取ったフィードバックを以下のバケツに分けて、改善の行き先を決める:

| 観察 | 行先 |
|---|---|
| 「取説にあったが見つけにくい」 | `skills/kei/SKILL.md` の節順 / 見出し / 索引語見直し |
| 「取説になくて推測で書いた」 | spec / SKILL.md / examples への追記(spec が source of truth) |
| 「Diagnostic が意味不明」 | `spec/errors/<code>.md` 改稿 / `fixes[]` 拡充 |
| 「fix の連鎖」 | check の fix 生成ロジック / 事前のスタイル誘導 |
| 「ツールが必要な情報を返してくれない」 | `kei_spec` 索引拡充 / `kei_examples` の検索性 |
| 「kei_check の Diagnostic 表記が CLI 散文と JSON で違う」 | 仕様化 / `diagnostic-schema.md` 同期 |

**生のフィードバックは捨てない。** 改善前のスナップショットとして、git 上に
`docs/dogfood/<date>-<task>.md` 等で残すか、関連 Issue に貼る。改善後に**同じお題**で
再ドッグフードして差分を測れば、改善が効いたかが KPI として出る(回数減・総評上昇)。

### 4. 次バージョン Issue 化(オプション・人手承認必須)

スキル呼び出し時に `--create-issues <milestone>`(例: `--create-issues v0.5`)が
渡されたら、Step 3 の振り分け結果を Issue 候補に変換し、**人間承認を経て**から
`gh issue create` / `gh issue comment` する。**自動投稿は絶対に行わない**(初回
ノイズを防ぐため、承認ゲートを必ず通す)。

#### 4-a. 候補生成

Step 3 で振り分けた観測 1 項目 = Issue 候補 1 件。複数バケツに該当するなら最上位
(spec / SKILL.md → Diagnostic → fix → 索引)1 つに割り当てる。

各候補の構造:

```
title:     [dogfood/<bucket-slug>] <観測の短い要約 (30 字以内)>
body:      ## お題
           <Step 1 で確定したお題の原文 1-3 文>

           ## 観測した摩擦
           <フィードバックの該当項目を原文引用>

           ## 推奨改善先
           <振り分け表「行先」セルをそのまま転記>

           ## 関連
           - 元 dogfood セッションの最終応答(全文)
labels:    dogfood, from-v<元バージョン>, <bucket-slug>
milestone: <引数で指定>
severity:  high   = 取説/MCP に到達できず詰まった、Diagnostic が誤誘導
           medium = 取説に無くて推測した、fix の連鎖で迂回
           low    = UX 改善余地のみ
```

`bucket-slug` は振り分け表の左列を kebab-case 化
(`docs-discoverability` / `spec-completeness` / `diagnostic-clarity` /
`fix-chain` / `mcp-indexing` / `diagnostic-schema-sync`)。

#### 4-b. 既存 Issue との dedup

各候補について:

```
gh issue list --milestone <X> --state open --search "<title の主要キーワード>" \
  --json number,title
```

タイトル類似(主要キーワード 2 つ以上一致)があれば、その候補を **「Issue #N に
追記コメント」** に差し替える。本文は body をそのまま使用。dedup 判定はあくまで
ヒントなので、最終判断は人間承認に委ねる。

#### 4-c. 人間承認

候補を全件まとめて表示(create N 件、comment M 件)。各候補に番号を振り、以下を
返してもらう:

- `全部` — 全候補を実行
- `#1, #3 だけ` — 指定 ID のみ実行
- `全部却下` — 何もしない
- `#2 はタイトルを X に変えて` — 指定 ID をユーザー指示通り編集

承認待ちが完了するまで `gh issue create` / `gh issue comment` を**絶対に実行しない**。

#### 4-d. 実行

承認分のみ:

- create: `gh issue create --title "<title>" --body "<body>" --label "<labels>" --milestone <X>`
- comment: `gh issue comment <N> --body "<body>"`

各成功で URL を表示。失敗(milestone 不存在 / labels 不存在 / network)は continue せず
**即報告してユーザーの指示を仰ぐ**(失敗 Issue を後から手で削除させるより安全)。

#### 4-e. 最終応答

```
Issue 化: created [#101, #102], commented [#88], skipped [候補#4 はユーザー却下]
URLs:
- https://github.com/A-1ro/kei-lang/issues/101
- https://github.com/A-1ro/kei-lang/issues/102
- https://github.com/A-1ro/kei-lang/issues/88#issuecomment-...
```

#### 4-f. 守るべき制約

- `--create-issues` 引数が **無い場合は Step 4 全体をスキップ**(従来通り Step 3 で終わる)
- milestone が存在しないときは `gh api repos/.../milestones` で確認し、自動作成しない
  (milestone のスコープ定義は人間の意思決定)
- 元バージョン(`from-v<X>`)は **dogfood を実行した時点の最新リリースタグ**
  (`gh release view --json tagName --jq .tagName` で取得)。判別不能なら人間に聞く
- 同一 dogfood セッション内で重複候補(同じ観測を 2 件にまとめてしまった)が出たら
  4-a の段階で 1 件に統合する。Issue tracker の同一観測重複は dedup より前に潰す

---

## このリポジトリ(Kei)での注意事項

- **検査エンジンや fix を更新した直後**: サブエージェントが叩く `mcp__kei-mcp__kei_check` は
  MCP サーバー内蔵の検査(ビルド時固定版)を呼ぶ。`kei_mcp` を再ビルドしないと最新が
  反映されないので、変更後は `cargo build -p kei_mcp` を経てから委譲する。
- **examples / spec を追加した直後**: `kei_mcp` の `build.rs` が `spec/**/*.md` と
  `examples/**/*.kei` をビルド時に埋め込む。同じく `cargo build -p kei_mcp` を経ないと
  サブエージェントから新例 / 新節が見えない。
- **CLAUDE.md / HANDOFF.md は引かない**。これらは「プロジェクト持ち向け」で、
  Kei を使うエージェントには見えない前提。ここを引かないと書けないなら、その情報を
  spec / skill / examples に降ろす — それ自体がドッグフードで見つけたい摩擦。
- **plan-then-delegate との関係**: あちらは「実装計画を Opus が詰めて Sonnet に渡す」
  汎用パターン。kei-dogfood は **意図的にその逆** で、Opus が計画書を書いてはいけない
  (=Kei の中身に踏み込んだ瞬間に検証が壊れる)。両者を混同しない。
