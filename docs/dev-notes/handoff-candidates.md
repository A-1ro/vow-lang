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
