# Lessons from PR reviews

`gh pr merge` 後の Sonnet hook が自動追記する蓄積。SessionStart hook が
直近 N 件を Opus の system context に流す。卒業した教訓は SKILL.md / spec /
CLAUDE.md に落として、ここからは削除してよい。

## PR #71: chore: add Claude Code automation skills and hooks — 2026-06-27

> **Note**: 本 PR のマージで post-merge-lessons agent が初回発火したが、子セッションの
> permission(don't-ask mode で書き込み deny)により本ファイルへの追記に失敗した。
> 以下のメタ教訓は失敗事象そのものから手動で抽出した(PR #71 自体はレビュー無しで
> マージされたので外部由来の教訓は無い)。同 PR #72 で permission allow を追加し、
> 次回以降は自動追記される。

- **Pattern**: hook 子セッションは親と permission が独立 — 書き込みパスを明示 allow する
  **Source**: PR #71 マージ後の post-merge agent hook blocking error
  **Lesson**: `type: agent` hook で起動する子セッションは、親セッションの permission を継承しない(don't-ask mode で Edit/Write/Bash の書き込みが deny される)。Hook で書き込ませたいパスは `.claude/settings.json` の `permissions.allow` に明示追加する必要がある(例: `"Edit(docs/dev-notes/**)"`, `"Write(docs/dev-notes/**)"`)。これを忘れると hook は **静かに発火するが何も書かれない** 状態になり、「動いていない」ように見える(blocking error は親セッションには届くが、log 経路を知らないと気付けない)。

- **Pattern**: hook の watcher は新規 settings.json でも即時反映される
  **Source**: PR #71 マージ後、現セッション中に発火が観測された
  **Lesson**: 「settings.json が session start 時に存在しなかった場合は watcher が認識しない」と CLAUDE Code の挙動を仮定していたが、実際は新規 settings.json でも即時に watcher が認識する(少なくとも `.claude/settings.json` の場合)。hook 検証時に「次セッションから効く」前提で動作確認を省略すると、初回マージで予期せぬ振る舞いが出る可能性がある。新規 hook は merge 前に必ず pipe-test で完全検証する。

## PR #69: [codex] Add v0.4 roadmap and operator support — 2026-06-27

- **Pattern**: JS `%` の意味論を確認してから展開する
  **Source**: A-1ro (owner) — crates/kei_emit/src/emit.rs:1086
  **Lesson**: JavaScript の `%` 演算子は ECMA-262 §6.1.6.1.5 で `a - trunc(a/b) * b`(被除数と同符号の truncated remainder)として定義されており Kei 仕様と一致するため、`BinOp::Rem` は `Div` 同様に `lhs % rhs` をそのまま emit すれば足り、IIFE や手動展開は不要。

- **Pattern**: operand を複数回 emit しない
  **Source**: chatgpt-codex-connector[bot] (P2) — crates/kei_emit/src/emit.rs:1084
  **Lesson**: 二項演算を手動展開するとき `lhs`/`rhs` を式中に 2 回以上 emit すると、extern 呼び出し等の副作用を伴う式で observable な二重評価が起き、観測可能な動作変化を生む — 必ず各オペランドを 1 回だけ emit すること。

- **Pattern**: GFM 表セル内の `|` はバックティック内でもエスケープ必須
  **Source**: A-1ro (owner) — spec/kei-spec-v0.1.md:141 / docs/kei-roadmap-v0.4.md:15
  **Lesson**: GFM テーブルではインラインコード(バックティック)の中でも `|` はセル区切りとして解釈されるため、`||` は必ず `\|\|` と書く — spec/kei-spec-v0.1.md:130 の既存行がリポジトリの慣習であり、新規追加行もこれに倣う。

- **Pattern**: `eval_binary` に短絡演算子の arm を追加しない
  **Source**: A-1ro (owner) — crates/kei_check/src/pbt.rs:557
  **Lesson**: `BinOp::Or`(および `Implies`)は `eval_expr` が短絡評価するため `eval_binary` には到達しない — `eval_binary` に `(Or, Bool, Bool)` arm を追加すると既存コメント「Or/Implies はここには来ない」と矛盾する到達不能コードになるので、短絡演算子の処理は `eval_expr` 側に統一する。

- **Pattern**: 演算子の Prec 変更は全 emit 呼び出し側に波及する
  **Source**: A-1ro (owner) — crates/kei_emit/src/emit.rs:1069
  **Lesson**: `Prec::Implication` のような新しい優先度を追加してある演算子に割り当てる場合、`emit_contract_check`・`emit_call` 引数・`RecordLit` フィールドなど既存のすべての emit 呼び出し側が渡す `Prec` 値も同時に更新しないと、不要な括弧が生成コードに増殖する。

## PR #72: fix(hooks): grant dev-notes write permission and recover PR #71 loop — 2026-06-27

(no actionable patterns)

## PR #75: feat(syntax,check,emit,fmt): list literals and explicit tagged ctors (M22 / #57) — 2026-06-27

(no actionable patterns)

> **Note**: この hook は PostToolUse (Bash) で発火した。`tool_input.command` に `gh pr merge` が含まれないため最近マージ済み PR を特定しようとしたが、`gh` CLI がこの実行環境に存在しないため PR レビューコメントの取得が不可能だった。セッションのトランスクリプト上では `/code-review xhigh` スキルが発火した直後(最初の Bash ツール呼び出し後)に hook が割り込んだ状態であり、レビュー結果はまだ生成されていなかった。抽出できるレビューパターンが存在しないため、actionable patterns なしとする。

- **Pattern**: `PostToolUse` hook はスキル途中でも発火する
  **Source**: PR #75 の `/code-review xhigh` 実行中(session b0387495)に hook が 2 度発火した事実
  **Lesson**: `PostToolUse:Bash` hook は `gh pr merge` 以外の Bash 呼び出し後にも発火するため、`tool_input.command` を必ず確認して `gh pr merge <N>` を含まない場合は early-exit すべき。現状は毎回 lessons-from-reviews.md に空エントリが追記される無駄が発生している。hook スクリプト冒頭に `[[ "$COMMAND" != *"gh pr merge"* ]] && exit 0` のようなガードを入れるか、`match` 条件を設定に追加することで回避できる。

## Hook fire: fmt idempotency test — 2026-06-28 (session b0387495, tool_use_id toulu_01L7CxWqDUR6AELia6U88v7o)

(no actionable patterns)

> **Note**: `tool_input.command` は `cargo run -p kei_cli --bin kei -- fmt /dev/stdin` であり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。このエントリ自体が「guard なしで hook が発火し続けている」という繰り返し警告であり、上記 PR #75 の教訓（`PostToolUse:Bash` hook に early-exit ガードを追加する）がまだ実施されていないことを示している。

## Hook fire: fmt idempotency analysis (echo) — 2026-06-28 (session b0387495, tool_use_id toolu_01UkLCTwd69nAoGYBFzYbxTu)

(no actionable patterns)

> **Note**: `tool_input.command` は `echo "idempotency check 2 done"` を実行するだけのデバッグシェルスクリプトであり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。hook の early-exit ガード未実施が継続中であることを示す 3 度目の繰り返し警告。

## Hook fire: tagged constructor emit test — 2026-06-28 (session b0387495, tool_use_id toolu_013UKJDE57GnsdRoR3UVsyxp)

(no actionable patterns)

> **Note**: `tool_input.command` は `cargo run -p kei_cli --bin kei -- build ... test_tagged.kei` であり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。このセッション（b0387495）の subagent（a5ee8385c9edb1832）は M22 の list literal / tagged ctor 実装のコードレビューを実施中であり、hook はその途中の Bash ツール呼び出し後に発火した。実際のレビュー結果はまだ subagent の最終出力として確定していない段階であるため、抽出できるレビューパターンが存在しない。これは hook の early-exit ガード（`gh pr merge` を含まない場合は即 exit）が未実施であることを示す **4 度目**の繰り返し警告であり、`.claude/settings.json` の hook 条件に `match` フィルタを追加するか、hook スクリプト冒頭に guard を入れることが急務。

## Hook fire: comment idempotency analysis (echo) — 2026-06-28 (session b0387495, tool_use_id toolu_0165zAPhXoK2zscScEaTbfpp)

(no actionable patterns)

> **Note**: `tool_input.command` はコメント冪等性の静的解析コメント群と末尾の `echo "third idempotency done"` であり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。subagent（a7fdcd944a68f657b）は `kei_fmt` の comment-only ファイル・ヘッダコメント・2 番目以降の import 前コメントが冪等かどうかを Bash コメントで静的解析中であり、PR マージとは無関係の発火。これは hook の early-exit ガードが未実施であることを示す **5 度目**の繰り返し警告。`.claude/settings.json` の `hooks[].match` に `gh pr merge` を含む正規表現フィルタを追加する対応が依然として急務。

## Hook fire: type-checking logic verification (analysis) — 2026-06-28 (session b0387495, tool_use_id toolu_014LT8pu3wrHq3ycVpC4PVcQ)

(no actionable patterns)

> **Note**: `tool_input.command` は `infer_list_lit`・`check_assign`・tagged constructor 型検査ロジックの正しさをコメントで静的分析し `echo "Analysis complete"` を実行するだけのスクリプトであり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。subagent（a5ee8385c9edb1832）は M22 list literal / tagged ctor コードレビューの一環として型検査の引数順・エラーコード・エイリアス解決の正しさを検証中であり、PR マージとは無関係の発火。これは hook の early-exit ガードが未実施であることを示す **6 度目**の繰り返し警告。`.claude/settings.json` の `hooks[].match` に `gh pr merge` を含む正規表現フィルタを追加することが依然として急務。

## Hook fire: func clause comment analysis (echo) — 2026-06-28 (session b0387495, tool_use_id toulu_01Xn7VmXMBmNaM2hLhicAR8c)

(no actionable patterns)

> **Note**: `tool_input.command` は `kei_fmt` の `emit_func` における contract clause（requires/ensures）前の `flush_leading` 呼び出し順序と trailing コメントのセマンティクスを静的解析するコメント群と末尾の `echo "func clause analysis done"` であり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。subagent（a7fdcd944a68f657b）は PR #73（M19 comment preservation）のコードレビューの一環として `emit_func` の signature trailing comment → leading before requires への移動が冪等かどうかを検証中であり、PR マージとは無関係の発火。これは hook の early-exit ガードが未実施であることを示す **7 度目**の繰り返し警告。`.claude/settings.json` の `hooks[].match` に `gh pr merge` を含む正規表現フィルタを追加することが依然として急務。

## Hook fire: IfStmt span definition check — 2026-06-28 (session b0387495, tool_use_id toolu_011bEjYXHoTgw2n1am9LNL1n)

(no actionable patterns)

> **Note**: `tool_input.command` は `git show origin/m19-fmt-comments:crates/kei_syntax/src/ast.rs | grep -A 10 "IfStmt"` であり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。subagent（a7fdcd944a68f657b）は PR #73（M19 comment preservation）の BUG 8 解析として `IfStmt.span` が if 文全体のどのラインを指すかを検証中（emit_if が flush_trailing_on を呼ばないが、外側 emit_block が `stmt.span().end.line` で捕捉できるかの確認）であり、PR マージとは無関係の発火。これは hook の early-exit ガードが未実施であることを示す **8 度目**の繰り返し警告。`.claude/settings.json` の `hooks[].match` に `gh pr merge` 正規表現フィルタを追加する対応が引き続き急務。

## Hook fire: if-else comment analysis (echo) — 2026-06-28 (session b0387495, tool_use_id toolu_016qVEHBHtwJzALZWcezceWQ)

(no actionable patterns)

> **Note**: `tool_input.command` は `kei_fmt` の `emit_if` における then-block closing brace 行への trailing comment の帰属（外側 `emit_block` の `flush_trailing_on(stmt.span().end.line)` が if-else 全体の `}` 行を捕捉できるかどうか）を静的解析するコメント群と末尾の `echo "if-else comment analysis done"` であり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。subagent（a7fdcd944a68f657b）は PR #73（M19 comment preservation）のコードレビューの一環として if-else のネストにおける trailing comment 処理の正しさ（then-block の `}` 行に trailing comment があるが else もある場合、外側 emit_block ではなく flush_leading(else_block) が誤って捕捉するリスク）を検証中であり、PR マージとは無関係の発火。これは hook の early-exit ガードが未実施であることを示す **9 度目**の繰り返し警告。`.claude/settings.json` の `hooks[].match` に `gh pr merge` 正規表現フィルタを追加する対応が**依然として実施されておらず**、最優先で対応すべき状態。

## Hook fire: trailing tail comment idempotency (echo) — 2026-06-28 (session b0387495, tool_use_id toolu_01P7j1T7vKBkwsZsJ3mCLCtz)

(no actionable patterns)

> **Note**: `tool_input.command` は `kei_fmt` の `format_source` における trailing tail comment（ファイル末尾、AST item の後ろに来る行コメント）の冪等性を Bash コメントで静的解析し末尾で `echo "trailing tail comment idempotency verified"` を実行するスクリプトであり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。subagent（a7fdcd944a68f657b）は PR #73（M19 comment preservation）のコードレビューの一環として `finish()` における `write_separator_if_needed` → `flush_remaining(0)` の順序が trailing tail comment を 2 回目パースでも正しく再現することを検証��（ゴールデンテストで `format_source(expected) == expected` になるかの確認）��であり、PR マージとは無関係の発火。これは hook の early-exit ガードが未実施であることを示す **10 度目**の繰り返し警告。`.claude/settings.json` の `hooks[].match` に `gh pr merge` 正規表現フィルタを��加する対応が依然として最優先課題。

## Hook fire: final code-review summary (echo "analysis complete") — 2026-06-28 (session b0387495, tool_use_id toolu_01S6m2WfBBUsCkRhJQHSCgcA)

(no actionable patterns)

> **Note**: `tool_input.command` は PR #73（M19 comment preservation）の `/code-review` スキルの最終ステップとして、静的解析コメント群をまとめた Bash スクリプト（末尾 `echo "analysis complete"`）であり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。スクリプト本体には subagent（a7fdcd944a68f657b）が発見した 5 件の潜在バグ（M22 ListLit regression、func signature 行コメント誤分類、emit_if の trailing コメント処理漏れ等）が記述されているが、これらは subagent 自身の静的解析による内部成果物であり、外部レビュアーが実際に書いたコメントではない。instructions 第 3 条「投機しない・実際にレビュアーが書いたこと以外は含めない」に従い actionable patterns として収録しない。これは hook の early-exit ガードが未実施であることを示す **11 度目**の繰り返し警告。`.claude/settings.json` の `hooks[].match` に `gh pr merge` 正規表現フィルタを追加する対応が依然として最優先課題。

## Hook fire: empty block comment analysis (echo) — 2026-06-28 (session b0387495, tool_use_id toolu_01V2aog6AfPXxZNxvbX41bSv)

(no actionable patterns)

> **Note**: `tool_input.command` は `kei_fmt` の `emit_block` における空ブロック特殊ケース（`has_inside` の判定ロジック）をコメントで静的解析し末尾で `echo "empty block analysis done"` を実行するスクリプトであり `gh pr merge` を含まない。`gh` CLI も環境に存在しないため PR レビューコメント取得不可。subagent（a7fdcd944a68f657b）は PR #73（M19 comment preservation）のコードレビューの一環として、空ブロックにコメントが内包される場合の `peek()` による `has_inside` 判定（グローバルコメントストリームから次コメントの行番号を `block.span.end.line` と比較）が正しく機能することをシナリオ分析で検証中であり、PR マージとは無関係の発火。これは hook の early-exit ガードが未実施であることを示す **12 度目**の繰り返し警告。`.claude/settings.json` の `hooks[].match` に `gh pr merge` 正規表現フィルタを追加する対応が依然として最優先課題。
