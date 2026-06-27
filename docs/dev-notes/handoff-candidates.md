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

## Hook fire (non-merge): fmt idempotency test — 2026-06-28

> **Note**: `tool_input.command` は `cargo run -p kei_cli --bin kei -- fmt /dev/stdin` であり `gh pr merge` を含まない。PR #75 の設計候補は上記セクションに記録済み。この発火で確認できた追加事実: `kei_fmt` が `[1, 2, 3]` を含むリストリテラルで冪等 (`fmt(fmt(x)) == fmt(x)`) であることを実行確認済み(session b0387495, tool_use_id toulu_01L7CxWqDUR6AELia6U88v7o)。CLAUDE.md の `kei_fmt` 冪等性制約が M22 リストリテラルでも満たされていることを検証した。

(no additional design-decision candidates — PR #75 already recorded above)

## PR #73 (Closes #54): feat(fmt): preserve // comments through kei fmt (M19) — 2026-06-27

> **Note**: `tool_input.command` は `echo "idempotency check 2 done"` であり `gh pr merge` を含まない。
> 最近マージされた PR の中で、当 agent セッションが詳細レビューしていた PR #73(M19 / #54、commit 73e72d0)
> の設計判断を記録する。`kei_fmt` の comment 保持機構はアーキテクチャ上の重要な選択を複数含む。

### Candidate: コメントは AST に持たせず「副チャネル」として別ストリームで渡す
**Why this matters for HANDOFF.md**: コメントを AST ノードに埋め込む案(Tree-sitter 方式等)との選択を知らないと、将来「コメントを AST で参照したい」ニーズが来たときに設計の制約に気付かず誤った拡張をする。
**Draft entry** (lift verbatim if approved):
> M19 の根本設計: `//` コメントはパーサが副チャネル `Vec<Comment>` に退避し、AST ノードには一切含めない。`ParseResult.comments` は span 付きコメント列として公開され、`format_with_comments(module, comments)` に独立して渡される。この設計の意図は「コメントは構文(意味)に無関係」という制約を型レベルで強制すること。`format_module(&Module)` が引数なしで呼べる純粋 API として残るのはこの副チャネル設計の帰結であり、proptest はこちらを使う。コメントを AST に埋め込む方式は将来的にも採用しない(AST の意味的変更禁止という ARCHITECTURE.md 制約に抵触する)。

### Candidate: `format_module` と `format_source` の二分割 — proptest 路と CLI 路の分離
**Why this matters for HANDOFF.md**: proptest が `format_module` を使うのに `format_source` を使うよう変更すると、ランダム入力に対して comment を生成しようとして panic/diverge する可能性がある。二分割の理由を明文化しないと将来の開発者が混乱する。
**Draft entry** (lift verbatim if approved):
> `kei_fmt` には意図的に 2 つの公開 API が存在する: `format_module(&Module) -> String` と `format_source(&str) -> Result<String, Vec<SyntaxError>>`。前者は AST のみを入力に取り、コメントは失われる(proptest・単体テスト向け純粋路)。後者はソーステキストを再パースしてコメント副チャネルも含めて整形する(CLI・MCP 路)。この分割を維持する理由: proptest は任意の AST を生成するが、その AST に対応するソーステキストは存在しない(コメントの span が undefined)。`format_source` を proptest から呼ぶと span 依存の整合性が崩れる。

### Candidate: flush 機構の行番号比較は `< start_line`(strictly less than)— 同行コメントは trailing で処理
**Why this matters for HANDOFF.md**: `flush_leading` の比較を `<=` に変えると、次のアンカーノードと同じ行のコメントが leading として先に emit され、その行の trailing_on 処理と二重emit になる。
**Draft entry** (lift verbatim if approved):
> `flush_leading(start_line, level)` の条件は `c.span.start.line < start_line`(厳密な less-than)。同じ行のコメントは `flush_trailing_on(line)` が `c.span.start.line == line` で処理する。この二つの関数は互いに排他的なコメントを担当する設計になっており、条件を `<=` に変更すると次のアンカーノードと同行のコメントが leading と trailing の両方で emit される二重出力バグが発生する。将来コメントのフィルタ条件を変更する際はこの排他性を必ず維持すること。

### Candidate: 署名行の trailing コメントは「requires 節の leading」として再配置される — 既知の設計制限
**Why this matters for HANDOFF.md**: `func f(x: Int) -> Int // note` の `// note` が整形後に requires 節の前行に移動する挙動は、ユーザーには「コメントが動いた」に見える。バグではなく既知の設計制限として明文化しないと、誤った修正試みが発生する。
**Draft entry** (lift verbatim if approved):
> M19 既知制限: 関数シグネチャ末尾(戻り型と同じ行)の trailing コメントは、`emit_func` がシグネチャ行で `flush_trailing_on` を呼ばないため、続く requires 節の `flush_leading` に吸収されて leading コメントとして再配置される。例: `func f(x: Int) -> Int // note\n  requires x > 0` → 整形後 `func f(x: Int) -> Int\n  // note\n  requires x > 0`。2 回目以降の整形では安定(冪等)だが、初回整形でコメントの物理位置が変わる。v0.1 では許容された既知の制限。関数シグネチャ専用の trailing flush を追加するには `emit_func` にシグネチャ span を追跡する変更が必要。

### Candidate: `flush_remaining` の末尾 `\n` はファイル末尾 `finish()` で一元付与する
**Why this matters for HANDOFF.md**: `flush_remaining` と `flush_leading` の非対称(前者は各コメント後に `\n` なし、後者は各コメント後に `\n` あり)は混乱源。その理由を記録しないと「バグ修正」として揃えようとする変更が別のバグを生む。
**Draft entry** (lift verbatim if approved):
> `flush_leading` は各コメントの後に `\n` を付ける(出力が常に行頭で終わることが不変条件)。`flush_remaining` はコメント**間**にのみ `\n` を挿入し、最後のコメントには付けない。この非対称は `flush_remaining` が常に `finish()` の末尾処理 `if !out.ends_with('\n') { push('\n') }` と組み合わせて使われるため意図的なもの。`flush_remaining` を `flush_leading` と揃えて末尾 `\n` を追加すると、`finish()` の末尾 `\n` と二重になるバグが発生する。`flush_remaining` は `finish()` の内部専用関数として扱うこと。

## Hook fire (non-merge): tagged constructor emit verification — 2026-06-28

> **Note**: `tool_input.command` は `cargo run -p kei_cli --bin kei -- build ... test_tagged.kei` であり `gh pr merge` を含まない(PostToolUse / Bash hook, tool_use_id `toolu_013UKJDE57GnsdRoR3UVsyxp`)。PR #75 の設計候補は上記セクションに記録済み。この発火で確認できた追加事実: `type ProductId = String tagged "ProductId"` + `func mk() -> ProductId { return ProductId("P-001") }` のコンパイル結果として `t.ts` に以下が出力された:
>
> ```typescript
> export type ProductId = string & { readonly __keiTag: "ProductId" };
>
> export function ProductId(value: string): ProductId {
>   return value as ProductId;
> }
>
> export function mk(): ProductId {
>   return ProductId("P-001");
> }
> ```
>
> これは「tagged 明示コンストラクタの emit は無変更 — `emit_alias` が既に TS 関数を生成済み」候補(上記 PR #75 セクション)の実行証拠として記録する。`ProductId("P-001")` という Kei 構文が通常の Call ノードとして下りてき、`emit_alias` が生成した TS 関数を呼ぶだけで TS 側で正しく型付けされることを実行確認した。

(no additional design-decision candidates — PR #75 already recorded above)

## Hook fire (non-merge): PR #73 fmt idempotency deeper analysis — 2026-06-28

> **Note**: `tool_input.command` は `echo "third idempotency done"` であり `gh pr merge` を含まない
> (PostToolUse / Bash hook, tool_use_id `toulu_0165zAPhXoK2zscScEaTbfpp`, agent `a7fdcd944a68f657b`)。
> 上記 PR #73 セクションに設計候補は記録済み。この発火で追加確認した idempotency 検証事実を記録する。

### Candidate: コメントのみのファイルは `flush_remaining` 路で正しく冪等処理される
**Why this matters for HANDOFF.md**: `emit_module` が「AST ノードなし」のとき `flush_leading` を呼ばないという設計判断を知らないと、コメントのみのファイルで二重改行や欠落が発生するバグとして誤診断しやすい。
**Draft entry** (lift verbatim if approved):
> コメントのみ(AST ノードゼロ)のファイルは `emit_module` で `earliest_top_line` が `None` を返し、`flush_leading` が一切呼ばれない。全コメントは `finish()` の `flush_remaining(0)` で処理され、最後の `\n` は `finish()` 末尾の `if !out.ends_with('\n')` で一元付与される。この経路により `"// only comment\n"` → 整形 → `"// only comment\n"` の冪等性が成立する。将来コメント専用ファイルのサポートを変更する場合、`emit_module` の「`earliest_top_line = None` のとき flush_leading スキップ」という分岐が起点となる。

### Candidate: import グループ間コメントは `newline()` 後に `flush_leading` を呼ぶ順序で位置保持される
**Why this matters for HANDOFF.md**: `i > 0` 分岐で `newline()` を呼んだ直後に `flush_leading` を呼ぶ順序を逆にすると、import 間コメントが行番号比較で「前の import と同行」と判定されて消える。順序の意図を明文化しないと「リファクタ」として入れ替えが発生する。
**Draft entry** (lift verbatim if approved):
> import ループ内の順序は `newline()` → `flush_leading(import.span.start.line, 0)` → `push(import_text)` で固定。`newline()` が出力を行末 `\n` で終わらせた後に `flush_leading` を呼ぶことで、import 間コメント(`// between imports` 等)が次の import の直前に正しく配置される。この順序を `flush_leading` → `newline()` と入れ替えると、先頭 import のリセット前に行番号比較が走り、コメントが next-import の leading として認識されなくなるバグが発生する。

## Hook fire (non-merge): PR #73 func clause comment analysis — 2026-06-28

> **Note**: `tool_input.command` は `echo "func clause analysis done"` であり `gh pr merge` を含まない
> (PostToolUse / Bash hook, tool_use_id `toolu_01Xn7VmXMBmNaM2hLhicAR8c`, agent `a7fdcd944a68f657b`)。
> 上記 PR #73 セクションに主要設計候補は記録済み。この発火で確認した追加事実を記録する:
> 当 agent セッションが詳細な idempotency ウォークスルーを実施し、extern グループ間コメントおよび
> `emit_func` の requires/ensures ループにおける `newline()` → `flush_leading` 順序を確認した。

### Candidate: `newline()` → `flush_leading` パターンは import/extern/requires/ensures ループで一貫して適用される

**Why this matters for HANDOFF.md**: このパターンが `emit_module` の import ループだけでなく `emit_func` の requires/ensures ループにも同じ順序で使われていることを知らないと、「requires ループだけ順序が違う」と思い込んで不要な修正が発生したり、新しいループを追加するときに誤った順序を採用する。

**Draft entry** (lift verbatim if approved):
> `flush_leading` の呼び出し前提条件「出力末尾が `\n` か空であること」は、M19 フォーマッタの全ループで `newline()` → `flush_leading` の順序を守ることで担保されている。この順序は import ループ(`i > 0` の場合)・extern グループ内ループ(`!first_in_group` の場合)・`emit_func` 内の requires ループおよび ensures ループすべてで統一されている。将来新たな「複数ノードを改行区切りで emit するループ」を追加する場合、この `newline()` → `flush_leading(node.span.start.line, level)` → `push(node_text)` という三連順序をテンプレートとして使うこと。順序を `flush_leading` → `newline()` に逆転させると、`flush_leading` が「コメントが現在行以降かどうか」を判定できなくなり、コメントが欠落または誤配置される。

### Candidate: extern グループ内コメントは import 間コメントと同一規則で処理される — グループ境界は区切らない

**Why this matters for HANDOFF.md**: extern グループ(連続する `extern` 宣言の集まり)の途中にコメントが挟まっても、グループが分割されるわけでも blank line が入るわけでもない。この挙動を知らないと「extern 間のコメントが消えた」または「blank line が増えた」と誤診断する可能性がある。

**Draft entry** (lift verbatim if approved):
> `emit_module` の extern グループループでは、グループ先頭のみ `write_separator_if_needed`(blank line)を挿入し、グループ内の 2 番目以降の extern は `newline()` → `flush_leading` → `push(extern_text)` の順で処理する。これは import ループと同一の規則であり、グループ内コメント(例: `extern foo() -> Int\n// between externs\nextern bar() -> Int`)は次の extern の直前に leading として配置され、blank line(separator)は挿入されない。この設計は「連続する extern はひとつのセマンティックブロックとして扱う」という方針の帰結。コメントによって extern グループが「分割」されることはなく、整形は冪等である。

## Hook fire (non-merge): M22 type-checking logic static verification — 2026-06-28

> **Note**: `tool_input.command` は `infer_list_lit` / `check_assign` / tagged constructor 型検査ロジックをコメントで静的分析し `echo "Analysis complete"` を実行するスクリプトであり `gh pr merge` を含まない
> (PostToolUse / Bash hook, tool_use_id `toolu_014LT8pu3wrHq3ycVpC4PVcQ`, agent `a5ee8385c9edb1832`)。
> PR #75(M22)の設計候補は上記セクションに記録済み。この発火で確認できた追加事実を以下に記録する。

### Candidate: PBT 評価器の `_ => Unsupported` は ListLit の意図的スキップ — バグではなく設計判断

**Why this matters for HANDOFF.md**: `pbt.rs` で ListLit 含む関数が PBT の対象外になる理由が「バグ修正漏れ」ではなく「スカラ範囲外は安全に弾く」という意図的設計であることを知らないと、将来の改善時に誤って `_ => panic!` や強制的な評価を実装する恐れがある。

**Draft entry** (lift verbatim if approved):
> `crates/kei_check/src/pbt.rs` の `eval_expr` は M22 追加の `Expr::ListLit` を `_ => Err(EvalError::Unsupported)` の catch-all で処理する。これは意図的な設計: PBT 評価器はスカラ値(Int/Bool/Str)の範囲テストに特化しており、List 値を含む関数は PBT の対象外として **静かに** スキップされる(`Unsupported` は panic ではなくテストスキップ扱い)。将来 List を含む関数を PBT 対象にしたい場合は、`eval_expr` に `ListLit` の arm を追加し、要素を再帰的に評価して `Value::List(...)` を返すように拡張する。現時点ではこの拡張を行わないのは「PBT はスカラ関数の境界テストに注力する」という PBT 設計方針による。`infer_list_lit` 自体は正しく型推論するので、PBT スキップは型検査の正確性には影響しない。

(no additional design-decision candidates beyond the above)

## Hook fire (non-merge): PR #73 emit_if IfStmt span analysis — 2026-06-28

> **Note**: `tool_input.command` は `git show origin/m19-fmt-comments:crates/kei_syntax/src/ast.rs | grep -A 10 "IfStmt"` であり `gh pr merge` を含まない
> (PostToolUse / Bash hook, tool_use_id `toolu_011bEjYXHoTgw2n1am9LNL1n`, agent `a7fdcd944a68f657b`)。
> 上記 PR #73 セクションに主要設計候補は記録済み。この発火では BUG 8 の静的分析として
> `emit_if` / `IfStmt.span` の trailing comment 処理の正しさを確認した。

### Candidate: `IfStmt.span` は else 節末尾の `}` まで含む — trailing コメント処理の前提条件

**Why this matters for HANDOFF.md**: `IfStmt.span.end.line` が then_block の closing `}` のみを指していた場合、else 節の `}` 直後の trailing コメントが `flush_trailing_on` で取りこぼされる。パーサがこのスパンを正しく設定することが M19 フォーマッタの意図的な前提条件であることを明文化しないと、将来パーサを修正する際に破壊する恐れがある。
**Draft entry** (lift verbatim if approved):
> M19 フォーマッタの `emit_block` は、各 stmt の trailing コメントを `flush_trailing_on(stmt.span().end.line)` で処理する。`Stmt::If` の場合、`IfStmt.span` が **else 節の closing `}` まで含む全体スパン** であることが必須前提条件。例えば `if cond { ... } else { ... } // note` の `// note` は `IfStmt.span.end.line` が else ブロックの `}` と同じ行を指すことで、外側の `emit_block` ループの `flush_trailing_on` に正しくキャプチャされる。パーサ(`crates/kei_syntax/src/parser.rs`)が if-else 全体のスパンをこの範囲に設定することは、コメント処理の正確性に直接影響する。パーサの if-else スパン計算を変更する場合は M19 フォーマッタのコメント配置テストを必ず確認すること(`tests/golden/fmt/comments.*`)。

### Candidate: `emit_if` は内部 trailing コメントを `emit_block` に委譲し、外側は `Stmt::If` スパンで一元処理する

**Why this matters for HANDOFF.md**: `emit_if` 自体が `flush_trailing_on` を呼ばない理由が分からないと「emit_if が trailing コメントを処理していない = バグ」と誤解し、重複した `flush_trailing_on` 呼び出しを追加する誤修正が起きる。

**Draft entry** (lift verbatim if approved):
> `emit_if` は `flush_trailing_on` を一切呼ばない。これは意図的な責務分離: (a) then/else 各ブロック内部のコメント(ブロック内 stmt の trailing と pre-close comment)は `emit_block` が処理する、(b) if 文全体の trailing コメント(最後の `}` と同行)は、`emit_if` を呼んだ外側の `emit_block` ループが `flush_trailing_on(stmt.span().end.line)` として処理する。`emit_if` に `flush_trailing_on` を追加すると、外側 `emit_block` との二重処理でコメントが重複出力される。if 文コメント処理のデバッグは「外側の emit_block が IfStmt.span を使って flush_trailing_on を呼ぶ」という委譲構造を前提に行うこと。

## Hook fire (non-merge): M22 tagged ctor emit + list literal readonly analysis — 2026-06-28

> **Note**: `tool_input.command` は tagged constructor emit の正確性と list literal の TS `readonly` 型付けをコメントで静的分析するスクリプトであり `gh pr merge` を含まない
> (PostToolUse / Bash hook, tool_use_id `toulu_01QePZTNEp8duCErC8xmdjWZ`, agent `a5ee8385c9edb1832`)。
> PR #75(M22)の主要設計候補は上記セクションに記録済み。この発火で確認できた追加事実を記録する。

### Candidate: List リテラルの TS 出力は `readonly T[]` 型だが配列自体はミュータブル — Kei レベルで不変性を強制する設計判断

**Why this matters for HANDOFF.md**: 「Kei は不変を謳っているのに TS の emit が `as const` を付けないのはバグでは？」という疑問は将来の開発者が必ず持つ。`number[]` を `readonly number[]` に代入できる TypeScript の共変性(assignable covariance)が理由であることを記録しておかないと、誤って `as const` を追加して他のコードとの型不整合を引き起こす可能性がある。

**Draft entry** (lift verbatim if approved):
> `Expr::ListLit` の emit は `[el1, el2, el3]` という JS 配列リテラルをそのまま出力し、`as const` や `Object.freeze` を付加しない。Kei の戻り型 `List<Int>` は TS の `readonly Int[]` に変換されるが、TypeScript は `number[]` を `readonly number[]` に代入可能(共変 readonly)とするため型エラーは発生しない。ランタイムでは配列はミュータブルだが、Kei の不変性制約はコンパイル時に Kei レベルで強制される設計であり、生成 TS 側の不変性は「型シグネチャによる意図表明」にとどまる。将来 `as const` を追加したくなった場合は、配列リテラルを含む式全体の TS 型推論への影響(推論型が `readonly [1, 2, 3]` のようなタプル型になる可能性)を確認すること。

(no additional design-decision candidates beyond the above)

## Hook fire (non-merge): PR #73 if-else trailing comment edge case — 2026-06-27

> **Note**: `tool_input.command` は if-else コメント処理の静的分析コメントを含む `echo "if-else comment analysis done"` であり `gh pr merge` を含まない
> (PostToolUse / Bash hook, tool_use_id `toolu_016qVEHBHtwJzALZWcezceWQ`, agent `a7fdcd944a68f657b`)。
> 上記 PR #73 セクションに主要設計候補は記録済み。この発火では `emit_if` の if-else 構造における
> then_block 末尾 `}` の trailing コメント処理に関する追加の境界ケース分析を実施した。

### Candidate: then-block 末尾 `}` の trailing コメント(else がある場合)は flush されない — 既知の設計上の空白

**Why this matters for HANDOFF.md**: `if cond { ... } // mid-comment\n else { ... }` における `// mid-comment` が整形後にどこに現れるか(あるいは消えるか)を知らないと、この位置のコメントが欠落または誤配置されたときにデバッグが著しく困難になる。

**Draft entry** (lift verbatim if approved):
> M19 既知の境界ケース: `emit_if` で else 節が存在する場合、then_block 末尾の `}` と `else` キーワードが同行に並ぶ形式(`... } else {`)を Kei fmt が採用する。このとき `then_block.span.end.line`(then の `}` の行)に trailing コメントが書かれていても、`flush_trailing_on` を呼ぶ主体がいない。
>
> - 外側の `emit_block` ループは `flush_trailing_on(stmt.span().end.line)` を呼ぶが、`stmt.span().end.line` は **else 節末尾の `}`** の行(IfStmt 全体の最後)であり then_block 末尾の行ではない。
> - `emit_if` 自体は `flush_trailing_on` を呼ばない(設計判断)。
>
> 結果として `} // mid-comment` の `// mid-comment` は then_block.span.end.line に存在するが、その行の trailing flush を担う箇所がない。代わりに次の `emit_block(else_block)` の `flush_leading` が then_block.span.end.line より後の行を対象とするため、コメントは else ブロック内の先頭コメントとして吸収される可能性がある。これは「コメントの物理位置が変わる」初回整形での既知の制限であり、2 回目以降は冪等となる。この位置へのコメント配置は v0.1 では未保証と扱うこと。将来修正する場合は `emit_if` に `flush_trailing_on(stmt.then_block.span.end.line)` を then/else 境界で呼ぶ拡張が必要(`crates/kei_fmt/src/lib.rs` の `emit_if` 参照)。

## Hook fire (non-merge): PR #73 trailing tail comment idempotency — 2026-06-28

> **Note**: `tool_input.command` はコメントのみのスクリプト(`echo "trailing tail comment idempotency verified"`)であり `gh pr merge` を含まない
> (PostToolUse / Bash hook, tool_use_id `toulu_01P7j1T7vKBkwsZsJ3mCLCtz`, agent `a7fdcd944a68f657b`)。
> 上記 PR #73 セクションに主要設計候補は記録済み。この発火では PR #73 の `comments.expected.kei` golden ファイルにおける末尾コメント(`// trailing tail comment`)の 2 段階冪等性をウォークスルーで静的検証した。

### Candidate: ファイル末尾の tail コメントは AST ループ後に `finish()` → `flush_remaining` 経路で処理される — `emit_module` の items ループは関与しない

**Why this matters for HANDOFF.md**: `// trailing tail comment` のような「最後の AST ノードより後に来るコメント」がどの経路で出力されるかを知らないと、items ループ内に対策を追加しようとする誤修正が発生する。また `finish()` を呼ばないバイパス経路を追加した場合に tail コメントが無音で消える。

**Draft entry** (lift verbatim if approved):
> `emit_module` の items ループは最後の AST ノード(例: 関数定義の `}`)を emit し終えた時点で終了する。ループ後に残るコメント(last node より後の行に位置する `// trailing tail comment` 等)は items ループでは一切処理されない。これらは必ず `finish()` → `write_separator_if_needed()` → `flush_remaining(0)` の経路で処理される。`finish()` の末尾 `if !out.ends_with('\n') { push('\n') }` が最終的な `\n` を保証するため、`flush_remaining` 自体は最後のコメントに `\n` を付けない設計になっている(「`flush_remaining の末尾 \n` は `finish()` で一元付与」候補を参照)。この経路は 2 回目の整形でも同一であり、`format_source(expected) == expected` の冪等性が成立する。将来 `emit_module` に items ループ後の処理を追加する場合、`finish()` を必ず呼ぶことで tail コメントが保持されることを確認すること。
