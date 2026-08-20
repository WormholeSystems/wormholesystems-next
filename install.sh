#!/bin/sh
# One-line installer for wsctl, the WormholeSystems setup tool:
#
#   curl --proto '=https' --tlsv1.2 -sSf https://install.wormhole.systems | sh
#
# Pin a version with WSCTL_VERSION=v0.1.0. Choose where it lands with WSCTL_BIN_DIR.
set -eu

REPO="${WSCTL_REPO:-WormholeSystems/wormholesystems-next}"
VERSION="${WSCTL_VERSION:-latest}"

case "$(uname -s)-$(uname -m)" in
	Linux-x86_64)  TARGET=x86_64-unknown-linux-gnu ;;
	Linux-aarch64) TARGET=aarch64-unknown-linux-gnu ;;
	Darwin-x86_64) TARGET=x86_64-apple-darwin ;;
	Darwin-arm64)  TARGET=aarch64-apple-darwin ;;
	*)
		echo "wsctl has no build for $(uname -s) $(uname -m)." >&2
		echo "Clone the repository and run: cargo run -p wsctl -- setup" >&2
		exit 1
		;;
esac

if [ "$VERSION" = latest ]; then
	URL="https://github.com/$REPO/releases/latest/download/wsctl-$TARGET"
else
	URL="https://github.com/$REPO/releases/download/$VERSION/wsctl-$TARGET"
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
if ! curl -fsSL -o "$TMP" "$URL"; then
	echo "Download failed. Is there a release for $TARGET yet?" >&2
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
read -r answer < /dev/tty 2>/dev/null || answer=n
case "$answer" in
	[nN]*) echo "Run \`wsctl setup\` whenever you are ready." ;;
	*) exec "$BIN_DIR/wsctl" setup < /dev/tty ;;
esac
