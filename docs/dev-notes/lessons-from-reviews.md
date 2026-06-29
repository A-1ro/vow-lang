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

## PR #79: chore(hooks): auto-run kei-code-review on gh pr create — 2026-06-28

- **Pattern**: `gh pr view --json` の fields に `author` を含め忘れる
  **Source**: A-1ro (owner) — general discussion (kei-code-review finding #1, CONFIRMED/correctness)
  **Lesson**: hook prompt で dependabot PR をスキップする条件として `author.login` を参照する場合、`gh pr view <N> --json` に `author` を明示指定しないとフィールドが null になりスキップ条件が常に無効になる。`--json` に渡す fields リストは実際に参照する全フィールドを列挙すること。

- **Pattern**: `--json` で取得した未使用フィールドを残さない
  **Source**: A-1ro (owner) — general discussion (kei-code-review finding #2, CONFIRMED/correctness)
  **Lesson**: hook スクリプトや prompt で `gh pr view --json body,...` のように `body` を取得しながら skip ロジックで一度も参照しないと dead field になる。使わないフィールドは `--json` から除いてノイズを減らす。逆に参照するフィールドは必ず取得リストに含めること(finding #1 と対になる教訓)。

- **Pattern**: draft PR チェックはセッション起動前に行う
  **Source**: A-1ro (owner) — general discussion (kei-code-review finding #3, PLAUSIBLE/pitfalls)
  **Lesson**: 現行の hook は draft PR でも Sonnet 子セッションを起動してから `gh pr view` で draft 判定しスキップしている。セッション起動コストを避けるには、親フック側(`tool_response.stdout` の URL 確定直後)で `gh pr view <N> --json isDraft` を叩いて draft なら子セッションを起動しない分岐を入れる方が望ましい。

## PR #81: feat(skills): kei-dogfood — auto-file next-version Issues from feedback — 2026-06-28

- **Pattern**: `gh issue create --label` は複数ラベルをカンマ区切りで渡せない
  **Source**: A-1ro (owner) — `.claude/skills/kei-dogfood/SKILL.md` line 292 (CONFIRMED/correctness)
  **Lesson**: `gh issue create --label "dogfood, from-v0.4, fix-chain"` のようにカンマ区切り文字列を 1 つの `--label` に渡すと、それを単一ラベル名として扱い 404/422 で失敗する。複数ラベルは `--label "dogfood" --label "from-v0.4" --label "fix-chain"` のように flag を繰り返すか、動的に `label_args` 配列を組み立てること。

- **Pattern**: `gh issue list --milestone` はマイルストーン番号で指定する
  **Source**: A-1ro (owner) — `.claude/skills/kei-dogfood/SKILL.md` line 268 (PLAUSIBLE/correctness)
  **Lesson**: `gh issue list --milestone v0.5` のようにタイトル文字列でフィルタすると、大文字小文字・空白の差異で silently 0 件になる。事前に `gh api repos/:owner/:repo/milestones --jq ".[] | select(.title==\"$milestone\") | .number"` でマイルストーン番号を解決し、番号で指定することで確実に dedup できる。

- **Pattern**: 自然言語の承認ゲートはLLMが短絡しうる
  **Source**: A-1ro (owner) — `.claude/skills/kei-dogfood/SKILL.md` line 286 (PLAUSIBLE/pitfalls)
  **Lesson**: 「承認前に `gh issue create` を絶対実行しない」を自然言語指示のみで縛ると、LLM が好意的な曖昧メッセージを承認とみなして gate を抜ける可能性がある。`permissions.allow` から当該コマンドを外してパーミッションプロンプトをバックストップにするか、SKILL.md に `# HARD GATE` ブロックを設けて禁止コマンドを明示するなど機械的な防線を追加すること。

- **Pattern**: `gh issue comment` の出力はコメント URL アンカーを含まない
  **Source**: A-1ro (owner) — `.claude/skills/kei-dogfood/SKILL.md` line 293 (PLAUSIBLE/pitfalls)
  **Lesson**: `gh issue comment <N> --body "..."` はデフォルトで Issue URL(`https://github.com/.../issues/N`)しか返さず、`#issuecomment-<id>` アンカーが付かない。ディープリンクを出力に含めたい場合は `--json url --jq '.url'` を追加して comment アンカー付き URL を取得すること(`gh` v2.17+ 必須)。

## PR #82: chore(skills): plan-then-delegate を実装タスクで常時発火に緩める — 2026-06-28

- **Pattern**: skill トリガーキーワードは複合語に絞る
  **Source**: A-1ro (owner) — `.claude/skills/plan-then-delegate/SKILL.md` (inline review comment, PLAUSIBLE/pitfalls)
  **Lesson**: `「対応」` のような単語を単体でスキル発火トリガーに追加すると、「〇〇の質問に対応して」「設計案に対応した…」など編集意図のない文脈でも誤発火する。トリガーキーワードは `「レビュー対応」「CI 対応」` のように複合語・修飾語付きに限定し、汎用単語は登録しないこと。

## PR #82 (altitude-finder re-run): kei-code-review PostToolUse — 2026-06-28

(no actionable patterns — hook triggered by `grep` tool call inside kei-code-review altitude finder for PR #83 (OPEN, not merged); most recent merged PR #82 already documented above)

## PR #82 (verifier re-run): kei-code-review PostToolUse — 2026-06-28

(no actionable patterns — hook triggered by Bash tool call (`mkdir`/`cat` creating `requires_old_lambda.kei` scratch file) inside kei-code-review verifier subagent for PR #83 (OPEN, not merged); most recent merged PR #82 already documented above)

## PR #83: feat: v0.4 remaining — M24 stock e2e + M25 lambdas + M26 Money notice — 2026-06-28

- **Pattern**: `old(lambda内式)` はラムダパラメータ参照を禁止する
  **Source**: A-1ro (owner) — crates/kei_emit/src/emit.rs:1205 (1st pass review, CONFIRMED/correctness 🔴)
  **Lesson**: `collect_old_exprs` がラムダ body を横断するとき、ラムダパラメータを参照する式(例: `old(p.qty)`)を巻き上げると未定義参照の TS が生成され `ReferenceError` で全実行が死ぬ。`check` 側にラムダパラム参照禁止ガードを設け、`emit` 側でも `old(...)` 中のラムダスコープ変数を検知して KEI-E4002 を発火させること。

- **Pattern**: AST 拡張後は pbt.rs の eval も追従させる
  **Source**: A-1ro (owner) — crates/kei_check/src/pbt.rs ~L980 (1st pass review, CONFIRMED/correctness 🔴)
  **Lesson**: `Expr::Lambda` を AST に追加した場合、`pbt.rs` の `eval_list_method` も `Expr::Lambda` を受け取る arm を追加しないと、generative 検証が `[bounded]` → `[runtime]` に静かに劣化する(反例検出が無効化される)。AST ノード追加時は必ず pbt.rs の eval 網羅性を確認すること。

- **Pattern**: spec §2.5 はパーサ実装に先行させる(0引数ラムダ)
  **Source**: A-1ro (owner) — crates/kei_syntax/src/parser.rs:1631 (1st pass review, CONFIRMED/invariant 🔴)
  **Lesson**: `() => expr` を受理するパーサ実装が先に入り spec §2.5 に「0引数ラムダ禁止」が追記されないと、不変条件 #4(spec-first)違反になる。新文法の実装前に spec を更新し、禁止構文はパーサでエラーにする(golden で固定する)こと。

- **Pattern**: 修正コードが新バグを持ち込む連鎖を想定してレビューを重ねる
  **Source**: A-1ro (owner) — general discussion (2nd pass review summary)
  **Lesson**: 1st pass で潰した F0/F1/F3 の修正コード自体が N0/N1/N3 という新規バグを持ち込んだ(0引数ラムダ修正の cascade、fix 文面の実現不能指示、`old()` 二段防御の片側漏れ)。修正 PR では必ず再レビューを 1 回追加し、「修正コードが新バグを入れていないか」を独立に確認すること。

- **Pattern**: fix 文面(Agent Repair Protocol)の実現可能性を検証する
  **Source**: A-1ro (owner) — crates/kei_check/src/check.rs:1600 (2nd pass review, CONFIRMED/correctness 🔴)
  **Lesson**: Diagnostic の `fix` フィールドに書く修正指示(例: 「Pass through the lambda parameter, or compute it outside」)は、その指示が実際に Kei の文法・制約下で実現可能かを確認してから書く。arity-1 制約やキャプチャ禁止により両案が実現不能な場合、誤誘導になり Agent Repair Protocol が機能しなくなる。

- **Pattern**: examples 内 `Money.zero` は spec §2.4 と整合させる
  **Source**: A-1ro (owner) — examples/contracts/withdraw.kei + transfer.kei (2nd pass review, CONFIRMED/invariant 🔴)
  **Lesson**: spec §2.4 で `Money.zero` を廃止・無効化した場合、examples/ 内のすべての `Money.zero` 参照も同時に更新する(MCP 配信時に spec と examples が矛盾するドキュメントを返してしまう)。spec 変更後は `grep -r "Money.zero" examples/` で残存参照を即座に検出・修正すること。

- **Pattern**: TS 予約語はラムダパラメータ名として素通りさせない
  **Source**: A-1ro (owner) — crates/kei_emit/src/emit.rs:935 (3rd pass review, CONFIRMED/pitfalls 🔴)
  **Lesson**: `class`/`var`/`null`/`this` 等の TypeScript 予約語がラムダパラメータ名として使われると、emit した TS が `tsc` で parse 不能になる。check.rs でラムダパラメータ名を検証する際に TS 予約語リストと照合し、予約語なら KEI-E 系エラーを発火させること(golden テストで `err_type_lambda_param_ts_reserved` として固定済み)。

- **Pattern**: `old(...)` walker は check と emit で重複しないよう構造化する
  **Source**: A-1ro (owner) — crates/kei_check/src/check.rs:3549 (3rd pass review, follow-up PR 行き)
  **Lesson**: `old(...)` を走査する walker が check.rs と emit.rs に 2 本同形で存在すると、一方に変更を加えたとき他方が同期漏れになる(M25 全体の構造的問題)。follow-up PR で共通 walker を `kei_check` または新クレートに切り出し、双方から参照する形に整理すること。

## PR #84: chore: bump version to 0.4.2 — 2026-06-29

(no actionable patterns)

## PR #84 (issue-check re-run): post-merge-lessons PostToolUse — 2026-06-29

(no actionable patterns — hook triggered by Bash tool call checking issue states (`gh issue view` loop for issues #54–#62), not by `gh pr merge`; most recent merged PR #84 already documented above)
