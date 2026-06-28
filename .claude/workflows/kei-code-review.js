export const meta = {
  name: "kei-code-review",
  description: "Kei 専用コードレビュー。CLAUDE.md / ARCHITECTURE.md / docs/dev-notes/ を scope で 1 回読んで全 finder に注入し、Kei 向け 5 角度(invariant-auditor + correctness + pitfalls + cleanup + altitude)で finder を回す。verify 前に file+行近傍で dedup し、verifier 呼び出し数を組み込み /code-review より半減させる。args は \"<level> [target]\" 形式で level は high|xhigh|max、target は PR 番号 / ブランチ / パス / 自由指示。target に --comment が含まれていれば、findings は呼び元(skill)で GitHub PR の inline comment として投稿される。",
  whenToUse: "Kei コンパイラ(この Rust ワークスペース)の PR / ブランチ diff をレビューするとき。組み込み /code-review よりも Kei 固有不変条件(golden 契約 / Diagnostic 三要件 / クレート依存方向 / spec-first)と過去 PR の dev-notes 教訓をきっちり押さえつつ、トークンを抑えたいときに使う。",
  phases: [
    { title: "Scope", detail: "diff command + Kei context (CLAUDE.md / ARCHITECTURE.md / dev-notes) を 1 回読む" },
    { title: "Find", detail: "5 角度の finder(invariant-auditor / correctness / pitfalls / cleanup / altitude)を並列実行" },
    { title: "Dedup", detail: "file + 行近傍で候補をクラスタリングし、代表 1 件に集約" },
    { title: "Verify", detail: "dedup 後の候補のみ独立 verifier で CONFIRMED / PLAUSIBLE / REFUTED 判定" },
    { title: "Synthesize", detail: "ランク付け・最終キャップ" },
  ],
}

// ─── Level parameters ───
const LEVEL_PARAMS = {
  high:  { perAngle: 5, maxFindings: 10, sweep: false },
  xhigh: { perAngle: 7, maxFindings: 15, sweep: false }, // dedup でカバーするので sweep は max のみに残す
  max:   { perAngle: 8, maxFindings: 15, sweep: true },
}

const RAW_ARGS = (typeof args === "string" ? args : "").trim()
const FIRST = RAW_ARGS.split(/\s+/)[0] || ""
const FIRST_IS_LEVEL = Object.prototype.hasOwnProperty.call(LEVEL_PARAMS, FIRST)
const LEVEL = FIRST_IS_LEVEL ? FIRST : "xhigh"
const TARGET = FIRST_IS_LEVEL ? RAW_ARGS.slice(FIRST.length).trim() : RAW_ARGS
const P = LEVEL_PARAMS[LEVEL]

// ─── Schemas ───
const SCOPE_SCHEMA = {
  type: "object", required: ["diffCommand", "files", "summary", "keiContext"],
  properties: {
    diffCommand: { type: "string" },
    files: { type: "array", items: { type: "string" } },
    summary: { type: "string" },
    keiContext: {
      type: "string",
      description: "CLAUDE.md の不変条件 + ARCHITECTURE.md の依存契約 + docs/dev-notes/*.md の過去 PR 教訓を 1 ブロックに集約した、finder 向けプレロード文字列。各項目は箇条書きで簡潔に。",
    },
    affectedAreas: {
      type: "array",
      items: { enum: ["kei_syntax", "kei_check", "kei_fmt", "kei_emit", "kei_cli", "kei_mcp", "spec", "examples", "golden", "runtime", "ci", "docs", "other"] },
      description: "diff が触っている領域。kei-invariants 角度の重点判断に使う",
    },
  },
}
const CANDIDATES_SCHEMA = {
  type: "object", required: ["candidates"],
  properties: {
    candidates: { type: "array", items: {
      type: "object", required: ["file", "summary", "failure_scenario"],
      properties: {
        file: { type: "string" },
        line: { type: "number" },
        summary: { type: "string" },
        failure_scenario: { type: "string" },
      },
    }},
  },
}
const VERDICT_SCHEMA = {
  type: "object", required: ["verdict", "evidence"],
  properties: {
    verdict: { enum: ["CONFIRMED", "PLAUSIBLE", "REFUTED"] },
    evidence: { type: "string" },
  },
}
const REPORT_SCHEMA = {
  type: "object", required: ["summary", "decisions"],
  properties: {
    summary: { type: "string" },
    decisions: { type: "array", items: {
      type: "object", required: ["index"],
      properties: {
        index: { type: "number" },
        merge: { type: "array", items: { type: "number" } },
      },
    }},
  },
}

