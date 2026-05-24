#!/usr/bin/env bash
# 🦾 Anos — AI Native OS
# Install script — user-space only, no root needed.
# Usage: curl -fsSL https://raw.githubusercontent.com/datnp1003/anos/main/install.sh | bash
# Dev:   ANOS_BRANCH=dev_lor curl -fsSL https://raw.githubusercontent.com/datnp1003/anos/dev_lor/install.sh | bash
set -e

echo "🦾 Anos — AI Native OS Installer"
echo "================================="
echo ""

# Detect OS/arch
OS=$(uname -s)
ARCH=$(uname -m)
if [ "$OS" != "Linux" ]; then
    echo "❌ Anos requires Linux. Detected: $OS"
    exit 1
fi

case "$ARCH" in
    aarch64|arm64) ANOS_ARCH="arm64" ;;
    x86_64|amd64) ANOS_ARCH="x86_64" ;;
    *) ANOS_ARCH="" ;;
esac

# User-space install dirs / knobs
INSTALL_DIR="${ANOS_INSTALL_DIR:-$HOME/.anos}"
BIN_DIR="${ANOS_BIN_DIR:-$HOME/.local/bin}"
ANOS_BRANCH="${ANOS_BRANCH:-main}"
ANOS_VERSION="${ANOS_VERSION:-latest}"
ANOS_BUILD_FROM_SOURCE="${ANOS_BUILD_FROM_SOURCE:-0}"
REPO="datnp1003/anos"

mkdir -p "$BIN_DIR"

ensure_runtime_assets() {
    if ! command -v git >/dev/null 2>&1; then
        echo "⚠️ git not found; runtime assets may be missing (ANOS-SYSTEM-PROMPT.md, skills)."
        return 0
    fi
    if [ -d "$INSTALL_DIR/.git" ]; then
        cd "$INSTALL_DIR"
        git fetch origin "$ANOS_BRANCH" >/dev/null 2>&1 || true
        git checkout "$ANOS_BRANCH" >/dev/null 2>&1 || true
        git pull --ff-only origin "$ANOS_BRANCH" >/dev/null 2>&1 || true
    else
        rm -rf "$INSTALL_DIR"
        git clone --depth 1 --branch "$ANOS_BRANCH" https://github.com/$REPO.git "$INSTALL_DIR" >/dev/null 2>&1 || true
    fi
    mkdir -p "$INSTALL_DIR/config"
}

install_launcher() {
    cat > "$BIN_DIR/anos" << 'LAUNCHER'
#!/usr/bin/env bash
ANOS_DIR="${ANOS_DIR:-$HOME/.anos}"
ANOSD_BIN="${ANOSD_BIN:-$HOME/.local/bin/anosd}"
ANOS_CLI_BIN="${ANOS_CLI_BIN:-$HOME/.local/bin/anos-cli}"
SOCK="/tmp/anos.sock"
export ANOS_DIR

STOP=false
if [ -S "$SOCK" ]; then
    echo "/ping" | nc -U -w 1 "$SOCK" 2>/dev/null | grep -q pong || { rm -f "$SOCK"; STOP=true; }
else
    STOP=true
fi
if [ "$STOP" = true ]; then
    mkdir -p "$ANOS_DIR"
    LOG="${ANOS_LOG:-$ANOS_DIR/anosd.log}"
    "$ANOSD_BIN" >>"$LOG" 2>&1 &
    for _ in $(seq 1 50); do [ -S "$SOCK" ] && break; sleep 0.1; done
fi
"$ANOS_CLI_BIN" "$@"
RC=$?
if [ "$STOP" = true ]; then
    pkill -f "$ANOSD_BIN" 2>/dev/null || true
    rm -f "$SOCK"
fi
exit $RC
LAUNCHER
    chmod +x "$BIN_DIR/anos"
}

try_binary_install() {
    if [ "$ANOS_BUILD_FROM_SOURCE" = "1" ]; then
        return 1
    fi
    if [ -z "$ANOS_ARCH" ]; then
        echo "⚠️ Unsupported binary arch '$ARCH'; falling back to source build."
        return 1
    fi
    if ! command -v curl >/dev/null 2>&1; then
        echo "⚠️ curl not found; falling back to source build."
        return 1
    fi

    local tag="$ANOS_VERSION"
    if [ "$tag" = "latest" ]; then
        if [ "$ANOS_BRANCH" = "dev_lor" ]; then
            tag="v0.1.1-dev.1"
        else
            tag="latest"
        fi
    fi

    local base="https://github.com/$REPO/releases"
    if [ "$tag" = "latest" ]; then
        base="$base/latest/download"
    else
        base="$base/download/$tag"
    fi

    local anosd_url="$base/anosd-linux-$ANOS_ARCH"
    local cli_url="$base/anos-cli-linux-$ANOS_ARCH"
    local tmp
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' RETURN

    echo "⬇️  Trying binary install ($ANOS_ARCH, $tag)..."
    if curl -fL "$anosd_url" -o "$tmp/anosd" && curl -fL "$cli_url" -o "$tmp/anos-cli"; then
        cp -f "$tmp/anosd" "$BIN_DIR/anosd"
        cp -f "$tmp/anos-cli" "$BIN_DIR/anos-cli"
        chmod +x "$BIN_DIR/anosd" "$BIN_DIR/anos-cli"
        ensure_runtime_assets
        install_launcher
        echo "✅ Installed release binaries."
        return 0
    fi

    echo "⚠️ Binary assets unavailable for $ANOS_ARCH/$tag; falling back to source build."
    return 1
}

source_build_install() {
    if ! command -v git >/dev/null 2>&1; then
        echo "❌ git is required for source install."
        exit 1
    fi

    if ! command -v rustc >/dev/null 2>&1; then
        echo "📦 Installing Rust (user-space)..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck disable=SC1091
        source "$HOME/.cargo/env"
    fi

    echo "✅ Rust: $(rustc --version)"

    if [ -d "$INSTALL_DIR/.git" ]; then
        echo "📦 Updating Anos ($ANOS_BRANCH)..."
        cd "$INSTALL_DIR"
        git fetch origin "$ANOS_BRANCH"
        git checkout "$ANOS_BRANCH"
        git pull --ff-only origin "$ANOS_BRANCH"
    else
        echo "📦 Cloning Anos ($ANOS_BRANCH)..."
        rm -rf "$INSTALL_DIR"
        git clone --branch "$ANOS_BRANCH" https://github.com/$REPO.git "$INSTALL_DIR"
        cd "$INSTALL_DIR"
    fi

    echo "🔨 Building anosd..."
    cd anosd && cargo build --release && cd ..
    echo "🔨 Building anos-cli..."
    cd anos-cli && cargo build --release && cd ..

    cp -f "$INSTALL_DIR/anosd/target/release/anosd" "$BIN_DIR/anosd"
    cp -f "$INSTALL_DIR/anos-cli/target/release/anos-cli" "$BIN_DIR/anos-cli"
    chmod +x "$BIN_DIR/anosd" "$BIN_DIR/anos-cli"
    install_launcher
}

try_binary_install || source_build_install

echo ""
echo "✅ Anos installed!"
echo ""
echo "  Add to PATH:  export PATH=\"$BIN_DIR:\$PATH\""
echo "  Run:          anos"
echo "  One-shot:     anos 'How much disk space is free?'"
echo ""
echo "  🦾 Enjoy your AI-native OS!"
