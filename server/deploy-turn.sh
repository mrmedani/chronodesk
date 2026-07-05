#!/bin/bash
set -euo pipefail

SERVER_IP="${1:?Usage: $0 <server_ip> <turn_user> <turn_pass>}"
TURN_USER="${2:?Usage: $0 <server_ip> <turn_user> <turn_pass>}"
TURN_PASS="${3:?Usage: $0 <server_ip> <turn_user> <turn_pass>}"

echo "==> Deploying CHRONODESK TURN server to $SERVER_IP"

rsync -avz --progress server/turn/ root@"$SERVER_IP":/root/chronodesk-turn/

ssh root@"$SERVER_IP" <<EOF
set -euo pipefail
cd /root/chronodesk-turn

# Replace placeholders
sed -i "s/<YOUR_SERVER_IP>/$SERVER_IP/g" turnserver.conf
sed -i "s/<TURN_USER>:<TURN_PASS>/$TURN_USER:$TURN_PASS/g" turnserver.conf

# Install docker if needed
if ! command -v docker &>/dev/null; then
  curl -fsSL https://get.docker.com | sh
fi

# Start coturn
docker compose up -d

echo ""
echo "==> TURN server running: turn:$SERVER_IP:3478"
echo "==> Username: $TURN_USER"
echo "==> Password: $TURN_PASS"
EOF
