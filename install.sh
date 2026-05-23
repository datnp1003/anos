#!/usr/bin/env bash
# 🦾 Anos — AI Native OS
# Install script — user-space only, no root needed.
# Usage: curl -fsSL https://raw.githubusercontent.com/datnp1003/anos/main/install.sh | bash
set -e

echo "🦾 Anos — AI Native OS Installer"
echo "================================="
echo ""

# Detect OS
OS=$(uname -s)
ARCH=$(uname -m)
if [ "$OS" != "Linux" ]; then
    echo "❌ Anos requires Linux. Detected: $OS"
    exit 1
fi

# Determine install dir
INSTALL_DIR="${ANOS_INSTALL_DIR:-$HOME/.anos}"
BIN_DIR="${ANOS_BIN_DIR:-$HOME/.local/bin}"

mkdir -p "$BIN_DIR"

# Check if Rust is installed
if ! command -v rustc >/dev/null 2>&1; then
    echo "📦 Installing Rust (user-space)..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo "✅ Rust: $(rustc --version)"

# Clone Anos
if [ -d "$INSTALL_DIR" ]; then
    echo "📦 Updating Anos..."
    cd "$INSTALL_DIR"
    git pull --ff-only origin main || true
else
    echo "📦 Cloning Anos..."
    git clone https://github.com/datnp1003/anos.git "$INSTALL_DIR"
    cd "$INSTALL_DIR"
fi

# Build
echo "🔨 Building anosd..."
cd anosd && cargo build --release && cd ..
echo "🔨 Building anos-cli..."
cd anos-cli && cargo build --release && cd ..

# Install binaries
cp -f "$INSTALL_DIR/anosd/target/release/anosd" "$BIN_DIR/anosd"
cp -f "$INSTALL_DIR/anos-cli/target/release/anos-cli" "$BIN_DIR/anos-cli"
chmod +x "$BIN_DIR/anosd" "$BIN_DIR/anos-cli"

# Create anos launcher
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
    "$ANOSD_BIN" &
    for _ in $(seq 1 30); do [ -S "$SOCK" ] && break; sleep 0.1; done
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

echo ""
echo "✅ Anos installed!"
echo ""
echo "  Add to PATH:  export PATH=\"$BIN_DIR:\$PATH\""
echo "  Run:          anos"
echo "  One-shot:     anos 'còn bao nhiêu disk?'"
echo ""
echo "  🦾 Enjoy your AI-native OS!"
