#!/bin/bash
set -euo pipefail

REPO="https://github.com/mrmedani/chronodesk.git"
PORT="${1:-21116}"
BIND="0.0.0.0"
SSH_USER="${2:-ubuntu}"
HOME_DIR="/home/$SSH_USER"

echo "==> Installing CHRONODESK Signaling Server on port $PORT"

# 1. Install system deps
sudo apt-get update -qq
sudo apt-get install -y -qq git curl build-essential pkg-config libssl-dev \
  libwayland-dev libxkbcommon-dev libdbus-1-dev libclang-dev llvm-dev

# 2. Install Rust
if ! command -v cargo &>/dev/null; then
  echo "==> Installing Rust..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
fi

# 3. Clone & build
if [ ! -d "$HOME_DIR/chronodesk" ]; then
  echo "==> Cloning repository..."
  git clone "$REPO" "$HOME_DIR/chronodesk"
fi

echo "==> Building signaling-server (release)..."
cd "$HOME_DIR/chronodesk"
export LIBCLANG_PATH=/usr/lib/llvm-14/lib
export PATH="$HOME/.cargo/bin:$PATH"

# Strip heavy deps for faster build (signaling only needs network crates)
cp Cargo.toml Cargo.toml.full
cat > Cargo.toml << 'CARGOEOF'
[package]
name = "chronodesk-signaling"
version = "0.4.2"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
dashmap = "6"
futures = "0.3"
ring = "0.17"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"

[[bin]]
name = "signaling-server"
path = "src/bin/signaling.rs"
CARGOEOF

cargo build --release --bin signaling-server
mv Cargo.toml.full Cargo.toml

# 4. Generate auth secret
AUTH_SECRET=$(openssl rand -hex 32)
echo "$AUTH_SECRET" > "$HOME_DIR/chronodesk/signaling-secret.txt"

# 5. systemd service
echo "==> Creating systemd service..."
sudo tee /etc/systemd/system/chronodesk-signaling.service > /dev/null <<SYSTEMD
[Unit]
Description=CHRONODESK Signaling Server
After=network.target

[Service]
Type=simple
User=$SSH_USER
WorkingDirectory=$HOME_DIR/chronodesk
ExecStart=$HOME_DIR/chronodesk/target/release/signaling-server --bind $BIND:$PORT --auth-secret $AUTH_SECRET
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
SYSTEMD

sudo systemctl daemon-reload
sudo systemctl enable chronodesk-signaling.service
sudo systemctl restart chronodesk-signaling.service

# 6. Firewall
echo "==> Opening port $PORT/tcp..."
sudo ufw allow "$PORT"/tcp 2>/dev/null || true
sudo ufw --force enable 2>/dev/null || true

# 7. Done
IP=$(curl -4 -s ifconfig.me 2>/dev/null || echo "$SERVER_IP")
echo ""
echo "============================================"
echo "  CHRONODESK Signaling Server is RUNNING"
echo "============================================"
echo ""
echo "  Status: $(sudo systemctl is-active chronodesk-signaling.service)"
echo "  Address: $IP:$PORT"
echo "  Auth Secret: $AUTH_SECRET"
echo ""
echo "  Logs: sudo journalctl -u chronodesk-signaling -f"
echo "  Stop: sudo systemctl stop chronodesk-signaling"
echo ""
