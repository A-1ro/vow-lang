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

## PR #74: feat(check): resolve import boundary types (M20 / #55) — 2026-06-27

### Candidate: 解決不能 import は opaque(Ty::Unknown)に倒すことが「健全性ギャップ」として設計上 OK
**Why this matters for HANDOFF.md**: `ModuleResolver::resolve` が `None` を返すケース(パースエラー・ファイル未存在)をエラーにせず opaque 扱いにするのは意図的な段階移行の設計判断であり、将来の「完全解決モード」と混同しやすい。
**Draft entry** (lift verbatim if approved):
> `ModuleResolver::resolve` が `None` を返した import 名は `NameKind::Import`(opaque / `Ty::Unknown`)として扱い、フィールドアクセスや match 網羅性検査をスキップする。これは v0.1 の「単一ファイル検査」と等価なフォールバックであり、「解決失敗 → コンパイルエラー」ではない。`FsModuleResolver` でパースエラーや読み取り失敗が起きた場合も同様に `None` を返して consumer をブロックしない。この「致命的でない健全性ギャップ」は M20 の意図的な段階移行設計。将来の「strict 解決モード」(解決失敗を E1xxx にする)とは別トラック。

### Candidate: `FsModuleResolver` は project root を `module a.b.c` 宣言と入力ファイルパスから逆算する
**Why this matters for HANDOFF.md**: プロジェクト root の決定アルゴリズムが `module` 宣言の段数依存であることを知らないと、`module` 宣言の無いファイルや段数が合わないファイルで resolver が無効化される理由が分からなくなる。
**Draft entry** (lift verbatim if approved):
> `FsModuleResolver` は入力ファイル `<F>` の `module a.b.c` 宣言からプロジェクト root を逆算する: `<F>` の親ディレクトリを `path.len()` 段遡ったパスが root(`<root>/a/b/c.kei` 規約)。`module` 宣言が無い・段数が足りない場合は `derive_root` が `None` を返し、CLI は `NoopResolver`(従来の単一ファイル検査)にフォールバックする。この挙動は `kei_cli::check` の呼び出し側で決定する。

### Candidate: namespace alias (`import a.b as Db`) は M20 でも opaque のまま — 将来拡張の温床
**Why this matters for HANDOFF.md**: `import a.b as Db` が M20 で解決されない理由が `Env::build` のコメント 1 行に埋もれており、将来「なぜ `Db.X` の型が Unknown になるのか」と混乱するリスクがある。
**Draft entry** (lift verbatim if approved):
> `import a.b as Db` のような namespace alias は M20 では意図的に解決対象外とし、`NameKind::Import`(opaque)のまま据え置く。`Db.fetch(...)` のような `<alias>.<name>` 経由の呼び出しは型不明・エフェクト不明として扱われる(extern 宣言があれば照合)。`import a.b { X, Y }` の名前付き import のみ M20 の型解決対象。namespace alias の型解決は将来拡張として明示的に先送りされた(`Env::build` のコメント参照)。

### Candidate: `module_type_defs` は対象モジュール内の import を追跡しない — Ty::Unknown に倒す
**Why this matters for HANDOFF.md**: 対象モジュールが別モジュールから import した型をフィールドに使っていると、そのフィールドが `Ty::Unknown` になる。これは現時点での意図的な制限であり、将来の「transitive 解決」とは切り分けて理解する必要がある。
**Draft entry** (lift verbatim if approved):
> `imports::module_type_defs` は対象モジュール(解決先の `.kei`)内に書かれた `import` を追跡しない。対象モジュール内で import 由来の型を参照するフィールド・バリアントは `Ty::Unknown` に倒される。例: `import foo { Foo }; record Bar { x: Foo }` の場合、`Bar.x` は `Ty::Unknown`。これは意図的な制限(コメントに「深い検査は別途」と明記)。Consumer 側 check.rs はこの `Ty::Unknown` を opaque として受け入れる。
