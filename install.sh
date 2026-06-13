#!/bin/sh
# Kei CLI installer.
#
# プラットフォームを判定し、GitHub Releases から対応する `kei` バイナリを
# ダウンロードして PATH 上のディレクトリに置く。Rust ツールチェインは不要。
#
#   curl -fsSL https://raw.githubusercontent.com/A-1ro/kei-lang/main/install.sh | sh
#
# 環境変数で上書き可能:
#   KEI_VERSION       導入するタグ(既定: 最新リリース。例: v0.1.0)
#   KEI_INSTALL_DIR   インストール先(既定: $HOME/.local/bin)
#
# Windows は対象外。Rust があれば代わりに次を使う:
#   cargo install --git https://github.com/A-1ro/kei-lang.git kei_cli

set -eu

REPO="A-1ro/kei-lang"
BIN="kei"
INSTALL_DIR="${KEI_INSTALL_DIR:-$HOME/.local/bin}"

err() {
	printf 'install: error: %s\n' "$1" >&2
	exit 1
}

# ダウンローダ: curl か wget のどちらかを使う。
if command -v curl >/dev/null 2>&1; then
	dl() { curl -fsSL "$1"; }
	dlo() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
	dl() { wget -qO- "$1"; }
	dlo() { wget -qO "$2" "$1"; }
else
	err "curl か wget が必要です"
fi

command -v tar >/dev/null 2>&1 || err "tar が必要です"

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
Linux) os_t="unknown-linux-gnu" ;;
Darwin) os_t="apple-darwin" ;;
*) err "未対応の OS: $os(cargo install --git を使ってください)" ;;
esac

case "$arch" in
x86_64 | amd64) arch_t="x86_64" ;;
arm64 | aarch64) arch_t="aarch64" ;;
*) err "未対応のアーキテクチャ: $arch" ;;
esac

target="${arch_t}-${os_t}"

# バージョン解決: 未指定なら最新リリースの tag_name を取得する。
version="${KEI_VERSION:-}"
if [ -z "$version" ]; then
	version="$(dl "https://api.github.com/repos/${REPO}/releases/latest" |
		sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
	[ -n "$version" ] || err "最新バージョンを取得できません(KEI_VERSION を指定してください)"
fi

asset="kei-${target}.tar.gz"
url="https://github.com/${REPO}/releases/download/${version}/${asset}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

printf 'install: %s %s (%s) を取得します\n' "$BIN" "$version" "$target" >&2
dlo "$url" "$tmp/$asset" || err "ダウンロード失敗: $url"

tar -xzf "$tmp/$asset" -C "$tmp" || err "アーカイブの展開に失敗しました"

mkdir -p "$INSTALL_DIR"
if install -m 0755 "$tmp/kei-${target}/${BIN}" "$INSTALL_DIR/${BIN}" 2>/dev/null; then
	:
else
	cp "$tmp/kei-${target}/${BIN}" "$INSTALL_DIR/${BIN}"
	chmod 0755 "$INSTALL_DIR/${BIN}"
fi

printf 'install: %s を %s に置きました\n' "$BIN" "$INSTALL_DIR/${BIN}" >&2

# PATH に入っていなければ案内する。
case ":$PATH:" in
*":$INSTALL_DIR:"*) ;;
*)
	printf 'install: %s が PATH にありません。シェルの設定に次を追加してください:\n' "$INSTALL_DIR" >&2
	printf '  export PATH="%s:$PATH"\n' "$INSTALL_DIR" >&2
	;;
esac
