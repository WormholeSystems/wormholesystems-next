#!/bin/sh
# One-line installer for wsctl, the WormholeSystems setup tool:
#
#   curl --proto '=https' --tlsv1.2 -sSf https://install-next.wormhole.systems | sh
#
# Pin a version with WSCTL_VERSION=v0.1.0. Choose where it lands with WSCTL_BIN_DIR.
set -eu

REPO="${WSCTL_REPO:-WormholeSystems/wormholesystems-next}"
VERSION="${WSCTL_VERSION:-latest}"

case "$(uname -s)-$(uname -m)" in
	Linux-x86_64)  TARGET=x86_64-unknown-linux-gnu ;;
	Linux-aarch64) TARGET=aarch64-unknown-linux-gnu ;;
	Darwin-arm64)  TARGET=aarch64-apple-darwin ;;
	*)
		echo "wsctl has no released build for $(uname -s) $(uname -m)." >&2
		echo "Build it from a checkout instead:" >&2
		echo "  cargo run -p wsctl -- setup" >&2
		exit 1
		;;
esac

ASSET="wsctl-$TARGET"
TOKEN="${WSCTL_TOKEN:-${GITHUB_TOKEN:-${GH_TOKEN:-}}}"

# A public release is a plain download. A private one is not reachable that way at all, so
# with a token we go through the API instead: find the asset by name, then ask for its bytes.
if [ -n "$TOKEN" ]; then
	if [ "$VERSION" = latest ]; then
		RELEASE="https://api.github.com/repos/$REPO/releases/latest"
	else
		RELEASE="https://api.github.com/repos/$REPO/releases/tags/$VERSION"
	fi
	# The response is pretty-printed, so flatten it first: one asset object per line puts
	# each id beside the name it belongs to. GitHub lists `name` before the nested
	# `uploader`, so splitting on the brace keeps the two together.
	ASSET_ID="$(
		curl -fsSL -H "Authorization: Bearer $TOKEN" \
			-H "Accept: application/vnd.github+json" "$RELEASE" |
			tr -d '\n' | tr '{' '\n' |
			grep "\"name\"[[:space:]]*:[[:space:]]*\"$ASSET\"" |
			head -1 |
			sed -n 's/.*"id"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p'
	)"
	if [ -z "$ASSET_ID" ]; then
		echo "No asset named $ASSET in $VERSION of $REPO." >&2
		echo "Check the release exists and the token can read the repository." >&2
		exit 1
	fi
	URL="https://api.github.com/repos/$REPO/releases/assets/$ASSET_ID"
elif [ "$VERSION" = latest ]; then
	URL="https://github.com/$REPO/releases/latest/download/$ASSET"
else
	URL="https://github.com/$REPO/releases/download/$VERSION/$ASSET"
fi

# /usr/local/bin when it is ours to write to, so it is already on PATH.
if [ -n "${WSCTL_BIN_DIR:-}" ]; then
	BIN_DIR="$WSCTL_BIN_DIR"
elif [ -d /usr/local/bin ] && [ -w /usr/local/bin ]; then
	BIN_DIR=/usr/local/bin
else
	BIN_DIR="$HOME/.local/bin"
fi
mkdir -p "$BIN_DIR"

echo "Downloading $URL"
# To a temporary file first: a half-written binary in place is worse than none.
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
if [ -n "$TOKEN" ]; then
	fetch() {
		curl -fsSL -H "Authorization: Bearer $TOKEN" -H "Accept: application/octet-stream" \
			-o "$1" "$2"
	}
else
	fetch() { curl -fsSL -o "$1" "$2"; }
fi

if ! fetch "$TMP" "$URL"; then
	echo "Download failed." >&2
	echo "If $REPO is private, set WSCTL_TOKEN to a token that can read it." >&2
	exit 1
fi
chmod +x "$TMP"
mv "$TMP" "$BIN_DIR/wsctl"
trap - EXIT

echo "Installed $("$BIN_DIR/wsctl" --version) to $BIN_DIR/wsctl"

case ":$PATH:" in
	*":$BIN_DIR:"*) ;;
	*) echo "Note: $BIN_DIR is not on your PATH." ;;
esac

# stdin is the pipe this script arrived through, so anything interactive has to read the
# terminal itself. With no terminal there is nothing to prompt, and that is fine.
[ -r /dev/tty ] || exit 0

printf 'Run the setup now? [Y/n] '
# Braced so a /dev/tty that exists but cannot be opened fails quietly: the redirect's own
# error is the shell's, not the read's, and would otherwise print before the fallback.
{ read -r answer < /dev/tty; } 2>/dev/null || answer=n
case "$answer" in
	[nN]*) echo "Run \`wsctl setup\` whenever you are ready." ;;
	*) exec "$BIN_DIR/wsctl" setup < /dev/tty ;;
esac