// ─── Phase 0: Scope (Kei context もここで 1 回だけ集約) ───
phase("Scope")
const scope = await agent(
  "## Kei コードレビューの scope 設定\n\n" +
  (TARGET
    ? "レビュー対象 / 指示(ユーザ verbatim): \"" + TARGET + "\"。PR 番号 / ブランチ / ref / パスが含まれていれば diff コマンドを組み立てる。自由指示(特定ファイルだけ等)も尊重し、それ以外は現在ブランチの diff('git diff @{upstream}...HEAD' → 'git diff main...HEAD' → 'git diff HEAD~1' の順でフォールバック)を使う。\n"
    : "明示的な対象なし — 現在ブランチをレビュー: 'git diff @{upstream}...HEAD' を優先(fallback: 'git diff main...HEAD' / 'git diff HEAD~1')。未コミット変更があれば 'git diff HEAD' も含める。\n") +
  "\n以下を 1 回で集約してください:\n\n" +
  "1. **diff コマンド** を確定し、実際に走らせて非空であることを確認。\n" +
  "2. **changed files** を列挙。\n" +
  "3. **affectedAreas** を crates/ ディレクトリ・spec/・examples/・tests/golden/ などへの paths から分類。\n" +
  "4. 何が変わったかを 1 段落で **summary**。\n" +
  "5. **keiContext** を組み立てる(これが本ワークフロー最大の節約ポイント。finder が再読しないで済むよう、必要な情報を一度に詰める):\n" +
  "   - リポジトリルートの CLAUDE.md を読み、**不変条件 1〜5**(golden が契約本文 / Diagnostic は span+code+fix / spec が SoT / runtime は独立 npm / 正規形維持)を箇条書きで抜粋。\n" +
  "   - ARCHITECTURE.md を読み、クレート依存契約(kei_syntax ← kei_fmt / kei_check ← kei_emit / kei_cli・kei_mcp は処理委譲)を抜粋。\n" +
  "   - `docs/dev-notes/` 配下に PR 教訓ファイルがあれば全部読み、**過去 PR で踏んだ地雷**(同じ指摘を二度踏まないため)を file 名付きで列挙。なければ \"(no dev-notes lessons)\" と書く。\n" +
  "   - HANDOFF.md が存在し affectedAreas と関連しそうなら、関連項目だけ抜粋。\n" +
  "   - 全体を 200 行以内に収める(finder への注入用なので冗長禁止、出典 path と要点のみ)。\n\n" +
  "Structured output only.",
  { label: "scope", schema: SCOPE_SCHEMA }
)
if (!scope) return { error: "Scope agent returned no result." }
if (!scope.files || scope.files.length === 0) {
  return { level: LEVEL, target: TARGET || undefined, summary: "No changes found to review.", findings: [], stats: { finders: 0, candidates: 0, verified: 0 } }
}
log(LEVEL + " review: " + scope.files.length + " files, areas=" + (scope.affectedAreas || []).join(","))

const SCOPE_BLOCK =
  "## レビュー対象\n" +
  "diff コマンド: " + scope.diffCommand + "\n" +
  "変更ファイル (" + scope.files.length + "): " + scope.files.join(", ") + "\n" +
  "影響領域: " + (scope.affectedAreas || []).join(", ") + "\n\n" +
  "## 変更概要\n" + scope.summary + "\n\n" +
  "## Kei コンテキスト(プレロード — 各 finder が CLAUDE.md / dev-notes を再読しないで済むよう scope phase で 1 回だけ集約済み)\n" +
  scope.keiContext + "\n" +
  (TARGET ? "\n## ユーザ指示(verbatim)\n" + TARGET + "\n上記の scope 制約・focus は finder のデフォルト角度より優先する。\n" : "")

