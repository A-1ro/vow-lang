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

## PR #72 (auditor re-run): kei-invariant-auditor PostToolUse — 2026-06-27

(no actionable patterns — hook triggered by invariant auditor `git diff --stat` tool use, not by `gh pr merge`; most recent merged PR #72 already documented above)
