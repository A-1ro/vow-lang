---
name: release-bump
description: Kei ワークスペースのリリースバージョンを上げる定型手順。Cargo.toml(workspace version)・plugin.json・marketplace.json・MCP golden を更新し、機能リリースならバンドルスキルも追従させ、検証 → PR、マージ後にタグ + GitHub Release まで通す。「バージョン上げて」「bump して」「リリース準備」と言われたときに使う。
---

# release-bump — バージョン bump とリリース

Kei の版数は **ワークスペース一括**(全クレートが `version.workspace = true`)。bump は数箇所の機械的更新 + 検証 + PR + タグ/Release の定型。直近実績は 0.2.1 → 0.3.0(PR #51 / tag v0.3.0)。

採番は semver。新機能あり = マイナー(0.2.x → 0.3.0)、修正のみ = パッチ(0.3.0 → 0.3.1)。以下 `X.Y.Z` を新バージョンとする。

## 1. ブランチ

最新 main から bump 用ブランチを切る(main で直接コミットしない)。

```
git checkout main && git pull origin main
git checkout -b chore/bump-X.Y.Z
```

## 2. 版数を上げるファイル(4 箇所 + lock)

| 対象 | 何を | 方法 |
|---|---|---|
| `Cargo.toml` | `[workspace.package]` の `version` | 手で編集 |
| `.claude-plugin/plugin.json` | `version` | 手で編集 |
| `.claude-plugin/marketplace.json` | `plugins[].version` | 手で編集 |
| `Cargo.lock` | ワークスペースクレートの版数 | `cargo build` で再生成 |
| `tests/mcp/initialize.response.json` | `serverInfo.version` | **手で編集せず** golden 再生成(下記) |

MCP の version は `env!("CARGO_PKG_VERSION")` 由来。Cargo.toml を上げてから golden を再生成する:

```
UPDATE_GOLDEN=1 cargo test -p kei_mcp --test golden_mcp
```

**対象外(別管理・触らない)**: `runtime/`(独立 npm パッケージ・独自版数)、`editors/vscode`(別系統)。

## 3. 機能リリースならバンドルスキルを追従させる(重要)

新機能を出すリリースで版数だけ上げると、**バンドルされる `skills/kei/SKILL.md` が旧版のまま**になり、プラグイン更新者が新機能を「使うな」と誤誘導される(= リリースが実質 stale。v0.3 で `List<T>` を「未実装」と書いたままにし Codex に指摘された実例あり)。新機能を含むなら必ず:

- `skills/kei/SKILL.md` … 新機能の記述を実態に更新(追加するコード例は `kei check` / `kei fmt --check` クリーンにする)。参照節に新 spec を追加。
- `.claude-plugin/plugin.json` の `description` … 末尾の `(vX: …)` 機能一覧を新版に更新(`marketplace.json` の description は版数タグ無しなので通常そのまま)。
- spec / examples を増やしたら、MCP は `build.rs` が `spec/**/*.md`・`examples/**/*.kei` を動的収集するので埋め込みは自動。`UPDATE_GOLDEN=1 cargo test -p kei_mcp` で MCP golden(`tests/mcp/*.response.json`)を追従させる。

パッチ(修正のみ)の bump ならこの節はスキップしてよい。

## 4. 検証

- `kei-verify` skill で fmt / clippy / test を一括。golden を再生成したら **必ず無印で再実行**して一致を確認する。
- e2e がローカルの lockfile を汚すので元に戻す:
  ```
  git checkout -- tests/cli/projects/app/package-lock.json tests/e2e/package-lock.json
  ```
- 触った `.kei`(スキルの例など)は `kei check` / `kei fmt --check` クリーン。

## 5. コミット → PR

```
git commit -m "chore: bump version to X.Y.Z"   # 本文に対象/対象外を明記
git push -u origin chore/bump-X.Y.Z
gh pr create --base main --title "chore: bump version to X.Y.Z" --body ...
```

コミットメッセージ末尾は `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`、PR 本文末尾は `🤖 Generated with [Claude Code](https://claude.com/claude-code)`。

## 6. マージ後: タグと GitHub Release

PR マージ後に main を最新化してから:

```
git checkout main && git pull origin main
git tag -a vX.Y.Z <merge-commit> -F -   # annotated。メッセージに変更概要
git push origin vX.Y.Z
```

タグ規約は `vX.Y.Z`。**タグ push を受けて release ワークフロー(`github-actions[bot]`)が GitHub Release を自動作成**する(自動生成ノート形式・Latest)。`gh release create` を別に叩くと重複で 422 になるので叩かない。

機能リリースは自動ノート(PR 列挙のみ)だと中身が伝わらないので、ハイライト/破壊的変更/対応 Issue を先頭に足して充実させる(自動の What's Changed は残す):

```
gh release edit vX.Y.Z --title "vX.Y.Z — <一言>" --notes "<ハイライト>\n\n---\n\n<元の What's Changed>"
```

対応 Issue があれば、`Closes #` で自動クローズされていない分を `gh issue close <n> --reason completed --comment "..."` で手動クローズする。

## チェックリスト

- [ ] Cargo.toml / plugin.json / marketplace.json の version を X.Y.Z に
- [ ] `cargo build` で Cargo.lock 再生成
- [ ] `UPDATE_GOLDEN=1 cargo test -p kei_mcp` で MCP golden 追従
- [ ] (機能リリース)skills/kei/SKILL.md と plugin.json description を新版に追従
- [ ] kei-verify pass(fmt/clippy/test)、golden 無印で再確認
- [ ] e2e lockfile を git checkout で復元
- [ ] commit(Co-Authored-By)→ push → PR
- [ ] (マージ後)annotated タグ vX.Y.Z を push、Release ノートを充実、関連 Issue クローズ
