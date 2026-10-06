#!/bin/sh
set -e

REPO="${GITHUB_REPO:-cismuc/tx}"
INSTALL_DIR="${TX_INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS
OS_NAME="$(uname -s)"
case "$OS_NAME" in
  Darwin)
    OS="macos"
    ;;
  Linux)
    OS="linux"
    ;;
  *)
    echo "Error: Unsupported operating system: $OS_NAME" >&2
    exit 1
    ;;
esac

# Detect Architecture
ARCH_NAME="$(uname -m)"
case "$ARCH_NAME" in
  x86_64|amd64)
    ARCH="x86_64"
    ;;
  arm64|aarch64)
    ARCH="arm64"
    ;;
  armv7*|armhf)
    if [ "$OS" = "linux" ]; then
      ARCH="armv7"
    else
      echo "Error: Unsupported architecture: $ARCH_NAME on $OS" >&2
      exit 1
    fi
    ;;
  *)
    echo "Error: Unsupported architecture: $ARCH_NAME" >&2
    exit 1
    ;;
esac

ASSET_NAME="tx-${OS}-${ARCH}"
DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ASSET_NAME}"

echo "Installing tx (${OS}-${ARCH})..."

mkdir -p "$INSTALL_DIR"
TARGET_FILE="$INSTALL_DIR/tx"
TMP_FILE="$(mktemp "${TMPDIR:-/tmp}/tx.XXXXXX")"

# Download binary
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$DOWNLOAD_URL" -o "$TMP_FILE"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$TMP_FILE" "$DOWNLOAD_URL"
else
  echo "Error: curl or wget is required to install tx" >&2
  exit 1
fi

chmod +x "$TMP_FILE"
mv "$TMP_FILE" "$TARGET_FILE"

echo "Installed tx to $TARGET_FILE"

# PATH verification
case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    ;;
  *)
    echo ""
    echo "Warning: $INSTALL_DIR is not in your PATH."
    echo "Add it to your shell configuration:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    echo ""
    ;;
esac

# Verify execution
if command -v tx >/dev/null 2>&1; then
  echo "Installation complete: $(tx --version)"
else
  echo "Installation complete: $($TARGET_FILE --version)"
fi
