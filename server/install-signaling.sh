#!/bin/bash
set -euo pipefail

REPO="https://github.com/mrmedani/chronodesk.git"
PORT="${1:-21116}"
BIND="0.0.0.0"

echo "==> Installing CHRONODESK Signaling Server on port $PORT"

# 1. Install Rust
if ! command -v cargo &>/dev/null; then
  echo "==> Installing Rust..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi

# 2. Clone & build
if [ ! -d "$HOME/chronodesk" ]; then
  echo "==> Cloning repository..."
  git clone "$REPO" "$HOME/chronodesk"
fi

echo "==> Building signaling-server (release)..."
cd "$HOME/chronodesk"
cargo build --release --bin signaling-server

# 3. systemd service
echo "==> Creating systemd service..."
sudo tee /etc/systemd/system/chronodesk-signaling.service > /dev/null <<SYSTEMD
[Unit]
Description=CHRONODESK Signaling Server
After=network.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$HOME/chronodesk
ExecStart=$HOME/chronodesk/target/release/signaling-server --bind $BIND:$PORT
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
SYSTEMD

sudo systemctl daemon-reload
sudo systemctl enable chronodesk-signaling.service
sudo systemctl restart chronodesk-signaling.service

# 4. Firewall
echo "==> Opening port $PORT/tcp..."
sudo ufw allow "$PORT"/tcp 2>/dev/null || true

# 5. Done
IP=$(curl -4 -s ifconfig.me 2>/dev/null || echo "YOUR_SERVER_IP")
echo ""
echo "============================================"
echo "  CHRONODESK Signaling Server is RUNNING"
echo "============================================"
echo ""
echo "  Status: $(sudo systemctl is-active chronodesk-signaling.service)"
echo "  Address: $IP:$PORT"
echo ""
echo "  Configure in Flutter app -> Settings:"
echo "    Signaling Server: $IP:$PORT"
echo ""
echo "  Logs: sudo journalctl -u chronodesk-signaling -f"
echo "  Stop: sudo systemctl stop chronodesk-signaling"
echo ""
