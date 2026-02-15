#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# QEMU VM — Graceful Shutdown
# ============================================================================
#
# Usage:
#   sudo bash infra/qemu/kill.sh ubuntu
#   sudo bash infra/qemu/kill.sh --all
# ============================================================================

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/_lib.sh"

kill_vm() {
    local name="$1"
    local vm_dir="$SCRIPT_DIR/vm/$name"
    local pid_file="$vm_dir/qemu.pid"
    local monitor_sock="$vm_dir/monitor.sock"

    # Graceful quit via QEMU monitor
    if [[ -S "$monitor_sock" ]]; then
        log "[$name] Sending quit to QEMU monitor..."
        echo "quit" | socat - "UNIX-CONNECT:$monitor_sock" 2>/dev/null || true
        sleep 2
    fi

    # Kill by PID
    if [[ -f "$pid_file" ]]; then
        local pid
        pid=$(cat "$pid_file")
        if kill -0 "$pid" 2>/dev/null; then
            log "[$name] Killing QEMU (PID $pid)..."
            kill "$pid" 2>/dev/null || true
            sleep 1
            if kill -0 "$pid" 2>/dev/null; then
                kill -9 "$pid" 2>/dev/null || true
            fi
        fi
        rm -f "$pid_file"
    fi

    # Clean up sockets
    rm -f "$vm_dir/monitor.sock" "$vm_dir/serial.sock"
    log "[$name] Stopped."
}

case "${1:?Usage: kill.sh <ubuntu|--all>}" in
    ubuntu)  kill_vm "$1" ;;
    --all)   kill_vm ubuntu ;;
    *)       die "Unknown target '$1' (use: ubuntu|--all)" ;;
esac