// ─── 5 つの角度 ───
const ANGLES = [
  {
    label: "kei-invariants",
    agentType: "kei-invariant-auditor",
    prompt:
      "Kei コンパイラの diff を **CLAUDE.md の不変条件と ARCHITECTURE.md の依存契約だけ** に照らして監査してください。\n\n" +
      SCOPE_BLOCK + "\n" +
      "重点: (1) golden 改変 / (2) クレート依存逆流 / (3) Diagnostic span+code+fix 欠落 / (4) spec-implementation 乖離 / (5) 正規形・null/例外禁止・import 明示。\n" +
      "監査レポートそのものではなく、**candidate finding の配列** で返してください(各 finding: file / line / summary / failure_scenario)。failure_scenario には「どの不変条件番号に違反するか」を明記。",
  },
  {
    label: "correctness",
    prompt:
      "Kei コンパイラの diff を correctness 観点でレビュー。\n\n" +
      SCOPE_BLOCK + "\n" +
      "全角度を 1 つに圧縮: (A) 行ごとの誤り / 反転条件・off-by-one・誤変数 copy-paste、(B) 削除された不変条件・ガード・テストの再確立漏れ、(C) caller / callee への波及で壊れる事前条件・戻り値形・例外、(D) ラッパー / プロキシ / Visitor のメソッド転送漏れ。\n" +
      "Kei コンテキストの不変条件・dev-notes 教訓に照らして「過去 PR で踏んだ地雷を再踏」していないかを最優先で確認。",
  },
  {
    label: "pitfalls",
    prompt:
      "Kei コンパイラの diff を **Rust / TypeScript / .kei 言語固有の落とし穴** だけに絞ってレビュー。\n\n" +
      SCOPE_BLOCK + "\n" +
      "Rust: derive(PartialEq/Hash) の順序依存・unchecked cast(as i64)・integer overflow(pow / mul)・unwrap / panic 経路・clone コスト・lifetime 漏れ・enum match 網羅性。\n" +
      "TypeScript(kei_emit 出力 / runtime): 浮動小数 == / falsy-zero / 暗黙的 Number coerce / async 漏れ。\n" +
      ".kei(examples / golden): null 使用・例外 throw・wildcard import・契約式での副作用。\n" +
      "diff が新たに導入したインスタンスのみ。既存コードは対象外。",
  },
  {
    label: "cleanup",
    prompt:
      "Kei コンパイラの diff の cleanup(reuse / simplification / efficiency)観点でレビュー。\n\n" +
      SCOPE_BLOCK + "\n" +
      "reuse: 既存ヘルパ(`cartesian`・`emit_*`・`Diagnostic::new` 等)の再実装、別関数の copy-paste。\n" +
      "simplification: 冗長な分岐・derive で済む手書き・3 段以上のネスト・dead code。\n" +
      "efficiency: ホットパスの冗長計算 / 過剰 clone / Vec::with_capacity の見積過剰 / 並列化可能な逐次 I/O。\n" +
      "failure_scenario には「具体的コスト」(何が重複か・保守時に同期が必要な箇所がどこか・どれくらいメモリ無駄か)を書く。correctness バグは絶対に出さない(他角度の担当)。",
  },
  {
    label: "altitude",
    prompt:
      "Kei コンパイラの diff の **altitude(修正粒度の妥当性)** だけをレビュー。\n\n" +
      SCOPE_BLOCK + "\n" +
      "重点: 共通機構の上に乗せた特殊ケース(本来下の層を一般化すべき)・絆創膏的な if 分岐・spec 側で直すべきものを実装側のワークアラウンドで隠している箇所・Diagnostic を発行すべき所で `None` を返して暗黙降格しているケース(過去 PR で頻発、dev-notes 参照)。\n" +
      "Kei は spec-first なので、実装で対症療法していたら必ず指摘。",
  },
]

const FINDER_PROMPT = a =>
  "## Kei コードレビュー finder — " + a.label + "\n\n" +
  a.prompt + "\n\n" +
  "candidate finding を最大 " + P.perAngle + " 件まで返す。各 finding は file / line / summary(1 行)/ failure_scenario(具体的なユーザ可視結果。中間状態の言及ではなく「どんな入力 / 状態で何が起きるか」)。何も見つからなければ空配列。\n\n" +
  "Structured output only."

// ─── Find: 5 角度を並列実行 ───
phase("Find")
const findResults = await parallel(
  ANGLES.map(a => () => {
    const opts = { label: a.label, phase: "Find", schema: CANDIDATES_SCHEMA }
    if (a.agentType) opts.agentType = a.agentType
    return agent(FINDER_PROMPT(a), opts).then(r => {
      if (!r) return []
      const sliced = (r.candidates || []).slice(0, P.perAngle)
      log(a.label + ": " + sliced.length + " candidates")
      return sliced.map(c => ({ ...c, angle: a.label }))
    })
  })
)
const allCandidates = findResults.filter(Boolean).flat()

