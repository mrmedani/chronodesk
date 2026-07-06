#!/bin/bash
set -euo pipefail

SERVER_IP="${1:?Usage: $0 <server_ip> <turn_user> <turn_pass>}"
TURN_USER="${2:?Usage: $0 <server_ip> <turn_user> <turn_pass>}"
TURN_PASS="${3:?Usage: $0 <server_ip> <turn_user> <turn_pass>}"
SSH_USER="${4:-ubuntu}"

echo "==> Deploying CHRONODESK TURN server to $SERVER_IP"

# Copy config files
rsync -avz --progress server/turn/ "$SSH_USER@$SERVER_IP":~/chronodesk-turn/

ssh "$SSH_USER@$SERVER_IP" <<EOF
set -euo pipefail
cd ~/chronodesk-turn

# Replace placeholders using python (avoids shell escaping issues)
python3 -c "
import os
path = 'turnserver.conf'
with open(path) as f: c = f.read()
c = c.replace('<YOUR_SERVER_IP>', '$SERVER_IP')
c = c.replace('<TURN_USER>:<TURN_PASS>', '$TURN_USER:$TURN_PASS')
with open(path, 'w') as f: f.write(c)
print('Config updated')
"

# Install docker if needed
if ! command -v docker &>/dev/null; then
  curl -fsSL https://get.docker.com | sh
  sudo usermod -aG docker "$USER"
fi

# Start coturn
sudo docker compose down -t 0 2>/dev/null || true
sudo docker compose up -d

echo ""
echo "==> TURN server running: turn:$SERVER_IP:3478"
echo "==> Username: $TURN_USER"
echo "==> Password: $TURN_PASS"
echo "==> Logs: sudo docker logs chronodesk-turn -f"
EOF
