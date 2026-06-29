# HANDOFF.md candidates

`gh pr merge` 後の Sonnet hook が自動追記する候補集。HANDOFF.md に昇格
させたいエントリは人間レビューを経て本体に移し、ここから削除する。

## PR #71: chore: add Claude Code automation skills and hooks — 2026-06-27

> **Note**: 本 PR のマージで post-merge-handoff agent が初回発火したが、子セッションの
> permission(don't-ask mode で書き込み deny)により本ファイルへの追記に失敗した。
> 以下 4 候補は agent の最終応答(blocking error report)から手動で復元したもの。
> 同 PR #72 で permission allow を追加し、次回以降は自動追記される。

### Candidate: post-merge agent は type:command + claude --print ではなく type:agent を使う
**Why this matters for HANDOFF.md**: 外部 CLI (`claude -p`) が将来別課金になったときに自己改善ループが無効化されないようにする設計判断。
**Draft entry**:
> Hooks から Sonnet サブエージェントを呼ぶときは Claude Code 内蔵の `type: agent` を使う(Workflow と同じプール)。`type: command` で `claude --print` を呼ぶ案もあるが、CLI の課金体系が将来変わったときに hooks が無効化されるリスクがあるため避ける。長い prompt は別ファイル(`.claude/hooks/*.prompt.md`)に切り出し、agent に Read させる戦略。

### Candidate: pre-commit-ci.sh は cargo test 後に e2e package-lock.json を working tree のみ復元する
**Why this matters for HANDOFF.md**: `cargo test --workspace` が `tests/cli/projects/app/package-lock.json` と `tests/e2e/package-lock.json` を変更する副作用への暗黙対処を明文化する。
**Draft entry**:
> e2e テスト(`tests/e2e/`, `tests/cli/projects/app/`)は npm/npx を呼ぶ過程で lockfile を変更することがある。`cargo test --workspace` の後に `git status` を見ると意図せぬ差分が出る。pre-commit-ci hook は `git checkout --` で **working tree のみ** 復元する(staged 状態は触らない)。意図して lockfile を更新したい場合は事前に staging する規約。

### Candidate: .claude/settings.json はチェックイン(`.local.json` ではない)
**Why this matters for HANDOFF.md**: CLAUDE.md 不変条件「fmt/clippy/test 全パスが完了条件」と整合させるためのプロジェクト規律。
**Draft entry**:
> `.claude/settings.json` をチェックイン(`.gitignore` 対象の `.local.json` ではない)するのは、CLAUDE.md の「fmt/clippy/test 全パスが Milestone 完了条件」を Claude Code 経由の commit すべてに強制するため。これにより別マシン/将来の自分/共同編集者にも同じ品質ゲートが効く。「自分専用にしたい」場合だけ `.local.json` に分離する余地はあるが、本リポジトリではプロジェクト規律として共有版を採用。

### Candidate: kei-dogfood は plugin SKILL.md と MCP の **両方** 接続が必須
**Why this matters for HANDOFF.md**: 取説経由は 2 つあって、片方だけだとドッグフードが成立しないという設計上の前提を明示する。
**Draft entry**:
> Kei の「取説経由の正当な推論経路」は 2 つ — (a) plugin の `skills/kei/SKILL.md`(取説の入口)と (b) MCP の 4 ツール(対話的な検索・検証)。kei-dogfood スキルは両方の接続を必須条件として要求する。SKILL.md だけだと検査できず、MCP だけだとサブエージェントが「何があるか」を知らないまま走る。どちらが欠けてもドッグフードの結果が無意味になる。

## PR #72: fix(hooks): grant dev-notes write permission and recover PR #71 loop — 2026-06-27

### Candidate: type:agent 子セッションは親の permission を継承しない — 書き込みパスを明示 allow する必要がある
**Why this matters for HANDOFF.md**: hook が「動いているのに何も書かれない」状態になる最大の落とし穴であり、将来 hook を追加するたびに踏むリスクがある。
**Draft entry** (lift verbatim if approved):
> `type: agent` hook で起動する子セッションは、親セッションの permission を一切継承しない。don't-ask mode で Edit / Write / Bash の書き込みが暗黙 deny される。Hook に書き込み操作をさせる場合は `.claude/settings.json` の `permissions.allow` に対象パスを明示する(例: `"Edit(docs/dev-notes/**)"`, `"Write(docs/dev-notes/**)"`)。この設定が抜けていると hook は **静かに発火するが何も書かれない** 状態になり、デバッグが困難。blocking error は親セッションのトランスクリプトには届くが、通常の操作では気付きにくい。

### Candidate: hook 用の permissions.allow は最小スコープで付与する
**Why this matters for HANDOFF.md**: 将来 hook パスが増えるたびに「とりあえず `Bash(*)`」で広げようとする誘惑があるが、それは品質ゲートの意味を損なう。
**Draft entry** (lift verbatim if approved):
> Hook が書き込みを必要とするパスには **最小スコープ** の permission を付与する方針。例えば post-merge agent が `docs/dev-notes/` に書き込むなら `"Edit(docs/dev-notes/**)"` と `"Write(docs/dev-notes/**)"` だけを許可し、`Bash(*)`(全 Bash 許可)や `Edit(*)`(全編集許可)には広げない。settings.json はチェックイン対象なので、広い permission を入れると全共同編集者のセッションに影響する。

## PR #70: chore: bump version to 0.4.0 — 2026-06-27

(no design-decision candidates for this PR)

<!-- 判断根拠:
     PR #70 はバージョン文字列の機械的置換のみ。
     ただし以下 2 点は将来の混乱防止として参考記録を残す。

     (a) MCP golden (tests/mcp/*.response.json) の serverInfo.version は
         env!("CARGO_PKG_VERSION") 由来なので、ワークスペース version 変更時は
         UPDATE_GOLDEN=1 cargo test -p kei_mcp --test golden_mcp の再生成が必須。
         しかし PR body にそのまま記載されており、HANDOFF.md に昇格するほど
         埋もれた情報ではないと判断。

     (b) バージョン管理対象外: runtime/(独立 npm パッケージ) と editors/vscode は
         Cargo workspace version とは独立して管理される。skills/kei/SKILL.md の
         バージョン言及は機能 PR 側で更新する慣例(本 PR ではなく PR #69 が担当)。
         これらも PR body に明記されているため HANDOFF.md 昇格不要と判断。
-->

## PR #72 (re-check / PostToolUse audit session): kei-invariant-auditor M19 監査 — 2026-06-27

> **Note**: このセクションは `gh pr merge` ではなく `kei-invariant-auditor` セッション内の
> `PostToolUse`(Bash: `git diff --stat $(git merge-base main HEAD)..HEAD`)で発火した
> post-merge-handoff hook によって追記された。マージ対象 PR は未確定(M19 / #54 が WIP)。
> 最新マージ済み PR は #72(既記録)。以下は監査セッションが観察した設計判断の候補。

### Candidate: コメントは AST ノードに持たせず `ParseResult.comments` 副チャネルに退避する
**Why this matters for HANDOFF.md**: パーサ・フォーマッタ間の責務分離の根拠を知らないと、将来「なぜ AST にコメントがないのか」という疑問に誤った答えを出しやすい。
**Draft entry** (lift verbatim if approved):
> M19 以降、`//` 行コメントは `Comment` トークンとして採取されるが、AST ノードには **一切持たせない**。代わりに `ParseResult.comments` にソース順で並べる副チャネルを採用。理由: (1) コメントは文法上「どの AST ノードに属するか」が一意に定まらない(前の文末か次の文の前か)。(2) proptest や codegen などコメントに関心のない消費者が AST を使う経路では不要なデータを持ち込まない。(3) フォーマッタは行番号ベースで leading / trailing を自力で再構築できるため副チャネルで十分。新しくコメントを処理するコードを書くときは `ParseResult.comments` を参照し、AST ノードを拡張しようとしないこと。

### Candidate: `format_module` は意図的にコメントを失う(proptest 用純粋経路)
**Why this matters for HANDOFF.md**: `format_module` と `format_source` の使い分けを知らないと「コメントが消えるバグ」と誤解される。
**Draft entry** (lift verbatim if approved):
> `kei_fmt` には 2 つの公開 API がある。`format_module(&Module) -> String` はコメントを **意図的に失う**。proptest / codegen など純粋な AST 入力経路向けで、コメントを引数に取らない設計。`format_source(&str) -> Result<String, _>` は内部でパースし `ParseResult.comments` を使ってコメントを保持する。CLI の `kei fmt` は後者を経由する。`format_module` を使って「コメントが消えた」という報告があっても仕様どおりなので修正しないこと。

### Candidate: フォーマッタの冪等条件は M19 以降も変わらない — コメントは位置が変わっても内容は保持
**Why this matters for HANDOFF.md**: コメント付きソースに対する冪等性の定義が明文化されていないと、将来のテスト設計が曖昧になる。
**Draft entry** (lift verbatim if approved):
> `kei_fmt` の冪等条件 `fmt(fmt(x)) == fmt(x)` は M19 以降もコメント付きソースで成立する必要がある。ただし「コメントが元のコラム位置に完全復元される」保証はない。leading コメントはインデントが揃え直され、trailing コメントは 1 スペースに正規化される。コメントの **テキスト内容** は失われないが、**位置** は整形後の正規形に変わる。引数並びや式中間のコメントは v0.4 では「次のアンカーノードの leading」に寄せられる(完全な位置復元は将来の拡張)。これを踏まえて golden test は整形後の位置で expected を書くこと。

## PR #79: chore(hooks): auto-run kei-code-review on gh pr create — 2026-06-28

### Candidate: auto-fix の対象は `CONFIRMED` かつ `kei-invariants` / `correctness` のみ — `pitfalls` / `cleanup` / `altitude` は人間判断に委ねる
**Why this matters for HANDOFF.md**: auto-fix ループがどの findings を自動適用してよいかの判断基準が明文化されていないと、将来 hook を改修した際に境界が曖昧になりリグレッションを招く。
**Draft entry** (lift verbatim if approved):
> `post-pr-create-review` hook の自動修正フィルタは `verdict == "CONFIRMED"` かつ `angle ∈ {kei-invariants, correctness}` の findings のみを対象にする。`PLAUSIBLE` は絶対に自動適用しない。`pitfalls`・`cleanup`・`altitude` の角度は主観・文脈依存が大きく機械判断に向かないため除外。この設計により「確実に壊れている箇所だけを直し、スタイルや改善提案は inline comment のみ」という分離が保たれる。

### Candidate: auto-fix で `git add -A` / `git add .` を禁止する理由はあらかじめ hook 本体に明記
**Why this matters for HANDOFF.md**: PR #71 の dev-notes 教訓「`cargo test` 後に `git add -A` すると e2e lockfile drift を拾う」が hook の hard rule に直結しているが、その因果を知らないと将来の改修者が規則の意図を誤解する。
**Draft entry** (lift verbatim if approved):
> hook の自動コミット処理で `git add -A` / `git add .` / `git add :/` を禁じている理由: `cargo test --workspace` は `tests/e2e/` と `tests/cli/projects/app/` の `package-lock.json` を副作用で変更することがあり、`git add -A` するとそのドリフトをコミットに混入させてしまう。auto-fix hook は `git add <file1> <file2> ...` と **変更した特定ファイルのみ** をステージする。この規則は `pre-commit-ci.sh` の lockfile 復元処理と対になっている(pre-commit 側は working tree を復元するが staged は触らない)。

### Candidate: post-pr-create-review hook のスキップ条件(draft / バージョンバンプ / dependabot)を hook prompt 本体で定義する理由
**Why this matters for HANDOFF.md**: hook が「なぜ発火したのに何もしなかったのか」が外から見えにくいため、skip 判断ロジックの所在を明文化しておく必要がある。
**Draft entry** (lift verbatim if approved):
> `post-pr-create-review` は以下の PR を明示的にスキップする: (1) `isDraft == true`(レビュー準備未完了)、(2) タイトルが `^chore: bump version` で始まるリリースバンプ PR(機械的変更なので review 不要)、(3) タイトルが `chore(deps)` で始まるか `author.login == "dependabot[bot]"`(依存更新 PR)。スキップ判断は hook prompt ファイル(`.claude/hooks/post-pr-create-review.prompt.md`)内に記述され、hook の final reply に `"skipped: <reason>"` が出力される。hook log を確認するときはこの文字列を探す。

### Candidate: 人間レビュー必須サーフェスのリストは hook と HANDOFF.md で共有されるべき
**Why this matters for HANDOFF.md**: `spec/`・`tests/golden/`・`HANDOFF.md`・`.claude/settings.json` など「auto-fix の対象外とする」ファイル群のリストが hook prompt にしか存在せず、HANDOFF.md 読者には見えていない。
**Draft entry** (lift verbatim if approved):
> auto-fix ループが絶対に書き換えない「人間レビュー必須サーフェス」: `spec/`、`tests/golden/`、`.github/`、`.claude/settings.json`、`.claude/workflows/`、`CLAUDE.md`、`ARCHITECTURE.md`、`HANDOFF.md`、`Cargo.lock`。CONFIRMED findings がこれらを指していても inline comment のみ投稿し、Edit は行わない。将来 hook を改修する際もこのリストを hook prompt と HANDOFF.md で同期すること。

## PR #81: feat(skills): kei-dogfood — auto-file next-version Issues from feedback — 2026-06-28

### Candidate: kei-dogfood Step 4 の自動投稿禁止は OS 権限プロンプトを二段目防壁として利用する設計
**Why this matters for HANDOFF.md**: LLM が hard rule を violation しようとした場合の防衛戦略として、`permissions.allow` に意図的にエントリを追加しない設計判断が含まれている。
**Draft entry** (lift verbatim if approved):
> kei-dogfood Step 4 の Issue 化では `gh issue create` / `gh issue comment` を **承認ゲートを通過するまで絶対に実行しない** hard rule がある。SKILL.md にルールを書くだけでなく、`.claude/settings.json` の `permissions.allow` に `Bash(gh issue create:*)` / `Bash(gh issue comment:*)` を **意図的に追加していない** ことで OS レベルの permission prompt を二段目の防壁にしている。承認済みユーザーは prompt に `y` を返すだけで済むが、未承認の violation は OS 段で止まる。将来 kei-dogfood の権限拡張を検討する際はこの二段防衛の意図を壊さないこと。

### Candidate: `gh issue list --milestone` はタイトル文字列を直接受け付ける(number 変換不要)
**Why this matters for HANDOFF.md**: GitHub CLI の `--milestone` が title を受け付けることは公式ドキュメントに目立たない形でしか記載されておらず、将来の dedup 処理実装者が「milestone number を先に取得しなければならない」と誤解するリスクがある。
**Draft entry** (lift verbatim if approved):
> `gh issue list --milestone <X>` は milestone の **title**（例: `v0.5`）を直接受け付ける。`gh api repos/.../milestones` で number を引いてから渡す必要はない。kei-dogfood Step 4-b の dedup 処理はこの挙動を前提にしている。将来 GitHub CLI のバージョンが変わって挙動が変わった場合は Step 4-b を修正する必要がある。

### Candidate: `gh issue create` の `--label` は複数フラグを繰り返す — カンマ区切りは単一ラベル扱い
**Why this matters for HANDOFF.md**: カンマ区切りで複数ラベルを指定できると思い込む実装ミスが起きやすく、Issue が意図したラベルなしで立つ。
**Draft entry** (lift verbatim if approved):
> `gh issue create` でラベルを複数付けるときは `--label <l1> --label <l2>` のように **フラグを繰り返す**。`--label "dogfood,severity:high"` のようにカンマ区切りで 1 フラグに渡すと単一ラベル扱いになり、カンマを含むラベル名で検索されて失敗するか、意図しないラベルが付く。kei-dogfood Step 4-d はこの挙動を前提として設計されている。

### Candidate: Step 4 の gh 呼び出しは non-zero exit で即 halt-and-report する — 部分成功より安全側に倒す
**Why this matters for HANDOFF.md**: "半分だけ立った Issue" を後で整理するコストが高いため、all-or-nothing ではなく halt-at-first-error を選んだ設計判断。
**Draft entry** (lift verbatim if approved):
> kei-dogfood Step 4-d の `gh issue create` / `gh issue comment` は、**いずれかの呼び出しが non-zero exit したら以後の処理を即停止**してユーザーに報告する。milestone / labels 不存在、rate limit、auth 切れ、network、`--label` typo、HTTP 5xx など要因を問わず一律 halt-and-report。「部分的に Issue が立って残りはエラー」の状態はユーザーに後始末を押し付けるため避ける。承認後に失敗したときは全候補を再表示してから再実行してもらう設計を想定。

## PR #82: chore(skills): plan-then-delegate を実装タスクで常時発火に緩める — 2026-06-28

### Candidate: plan-then-delegate のトリガを「明示キーワード」から「コード編集を伴う指示全般」に拡大した理由
**Why this matters for HANDOFF.md**: 「なぜ skill description がこんなに広いのか」と感じた将来の改修者が、誤って以前の narrow trigger に戻す変更を入れないよう意図を明文化しておく必要がある。
**Draft entry** (lift verbatim if approved):
> `plan-then-delegate` の発火トリガは当初「sonnet に任せる」「ハンドオフ」などの明示的な委譲キーワード中心だった。実装タスクでも素通りされることが多く、二段委譲の恩恵(Opus の context 節約・Sonnet の速さ活用)を受けられないケースが頻発したため、「コード編集を伴う指示が来たら原則として常に発火」する設計に変更した。代表トリガ語: 実装 / 修正 / fix / 追加 / リファクタ / レビュー対応。除外は探索的タスクと golden/spec 判断が頻発する作業の 2 ケースのみに絞り、「1 ファイル数行だから委譲オーバーヘッドが大きい」という除外理由は撤廃した。

### Candidate: 「対応」ではなく「レビュー対応」に限定したトリガ絞り込みの経緯
**Why this matters for HANDOFF.md**: 「対応」という動詞は日本語として汎用的すぎ、コード編集を意図しない文脈(「質問に対応して」「エラーに対応した設計案を提示して」)でも skill が誤発火するリスクがあることを知らないと、将来ふたたび「対応」に戻す変更が入りうる。
**Draft entry** (lift verbatim if approved):
> `plan-then-delegate` のトリガ語として「対応」を追加したあと、コードレビュー指摘(PR #82 / pitfalls 角度)により「レビュー対応」に限定した。理由: 「対応」は「質問に対応して」「エラーに対応した設計案を提示して」のようにコード編集を伴わない文脈でも頻出し、skill が誤発火するリスクが高い。複合語「レビュー対応」に絞ることでコードレビューコメントの修正という特定用途だけをカバーし、「実装」「修正」「fix」「追加」「リファクタ」の既存トリガで残りのカバレッジを担保している。

## PR #83 (PostToolUse audit — old_counter correctness): feat: v0.4 remaining — M24 stock e2e + M25 lambdas + M26 Money notice — 2026-06-28

> **Note**: このセクションは kei-code-review verifier セッション内の `PostToolUse`
> (Bash: `grep -n "old_counter\|old\$\|kei\$old\|\"old\"" .../crates/kei_emit/src/emit.rs | head -40`)
> で発火した post-merge-handoff hook によって追記された。PR #83 は現時点で OPEN。
> 最新マージ済み PR は #82(上記セクションに記録)。以下は verifier が `old_counter` /
> `kei$old$N` の実装を確認した際に観察した設計判断の候補。

### Candidate: `forbid_old_capturing_lambda_param` の `refs_any` ガードは lambda param を参照しない `old(...)` を検出できない — emit の `collect_old_exprs` 停止と非対称になる危険

**Why this matters for HANDOFF.md**: `refs_any` が false を返しても `emit_call` は lambda body 内で `old(...)` を見て `old_counter` を進めるため、`kei$old$N` への undeclared 参照が TS に吐き出されて実行時 ReferenceError になる。「check が通った = emit が安全」という前提が崩れるケースの存在を知らないと、将来の実装者が `collect_old_exprs` の lambda 停止を「防御的すぎる」として外してしまいやすい。
**Draft entry** (lift verbatim if approved):
> `check.rs` の `forbid_old_capturing_lambda_param` は `refs_any(expr, lambda_params)` で **lambda param を参照する** `old(e)` だけを KEI-E4002 で弾く。しかし `old(Database.maxLimit())` や `old(42)` のように lambda param を参照しない `old(...)` は `refs_any` が false を返すため check をすり抜ける。一方 `emit.rs` の `collect_old_exprs` は lambda 境界で walk を **無条件停止** するため、これらの式は関数入口の `const kei$old$N = ...` に bind されない。それでも `emit_call`(emit.rs:1028–1032、`old_counter` 0→1 に進める)は lambda body 内で `name == "old"` を見て `kei$old$0` をインライン参照する。結果として TS 実行時に `ReferenceError: kei$old$0 is not defined` が発生する。修正方針: `forbid_old_capturing_lambda_param` の対象を「lambda param を参照するかどうかに依らず、lambda body 内の全 `old(...)` 呼び出し」に広げる。あるいは `collect_old_exprs` の停止を緩めて lambda param を参照しない `old(e)` は lambda body 内でも lift する。いずれの方針でも `err_contract_old_lambda_param.expected.json` golden を更新し E4002 のカバレッジを広げること。

## PR #83 (PostToolUse audit — runtest.mjs ReferenceError 実証): feat: v0.4 remaining — M24 stock e2e + M25 lambdas + M26 Money notice — 2026-06-28

> **Note**: このセクションは同セッション(73569efa)内の別 `PostToolUse`
> (Bash: `node runtest.mjs` → `threw: ReferenceError kei$old$0 is not defined`)
> で発火した post-merge-handoff hook によって追記された。直前のセクション
> (grep audit)と同一 PR #83 レビューセッションの続き。最新マージ済み PR は #82。
> 以下は verifier が実際に kei_emit でトランスパイルした TS 出力と
> Node.js 実行で ReferenceError を確認した際の補足設計判断。

### Candidate: kei_emit が生成する TS の IIFE 本体と ensures チェック部の `old` 参照が分裂する構造的問題

**Why this matters for HANDOFF.md**: 実トランスパイル出力を見ると IIFE 本体(`const kei$result = ...`)では `Database.maxLimit()` を直接呼ぶのに対し、ensures チェック部(`xs.every((p) => p < kei$old$0)`)は存在しない `kei$old$0` を参照するという **二重真実** 状態が生まれる。どちらの半分だけ見ても問題に気付けないため、kei_emit の出力を通して両箇所を並べて見る習慣が必要。
**Draft entry** (lift verbatim if approved):
> `kei_emit` が `func f(xs) ensures xs.all(p => p < old(Database.maxLimit()))` をトランスパイルすると、以下のような TS が生成される:
> ```ts
> export function allBelowLimit(xs: readonly number[]): boolean {
>   const kei$result = ((): boolean => {
>     return xs.every((p) => p < Database.maxLimit());  // ← IIFE 本体: old なし
>   })();
>   if (!(kei$result === xs.every((p) => p < kei$old$0))) {  // ← ensures: kei$old$0 が未宣言!
>     throw new KeiContractViolation({ ... });
>   }
>   return kei$result;
> }
> ```
> IIFE 本体は `collect_old_exprs` が lambda 境界で停止するため `Database.maxLimit()` を lift せず直接呼び出す。ensures チェック部は `emit_call` が `old_counter` を進めて `kei$old$0` を参照するが、`const kei$old$0 = ...` は関数先頭に存在しない。Node.js で実行すると `ReferenceError: kei$old$0 is not defined` が throw される。PBT(`--generative`)はコンビネータ引数の lambda のみ eval するため ensures のこの経路を通らず、generative グリーンのまま本番 ReferenceError になる。

## PR #83 (PostToolUse audit — emit.rs grep / fix design): feat: v0.4 remaining — M24 stock e2e + M25 lambdas + M26 Money notice — 2026-06-28

> **Note**: このセクションは同セッション(73569efa)内の別 `PostToolUse`
> (Bash: `grep -n "old_counter\|name == \"old\"\|kei\\$old" .../emit.rs | head -50`
> → ugrep escape error で失敗)で発火した post-merge-handoff hook によって追記された。
> 直前 2 セクション(old_counter correctness / runtest.mjs 実証)と同一 PR #83 レビュー
> セッションの続き。grep は失敗したが、同セッションで確認した PR #83 の diff から
> 「N3 修正がどの選択肢を採り、なぜか」という設計判断が読み取れるため候補として記録する。

### Candidate: N3 修正は emit 側を緩めず check 側を強化する方針を選択した理由

**Why this matters for HANDOFF.md**: 前 2 セクションで記録した `old(...)` + lambda の ReferenceError バグに対し、修正方針は「`collect_old_exprs` の停止条件を緩める(emit 側)」ではなく「`forbid_old_inside_lambda_body` で check 側を強化する(check 側)」を選んだ。この選択の根拠が明文化されていないと、将来の実装者が emit 側 の停止を「過防衛」と見て外してしまいやすい。
**Draft entry** (lift verbatim if approved):
> PR #83 (N3 / M25) で採用した修正方針: lambda body 内の `old(...)` は **check 側で一律 KEI-E4002 を出す**(`forbid_old_inside_lambda_body` 関数)。emit 側の `collect_old_exprs` lambda 境界停止は **そのまま維持** し、二段防御として残す。emit 側停止を緩める(「lambda param を参照しない `old(e)` は lift を許す」)案も検討されたが却下された。理由: (a) `old(e)` は「関数入口で 1 回評価」・lambda body は「呼び出しごとに評価」という **時相が根本的に噛み合わない** ため、emit 側でどう lift しても契約の意味論が崩れる。(b) check が通った入力で emit が壊れる二重真実状態そのものを排除する方が、将来の emit 実装者の認知負荷が低い。この二段防御(check 禁止 + emit 停止)を外す変更は、どちらの層が何を守っているかを理解してから行うこと。

### Candidate: `lambda_floor` の save/restore パターンはネストラムダ用であり `Option<usize>` は「深さ」ではなく「有無」を表す

**Why this matters for HANDOFF.md**: `lambda_floor: Option<usize>` を見ると「ネスト深度カウンタ」に見えるが実際は「現在ラムダ中かどうかのフラグ兼 scopes のインデックス」であり、深さはスコープスタックの長さで暗黙に表現される。この区別が明文化されていないと、将来の実装者が `lambda_floor` を `usize` のカウンタとして扱い誤ったスコープ境界を生成しやすい。
**Draft entry** (lift verbatim if approved):
> `FnChecker.lambda_floor: Option<usize>` は「現在コンビネータ引数ラムダの body 内にいるか」を示すフラグ兼スコープインデックス。`None` はラムダ外、`Some(i)` は `self.scopes[i..]` だけを `lookup_scope` の参照対象にすることで外側関数スコープのキャプチャを禁止する。**ネストの深さは表現しない**—内側ラムダの `check_combinator_lambda_arg` 呼び出し時に `prev_floor = self.lambda_floor` を退避し、新しい `Some(self.scopes.len() - 1)` をセットして返り際に復元する save/restore パターンを使う。これにより `fold(0, (acc, xs) => xs.fold(0, (a, x) => a + x))` のようなネストラムダでも各層が独立したキャプチャ禁止スコープを持てる。将来このフィールドを深さカウンタに転用しようとしないこと。

## PR #83 (PostToolUse audit — requires_old_lambda test fixture): feat: v0.4 remaining — M24 stock e2e + M25 lambdas + M26 Money notice — 2026-06-28

> **Note**: このセクションは同セッション(73569efa)内の `PostToolUse`
> (Bash: `cat > .../scratchpad/requires_old_lambda.kei` — `requires xs.all(p => old(p.qty) > 0)` の
> テストフィクスチャ作成)で発火した post-merge-handoff hook によって追記された。
> 直前 3 セクション(old_counter correctness / runtest.mjs 実証 / emit.rs grep)と同一
> PR #83 レビューセッションの続き。最新マージ済み PR は #82。
> 以下は `requires` 節での `old(...)` + lambda の二重エラー問題を実証するフィクスチャ作成時に
> 観察した設計判断の候補。

### Candidate: `requires` 節内の `old(...)` は「ensures 外で old を使った」エラーと「lambda 内で old を使った」エラーが同一スパンから二重に発火する

**Why this matters for HANDOFF.md**: `requires xs.all(p => old(p.qty) > 0)` という入力に対して、`forbid_old_inside_lambda_body` (Ensures/Requires 共通発火)と既存の「old は ensures 節のみ合法」チェックが同一スパンを二重に報告するため、ユーザーに重複した KEI-E4002 が届き、fix 提案(`let prev = old(seed)` を ensures 前に記述)が誤解を招く。requires mode では「old を使うな」が正解であり「old をラムダ外に出す」という fix は無意味。
**Draft entry** (lift verbatim if approved):
> `check.rs` の `forbid_old_inside_lambda_body` は `ContractMode::Requires` でも `ContractMode::Ensures` でも発火する設計になっている。しかし `requires` 節で `old(...)` を使うと、さらに既存の「old は ensures 節でのみ使用可能」チェックも同一スパンに KEI-E4002 を出すため、同一入力から **2 つの E4002 エラー** が重複して報告される。`requires xs.all(p => old(p.qty) > 0)` のような入力では: (1) 「old は ensures 節でのみ使用可能」— スパン `old(p.qty)` (2) 「lambda body 内で old は使用不可」— スパン `old(p.qty)` が重複する。fix 提案として表示される `let prev = old(seed); xs.all(...)` は ensures-mode 向けのテンプレートであり requires-mode では無意味な誤誘導になる。修正方針: `forbid_old_inside_lambda_body` は `ContractMode::Ensures` でのみ発火させる(requires では既存の old-in-requires エラーが上位で弾く)か、old-in-requires チェックを早期リターンにして lambda チェックを抑制する。golden `err_contract_old_lambda_nonparam.expected.json` の diagnostics 配列が 1 件か 2 件かでどちらの方針が採られたかを確認できる。

## PR #84: chore: bump version to 0.4.2 — 2026-06-29

(no design-decision candidates for this PR)

<!-- 判断根拠:
     PR #84 はバージョン文字列の機械的置換(Cargo.toml / Cargo.lock / plugin.json /
     marketplace.json / MCP golden 3 件 = 0.4.1 → 0.4.2)と、skills/kei/SKILL.md への
     M25/M26 ドキュメント追記が中心。

     SKILL.md の実質追加内容:
     (a) コンビネータ引数位置限定ラムダ(v0.4 / M25)— lambda body 内 old() 禁止 (KEI-E4002)・
         lambda param 名が TS 予約語衝突 (KEI-E2001)・let f = (lambda) は引き続き KEI-E2001
     (b) M26: spec §2.4 / §2.5 の新設と stock_direct.kei の追加

     これらはすべて PR #83 の post-merge セクション(上記に記録済み)で設計判断として
     詳細に捕捉済みのため、重複登録は不要と判断。

     MCP golden の version 文字列は env!("CARGO_PKG_VERSION") 由来のため
     workspace version 変更時に UPDATE_GOLDEN=1 cargo test -p kei_mcp 再生成が必要だが、
     これも PR #70 の判断根拠コメントで記録済み。

     runtime/ と editors/vscode は Cargo workspace version とは独立して管理される慣例も
     本 PR body に明記されており HANDOFF.md 昇格不要。
-->

## PR #84 (PostToolUse audit — issue state check): chore: bump version to 0.4.2 — 2026-06-29

> **Note**: このセクションはセッション 73569efa 内の `PostToolUse`
> (Bash: `for n in 54 55 56 57 58 59 60 61 62; do echo -n "#$n: "; gh issue view $n --json state --jq '.state'; done`)
> で発火した post-merge-handoff hook によって追記された。最新マージ済み PR は #84(上記に記録済み)。
> 以下はセッションが関連 Issue の状態を確認した際に観察した設計判断の候補。

### Candidate: M24 / M25 / M26 を実装した PR #83 がマージされてもその実装 Issue が自動クローズされない

**Why this matters for HANDOFF.md**: PR body に `#56` / `#59` / `#61` への言及があるにもかかわらず GitHub が Issue を自動クローズしなかった場合、ロードマップ進捗が実態と乖離しているように見える。自動クローズの条件(キーワード `Closes #N` or `Fixes #N`)を満たしていなかった可能性が高く、将来の PR テンプレート設計に影響する。
**Draft entry** (lift verbatim if approved):
> GitHub の Issue 自動クローズは PR body に `Closes #N` / `Fixes #N` / `Resolves #N` キーワードが含まれる場合のみ機能する。PR #83(feat: v0.4 remaining)は `#56` / `#59` / `#61` を参照したが、マージ後の Issue 状態確認(セッション 73569efa)で Issue #56(M24)・#59(M25)・#61(M26)が OPEN のままだった。マージで自動クローズさせたい Issue は PR body に `Closes #<N>` を明記する規律を徹底すること。また Milestone の Issue 完了状態はロードマップ(`docs/kei-roadmap-v0.4.md`)の `✅` 記号と GitHub Issue の状態の両方で確認することが必要(片方だけ見ると不一致が生じる)。

## PR #83: feat: v0.4 remaining — M24 stock e2e + M25 lambdas + M26 Money notice — 2026-06-28

> **Note**: 直前の 4 セクション(old_counter correctness / runtest.mjs 実証 / emit.rs grep / requires_old_lambda fixture)は `gh pr merge 83` **前** の code review セッション内 `PostToolUse` で追記されたもの。本セクションは `gh pr merge 83 --squash --delete-branch --auto` の完了を受けて発火したマージ後フック(セッション 73569efa)が追記した正式な post-merge 候補集。

### Candidate: M25 — lambda は「値」ではなく「コンビネータ引数位置の構文糖」として設計された(案 2 維持)
**Why this matters for HANDOFF.md**: `let f = (p => p.id)` が KEI-E2001 になる理由を知らないと、「型推論が足りないバグ」と誤解されやすい。第一級関数値を将来追加するときも、この決定との整合を意識する必要がある。
**Draft entry** (lift verbatim if approved):
> M25(#59 / v0.4)で追加したラムダ構文 `p => expr` / `(a, b) => expr` は、`List<T>` の `map`/`filter`/`fold`/`all`/`any` **引数位置でのみ** 合法な構文糖であり、値として保存・再利用することはできない。`let f = (p => p.id)` は依然として KEI-E2001(型不一致)になる。これは M9(spec §10)で合意した「案 2: 第一級関数値を導入しない」方針の継続。ラムダの `infer` arm が「コンビネータ引数以外で出現した場合のみ」到達するように設計されており、`check_combinator_fn_arg` がラムダを別経路で先処理してこの arm には降りない。将来第一級関数値を追加する場合は `infer` の Lambda arm を書き換え、`check_combinator_fn_arg` の分岐も見直すこと。

### Candidate: M25 — TS 予約語チェックをラムダパラメータの **check 段階**で弾く理由(emit 段ではなく)
**Why this matters for HANDOFF.md**: 「なぜ Kei 自体は予約していない `class` や `var` をラムダパラメータで弾くのか」が明文化されていないと、将来の実装者がこれを誤ったエラーとして削除しやすい。
**Draft entry** (lift verbatim if approved):
> lambda パラメータ名が TypeScript 予約語(`class`, `var`, `null`, `this`, `function`, `delete`, `typeof`, `let`, `await`, `async` 等)と衝突する場合、`check.rs` が check 段階で KEI-E2001 を出す([4] / M25)。Kei 自体は `class` を予約語と定義していないが、emit 後の `(class) => ...` は `tsc` が parse 不能になる。**emit 段で弾かない**のは、TS コンパイルエラーより明確な Kei レベルの診断を届けるため。検出単位は v0.4 では lambda パラメータのみ(将来 `let` / 関数パラメータ全般への拡張が議論されたが、スコープ外として延期)。予約語リストは ES2022 + TS strict mode を網羅した `is_ts_reserved_word()` ヘルパで管理する。

### Candidate: M25 — 0 引数 `() =>` はパーサが `Expr::Error` sentinel を返し check まで届かない
**Why this matters for HANDOFF.md**: `check_combinator_lambda_arg` に「0 引数ラムダが来ない前提」の `debug_assert` があり、その理由を知らないと将来の変更者が assert を外してしまいやすい。また 0 引数ラムダのエラーが golden にどのレイヤで記録されるかを誤解しやすい。
**Draft entry** (lift verbatim if approved):
> 0 引数ラムダ `() => expr` はパーサ段階で `KEI-E0101` を出し `Expr::Error` sentinel を返す(N0 / M25)。`Expr::Error` は下流の walker が no-op で扱うため、`check_combinator_lambda_arg` には 1 個以上のパラメータを持つ `Expr::Lambda` のみが届く。`check_combinator_lambda_arg` 先頭の `debug_assert!(!lparams.is_empty(), ...)` はこの前提を明示するガード。0 引数ラムダの golden は `tests/golden/syntax/err_lambda_zero_params.*` に記録されており、check レイヤの golden ではなく syntax レイヤに分類される。将来 lambda 引数の arity 検査を拡張するときは、0 引数 case のみ `Expr::Error` 経路であることを忘れないこと。

### Candidate: M24 — `extern query` はスタブで純粋観測子として実装し、`old()` との組み合わせで外部状態事後条件 e2e を可能にする
**Why this matters for HANDOFF.md**: `extern query` がなぜ `uses` エフェクトを持たないのか、またなぜそれが `old()` との組み合わせで機能するかを知らないと、将来の在庫/残高ドメインの e2e 設計が外部観測子を誤って `uses Database.Read` にしてしまう。
**Draft entry** (lift verbatim if approved):
> M24(#56)の在庫ドメイン e2e(`examples/contracts/stock_direct.kei`)は `extern query Database.quantityOf(product: ProductId) -> Int` を使う。`extern query` は副作用(uses)を持たない純粋観測子として定義され、`ensures Database.quantityOf(product) == old(Database.quantityOf(product)) - amount` のように `old(外部状態観測)` として事後条件に書ける。スタブ側(`tests/e2e/stubs/database.ts`)では状態を保持し `quantityOf` が読み取りのみ行う純粋関数として実装される。`extern query` を `uses` 付きで定義すると `ensures` 節内で副作用エラーが出るため、読み取り専用の外部状態にはかならず `extern query`(uses なし)を選ぶこと。反例 3 種(off-by-one / forgot / wrong-id)は `KeiContractViolation(clause: "ensures")` として runtime で検出される設計を確認済み。

### Candidate: M26 — `Money` / `core.money` は spec 上の架空型であり stdlib に実装されていない
**Why this matters for HANDOFF.md**: spec §2.1–§2.3 の例に登場する `Money` や `core.money` が実際には存在しないことを知らないと、実プロジェクトでこれをインポートしようとしてコンパイルエラーに悩む。
**Draft entry** (lift verbatim if approved):
> M26(#61 / v0.4)で `spec/kei-spec-v0.1.md §2.4` に明記: `Money` / `core.money` は spec §2.1–§2.3 の例で登場する **説明用の架空型・架空モジュール** であり、stdlib に実装されていない。実プロジェクトでは (a) `Int`(最小通貨単位)をそのまま使う、または (b) `type Money = Int tagged "Money"` を自前定義する。`Money.zero` のような静的メンバアクセスは Kei 構文にないため `Money(0)` で構築すること。`examples/contracts/withdraw.kei` と `examples/effects/transfer.kei` は架空 Money 例として残り、e2e は `tests/e2e/stubs/core/money.ts` の差し替えで動く — 実装プロジェクトのひな型としては使わないこと。固定小数点(`Decimal`)と `core.money` の実在化は v0.5+ で別途検討予定。
