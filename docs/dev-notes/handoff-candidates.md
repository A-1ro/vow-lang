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