// ─── Dedup: file + 行近傍(15 行以内)で同根とみなしクラスタの代表 1 件だけ verify へ ───
phase("Dedup")
function dedup(cs) {
  const byFile = new Map()
  for (const c of cs) {
    if (!byFile.has(c.file)) byFile.set(c.file, [])
    byFile.get(c.file).push(c)
  }
  const out = []
  for (const [, list] of byFile) {
    list.sort((a, b) => (a.line ?? 0) - (b.line ?? 0))
    let cluster = []
    const flush = () => {
      if (cluster.length === 0) return
      cluster.sort((x, y) => (y.failure_scenario?.length ?? 0) - (x.failure_scenario?.length ?? 0))
      const rep = cluster[0]
      const others = cluster.slice(1).map(c => (c.file + ":" + (c.line ?? "?") + " (" + c.angle + ")"))
      out.push({ ...rep, also_reported_by: others })
      cluster = []
    }
    for (const c of list) {
      if (cluster.length === 0) { cluster.push(c); continue }
      const last = cluster[cluster.length - 1]
      if (c.line != null && last.line != null && c.line - last.line <= 15) cluster.push(c)
      else { flush(); cluster.push(c) }
    }
    flush()
  }
  return out
}
const deduped = dedup(allCandidates)
log("dedup: " + allCandidates.length + " → " + deduped.length + " (saved " + (allCandidates.length - deduped.length) + " verifier calls)")

// ─── Verify ───
phase("Verify")
const VERIFIER_PROMPT = c =>
  "## Kei コードレビュー verifier\n\n" + SCOPE_BLOCK + "\n" +
  "## 候補\n" +
  "File: " + c.file + (c.line != null ? ":" + c.line : "") + "\n" +
  "Angle: " + c.angle + "\n" +
  "Summary: " + c.summary + "\n" +
  "Failure scenario: " + c.failure_scenario + "\n" +
  (c.also_reported_by && c.also_reported_by.length > 0
    ? "Also reported at: " + c.also_reported_by.join(", ") + "(同根の可能性あり。verify は代表の file:line を見るが、これら他箇所が独立した別バグなら REFUTED ではなく分けて確認する)\n"
    : "") + "\n" +
  "diff コマンドを実行し、該当ファイルを Read して **唯一の verdict** を返す:\n\n" +
  "- **CONFIRMED** — 引き金となる入力・状態と、誤出力 / クラッシュを名指しできる。該当行を引用。\n" +
  "- **PLAUSIBLE** — メカニズムは実在、引き金が未確定(タイミング / env / config)。confirm に必要な条件を述べる。\n" +
  "- **REFUTED** — 事実誤認(コードがそう書いていない)/ 別箇所でガード済み。証拠の行を引用。\n\n" +
  "PLAUSIBLE by default — 「投機的だから」「runtime state 依存だから」という理由で REFUTED にしない。コードから構成可能な事実誤認 / 不可能性証明 / 既存ガードがあるときだけ REFUTED。\n\n" +
  "Structured output only."

const verified = (await parallel(deduped.map(c => () =>
  agent(VERIFIER_PROMPT(c), { label: "verify:" + c.angle, phase: "Verify", schema: VERDICT_SCHEMA })
    .then(v => (v ? { ...c, verdict: v.verdict, evidence: v.evidence } : null))
))).filter(Boolean)

const surviving = verified.filter(c => c.verdict !== "REFUTED")
const refuted = verified.filter(c => c.verdict === "REFUTED")
log("Verify done: " + verified.length + " → " + surviving.length + " kept, " + refuted.length + " refuted")

// ─── Sweep (max のみ): kei-invariants 角度でもう 1 回、新規候補だけ ───
if (P.sweep) {
  phase("Sweep")
  const known = surviving.length > 0
    ? surviving.map(c => "- " + c.file + (c.line != null ? ":" + c.line : "") + " — " + c.summary).join("\n")
    : "(none)"
  const sweep = await agent(
    "## Kei sweep — 既出を除いた追加候補のみ\n\n" + SCOPE_BLOCK + "\n" +
    "## 既出候補(再導出禁止)\n" + known + "\n\n" +
    "上を踏まえ、見落としがちな:移設で落ちたガード・対称関数で片方だけ未更新・golden と実装の同期漏れ・dev-notes の地雷再踏(特に \"hook が静かに発火するが何も書かれない\" 系の不可視回帰)を探す。最大 6 件。なければ空。\n\nStructured output only.",
    { label: "sweep", phase: "Sweep", schema: CANDIDATES_SCHEMA }
  )
  if (sweep && sweep.candidates && sweep.candidates.length > 0) {
    const sliced = sweep.candidates.slice(0, 6).map(c => ({ ...c, angle: "sweep" }))
    log("sweep: " + sliced.length + " candidates")
    const sweepVerified = (await parallel(sliced.map(c => () =>
      agent(VERIFIER_PROMPT(c), { label: "verify:sweep", phase: "Sweep", schema: VERDICT_SCHEMA })
        .then(v => (v ? { ...c, verdict: v.verdict, evidence: v.evidence } : null))
    ))).filter(Boolean)
    for (const v of sweepVerified) {
      if (v.verdict !== "REFUTED") surviving.push(v)
      else refuted.push(v)
    }
  }
}

