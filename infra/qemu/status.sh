#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# QEMU VM — Status
# ============================================================================
#
# Usage:
#   bash infra/qemu/status.sh              # show all VMs
#   bash infra/qemu/status.sh ubuntu       # show one VM
# ============================================================================

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/_lib.sh"

show_vm() {
    local name="$1"
    local vm_dir="$SCRIPT_DIR/vm/$name"
    local pid_file="$vm_dir/qemu.pid"
    local info_file="$vm_dir/info.env"
    local mac
    mac=$(vm_mac "$name")

    echo "=== $name (MAC $mac) ==="

    # Check if running
    if [[ -f "$pid_file" ]] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
        echo "  Status: running (PID $(cat "$pid_file"))"
    else
        echo "  Status: stopped"
        # Show saved info if available
        if [[ -f "$info_file" ]]; then
            echo "  Last known IP: $(grep VM_IP "$info_file" 2>/dev/null | cut -d= -f2 || echo "unknown")"
        fi
        return
    fi

    # Live IP discovery via ARP
    local ip
    ip=$(find_guest_ip "$mac" 2>/dev/null || true)
    if [[ -n "$ip" ]]; then
        echo "  IP: $ip"
        if check_ssh "$ip"; then
            echo "  SSH: reachable"
        else
            echo "  SSH: not responding"
        fi
    else
        echo "  IP: not found (VM may still be booting)"
    fi

    # Socket info
    [[ -S "$vm_dir/monitor.sock" ]] && echo "  Monitor: $vm_dir/monitor.sock"
    [[ -S "$vm_dir/serial.sock" ]]  && echo "  Serial:  $vm_dir/serial.sock"
}

case "${1:-ubuntu}" in
    ubuntu) show_vm "$1" ;;
    *)      die "Unknown VM '$1' (use: ubuntu)" ;;
esac
