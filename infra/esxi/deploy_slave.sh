#!/usr/bin/env bash
set -euo pipefail

# Deploy modbus_slave.py to a running Linux VM via SSH.
# Assumes the VM is reachable at localhost:2222 (default port forwarding).
#
# Usage:
#   ./deploy_slave.sh [--host localhost] [--port 2222] [--user root]

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
SLAVE_DIR="$SCRIPT_DIR/../modbus-slave"

HOST="${HOST:-localhost}"
SSH_PORT="${SSH_PORT:-2222}"
USER="${USER:-root}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --host) HOST="$2"; shift 2 ;;
        --port) SSH_PORT="$2"; shift 2 ;;
        --user) USER="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

echo "Deploying Modbus slave to $USER@$HOST:$SSH_PORT"

# Copy files
scp -P "$SSH_PORT" \
    "$SLAVE_DIR/modbus_slave.py" \
    "$SLAVE_DIR/requirements.txt" \
    "$USER@$HOST:/tmp/"

# Install deps and start
ssh -p "$SSH_PORT" "$USER@$HOST" bash -s <<'REMOTE'
set -e
cd /tmp
pip install -r requirements.txt 2>/dev/null || pip3 install -r requirements.txt
echo "Starting Modbus TCP slave on port 502..."
nohup python3 modbus_slave.py --port 502 --cycle-ms 100 > /tmp/modbus_slave.log 2>&1 &
echo "PID: $!"
sleep 1
if kill -0 $! 2>/dev/null; then
    echo "Modbus slave running (PID $!)"
else
    echo "ERROR: slave failed to start"
    cat /tmp/modbus_slave.log
    exit 1
fi
REMOTE

echo "Done. Modbus TCP slave is running on $HOST:502"
echo "From host: connect to localhost:5502 (port-forwarded)"
