---
name: kei-code-review
description: Kei コンパイラ(この Rust ワークスペース)の PR / ブランチ diff を、Kei 固有不変条件と過去 PR 教訓に押さえてレビューする。組み込み /code-review よりトークン消費が抑えられる(角度を Kei 向け 5 個に圧縮 + verify 前 dedup)。`--comment` で findings を GitHub PR の inline comment として投稿。「Kei レビューして」「PR#NN を Kei 用にレビュー」と言われたら呼ぶ。
---

# kei-code-review — Kei 専用コードレビュー

Kei コンパイラの diff を、組み込み `/code-review` よりも:
- **Kei 不変条件 / dev-notes 教訓に押さえる**(CLAUDE.md・ARCHITECTURE.md・`docs/dev-notes/` を scope phase で 1 回だけ集約して全 finder に注入)
- **トークン消費を抑える**(finder 角度を 10 → 5 に圧縮 + verify 前に file+行近傍で dedup)
- **`kei-invariant-auditor` を finder の 1 つとして組み込む**

レビューする。組み込み `/code-review` の汎用角度(conventions・angle-A/B/C/D/E 等)が Kei では重複しがちだった点を解消した。

## 起動方法

`Workflow({ name: "kei-code-review", args: "<level> [target]" })`

- `level`: `high` / `xhigh`(既定)/ `max`
  - high: 角度 5・perAngle 5・cap 10
  - xhigh: 角度 5・perAngle 7・cap 15(sweep なし)
  - max: 角度 5・perAngle 8・cap 15 + sweep
- `target`: PR 番号(`PR#76`)/ ブランチ(`branch-name`)/ ref 範囲 / パス / 自由指示 / `--comment` フラグ
- 例:
  - `args: "xhigh PR#76"` — PR #76 を xhigh でレビュー(コメント投稿なし、テキスト報告のみ)
  - `args: "xhigh PR#76 --comment"` — PR #76 を xhigh でレビュー後、findings を inline PR comment として投稿
  - `args: "high"` — 現在ブランチを high で軽くレビュー
  - `args: "max crates/kei_check/ をだけ"` — kei_check の変更だけ max effort で

## findings の投稿(--comment)

`target` に `--comment` が含まれていれば、workflow 完了後に呼び元(本スキル)で findings を GitHub PR の inline comment として投稿する。手順:

1. `gh pr view <PR番号> --json number,headRefOid,baseRefName,headRefName,url` で PR メタを取る。owner/repo は `git remote get-url origin` から抽出するか `A-1ro/kei-lang` を既定とする。
2. `mcp__github__pull_request_review_write` を method=`create` で呼び、pending review を作成(commitID は headRefOid)。
3. 各 finding を `mcp__github__add_comment_to_pending_review` で追加。
   - `subjectType: "LINE"` + `side: "RIGHT"` で当該行に inline コメント。
   - finding の line が **diff hunk 外**(本 PR で未改変だが新経路から流入する既存コードを指摘する場合)は `subjectType: "FILE"` を使い、本文冒頭に「※ 既存関数(本 PR では未改変)だが、本 PR で X 経路が新設されたため新たな regression として顕在化」と注釈。
4. `mcp__github__pull_request_review_write` を method=`submit_pending`, event=`COMMENT` で submit。

ファイルパスはリポジトリルートからの相対パス(`crates/kei_check/src/pbt.rs` 形式)。workflow が返す絶対パスは投稿前に `/Users/.../kei-lang/` プレフィックスを除去する。

`--comment` が無いときは findings をターミナルに重要度順で提示するだけ。

## 5 つの角度

| 角度 | 担当 | 備考 |
|---|---|---|
| `kei-invariants` | CLAUDE.md / ARCHITECTURE.md 不変条件 | `kei-invariant-auditor` サブエージェントを起動 |
| `correctness` | 行ごとの誤り・削除されたガード・caller/callee 波及・wrapper の転送漏れ | 組み込みの angle-A/B/C/E を 1 角度に圧縮 |
| `pitfalls` | Rust/TS/.kei 固有の落とし穴 | derive 順序依存・unchecked cast・falsy zero・契約式の副作用 など |
| `cleanup` | reuse / simplification / efficiency | correctness バグは出さない |
| `altitude` | 修正粒度の妥当性 | 特殊ケース乱発・spec 側で直すべきものの実装ワークアラウンド |

## 報告フォーマット

workflow 戻り値の `findings[]` を重要度順(invariants → correctness → pitfalls → cleanup/altitude / CONFIRMED → PLAUSIBLE)で提示する。各 finding は file:line・verdict・angle・summary・failure_scenario。

`--comment` モードでは投稿先 PR URL も最後に表示。

## トレードオフ

- 組み込み `/code-review` の挙動更新(ranking / verify pass 改良)には追従しないので、半年に 1 回程度は built-in script(`~/.claude/projects/.../workflows/scripts/code-review-*.js`)と diff を取って参考にする。
- 5 角度に圧縮しているので、Kei 文脈外の汎用バグ(例: 純粋に Rust のジェネリック規約違反)を取りこぼす可能性がある。**最終リリース直前は組み込み `/code-review max` も併走**するのが安全。

## 関連

- `kei-invariant-auditor` サブエージェント — 不変条件監査(本 workflow が finder の 1 つとして利用)
- `kei-verify` スキル — `cargo fmt --check` + clippy + test の完了条件検証(レビューの前後に併走)
- `docs/dev-notes/` — 過去 PR 教訓の蓄積(scope phase で本 workflow に注入される)
