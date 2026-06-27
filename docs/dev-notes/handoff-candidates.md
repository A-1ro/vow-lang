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

## PR #75: feat(syntax,check,emit,fmt): list literals and explicit tagged ctors (M22 / #57) — 2026-06-27

### Candidate: 空リスト `[]` は `Ty::List(Unknown)` — エラーではなく遅延具体化
**Why this matters for HANDOFF.md**: 将来 List 型を扱う推論拡張をするとき、空リテラルの型が `Unknown` 基底で渡ってくることを知らないと型エラーの原因究明で迷う。
**Draft entry** (lift verbatim if approved):
> `infer_list_lit` で要素が空の場合、`Ty::List(Unknown)` を返す(エラーにしない)。空 `[]` は `let xs: List<Int> = []` のように let の型注釈や関数の引数位置・戻り型位置で `Unknown` が具体型に unify されることで意味を持つ。この「遅延具体化」方針を取ったため、空リストを注釈無しで使う箇所があると型エラーではなく `Unknown` のまま下流まで流れる可能性がある。将来 List の型推論を拡張する場合、この起点を把握しておくこと(`crates/kei_check/src/check.rs` の `infer_list_lit`)。

### Candidate: tagged 明示コンストラクタの emit は無変更 — `emit_alias` が既にTS関数を生成済み
**Why this matters for HANDOFF.md**: `ProductId("P-001")` という Kei 構文が TS に変換されるとき「なぜ emit.rs に特別なコードが要らないのか」が分かりにくく、将来 tagged 型の emit を修正しようとしたとき見落とす可能性がある。
**Draft entry** (lift verbatim if approved):
> `type ProductId = tagged string` の alias 宣言に対し、`emit_alias` が `export function ProductId(value: string): ProductId { ... }` という TS コンストラクタ関数をすでに出力している。そのため Kei の `ProductId("P-001")` は **通常の Call ノードとして** emit されるだけで足り、`emit_expr` 側に tagged-ctor 専用の特別ロジックは一切不要。この「構文糖 → 通常 Call → 既存 TS 関数」の対応を知らないと、emit のどこを読んでも tagged ctor 変換コードが見つからず困惑する。エントリポイントは `crates/kei_check/src/check.rs` の `call_named` / `NameKind::Alias` 分岐で型を解決し、emit は従来の Call 経路に任せる設計。

### Candidate: List リテラルの要素 emit に `Prec::Implication` を使う理由
**Why this matters for HANDOFF.md**: 他の場所で要素を emit するとき誤って低い Prec を使うと record リテラル / tagged ctor が不要な括弧なしで出て ambiguity が生じる恐れがあり、逆に高すぎる Prec を使うと必要な括弧が落ちる。
**Draft entry** (lift verbatim if approved):
> `Expr::ListLit` の各要素を `emit_expr` に渡すときは `Prec::Implication` を指定する(M22 実装: `crates/kei_emit/src/emit.rs`)。これは「リスト要素には record リテラルや tagged ctor が来る可能性があり、それらは高めの Prec で処理されるが、カンマを含む構文は括弧が必要になる場合がある」ため、emit_expr 側の親 Prec 判定に委ねる最上位に近い値を渡す設計。PR #69 の教訓「演算子の Prec 変更は全 emit 呼び出し側に波及する」と合わせて参照のこと。

### Candidate: `infer_list_lit` は先頭要素を正とし、後続を先頭に合わせる単方向 unify
**Why this matters for HANDOFF.md**: 双方向 unify への切り替えや「最も具体的な型への統合」戦略への変更を検討するとき、現行の非対称設計の理由と限界を把握する必要がある。
**Draft entry** (lift verbatim if approved):
> `infer_list_lit`(M22)は先頭要素の推論結果を「期待型」とし、後続要素を `check_assign(first, e_i)` で合わせる単方向 unify を採用している。これは `check_assign` が既存の単一方向 API であることと、先頭を権威とする方がエラーメッセージが直感的(「2 番目の要素が期待型と合わない」と言える)であることの両方による選択。将来双方向 unify(most-specific-supertype)が必要になる場合はこの関数を置き換える起点となる。`crates/kei_check/src/check.rs` `infer_list_lit` 参照。