const stats = {
  level: LEVEL,
  finders: ANGLES.length,
  candidatesRaw: allCandidates.length,
  candidatesDeduped: deduped.length,
  verified: verified.length,
  refuted: refuted.length,
}

if (surviving.length === 0) {
  return { level: LEVEL, target: TARGET || undefined, summary: "No findings survived verification.", findings: [], stats }
}

// ─── Synthesize: ランク + キャップ ───
phase("Synthesize")
// invariants > correctness > pitfalls > cleanup > altitude(altitude は重大度低めとして cleanup と同列扱い)
const angleRank = { "kei-invariants": 0, correctness: 1, pitfalls: 2, sweep: 2, cleanup: 4, altitude: 4 }
const verdictRank = v => (v === "CONFIRMED" ? 0 : 1)
const rank = c => (angleRank[c.angle] ?? 3) * 2 + verdictRank(c.verdict)
const ranked = surviving.slice().sort((a, b) => rank(a) - rank(b))
const block = ranked.map((c, i) =>
  "### [" + i + "] " + c.file + (c.line != null ? ":" + c.line : "") + " (" + c.verdict + " / " + c.angle + ")\n" +
  c.summary + "\nFailure scenario: " + c.failure_scenario + "\nVerifier evidence: " + c.evidence + "\n"
).join("\n")

const report = await agent(
  "## Kei コードレビュー最終レポート\n\n" +
  ranked.length + " 件が verify を通過(" + LEVEL + " effort)。\n\n" + block + "\n" +
  "## Instructions\n" +
  "1. index ベースで決定を返す(本文を再生成しない)。\n" +
  "2. 同じ defect を別角度で報告したものは 1 件にまとめ、他は merge 配列に index を入れる。\n" +
  "3. 重要度順(invariants → correctness → pitfalls → cleanup/altitude)で最大 " + P.maxFindings + " 件。\n" +
  "4. 2〜3 文の summary を書く(Kei 不変条件への影響を中心に)。\n\nStructured output only.",
  { label: "synthesize", schema: REPORT_SCHEMA }
)

const decisions = report && Array.isArray(report.decisions) ? report.decisions : []
const valid = i => Number.isInteger(i) && i >= 0 && i < ranked.length
const loc = c => c.file + (c.line != null ? ":" + c.line : "")
const seen = new Set()
const claim = i => (valid(i) && !seen.has(i) ? (seen.add(i), true) : false)
const findings = []
for (const d of decisions) {
  if (findings.length >= P.maxFindings) break
  if (!claim(d.index)) continue
  const c = ranked[d.index]
  const merged = (Array.isArray(d.merge) ? d.merge : []).filter(claim).map(i => ranked[i])
  const verdict = merged.some(m => m.verdict === "CONFIRMED") ? "CONFIRMED" : c.verdict
  const also = merged.length > 0 ? " [same root cause also at: " + merged.map(loc).join(", ") + "]" : ""
  findings.push({ file: c.file, line: c.line, summary: c.summary + also, failure_scenario: c.failure_scenario, verdict, angle: c.angle })
}
for (let i = 0; i < ranked.length && findings.length < P.maxFindings; i++) {
  if (seen.has(i)) continue
  const c = ranked[i]
  findings.push({ file: c.file, line: c.line, summary: c.summary, failure_scenario: c.failure_scenario, verdict: c.verdict, angle: c.angle })
}
const summary = report && report.summary
  ? report.summary
  : "Synthesis step was unusable — returning verified findings ranked, unmerged."

return {
  level: LEVEL,
  target: TARGET || undefined,
  summary,
  findings,
  refuted: refuted.map(c => ({ file: c.file, line: c.line, summary: c.summary, angle: c.angle })),
  stats: { ...stats, reported: findings.length },
}
