#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# Deploy Modbus TCP Slave to QEMU Ubuntu VM
# ============================================================================
#
# Uploads modbus_slave.py, installs pymodbus, starts the slave process.
# Uses SSH key auth (ed25519 injected via cloud-init).
#
# Usage:
#   bash infra/qemu/deploy-slave.sh
#
# After deploy:
#   ssh ubuntu@<ip> "sudo journalctl -u modbus-slave -f"
# ============================================================================

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/_lib.sh"

INFO_FILE="$SCRIPT_DIR/vm/ubuntu/info.env"
[[ -f "$INFO_FILE" ]] || die "VM info not found: $INFO_FILE. Run install-ubuntu.sh first."
source "$INFO_FILE"

SSH="ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5"
SCP="scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null"

# --- Verify VM is reachable ---
step "Checking VM connectivity ($VM_IP)"

if ! check_ssh "$VM_IP"; then
    log "Saved IP $VM_IP not responding, scanning..."
    MAC=$(vm_mac ubuntu)
    NEW_IP=$(find_guest_ip "$MAC" 2>/dev/null || true)
    if [[ -n "$NEW_IP" ]] && check_ssh "$NEW_IP"; then
        log "Found VM at $NEW_IP (was $VM_IP)"
        VM_IP="$NEW_IP"
    else
        die "Cannot reach Ubuntu VM. Is it running? Try: sudo bash infra/qemu/boot.sh ubuntu --bg"
    fi
fi

log "VM reachable at $VM_IP"

# --- Upload modbus-slave ---
step "Uploading modbus-slave to VM"

$SCP -r "$SCRIPT_DIR/modbus-slave/" ubuntu@"$VM_IP":/home/ubuntu/
log "Files uploaded"

# --- Install dependencies ---
step "Installing pymodbus"

$SSH ubuntu@"$VM_IP" "sudo pip3 install --break-system-packages -q pymodbus 2>&1 | tail -3"
log "pymodbus installed"

# --- Start slave via systemd ---
step "Starting Modbus TCP slave"

$SSH ubuntu@"$VM_IP" "sudo systemctl stop modbus-slave.service 2>/dev/null; sudo systemctl reset-failed modbus-slave.service 2>/dev/null; true"
sleep 1

$SSH ubuntu@"$VM_IP" "sudo systemd-run --unit=modbus-slave python3 /home/ubuntu/modbus-slave/modbus_slave.py --port 502"
sleep 2

# --- Verify ---
step "Verifying Modbus TCP port 502"

ACTIVE=$($SSH ubuntu@"$VM_IP" "sudo systemctl is-active modbus-slave.service" 2>/dev/null || true)
if [[ "$ACTIVE" == "active" ]]; then
    log "modbus-slave.service is active"
else
    die "modbus-slave.service failed. Check: ssh ubuntu@$VM_IP 'sudo journalctl -u modbus-slave --no-pager'"
fi

if printf '' | nc -G 2 -w 2 "$VM_IP" 502 >/dev/null 2>&1; then
    log "Modbus TCP port 502 is open on $VM_IP"
else
    log "WARNING: Port 502 not responding from host. Firewall?"
fi

# --- Update config ---
TOML="$SCRIPT_DIR/../../config/hal_modbus_tcp.toml"
if [[ -f "$TOML" ]]; then
    sed -i '' "s/^host = .*/host = \"$VM_IP\"/" "$TOML"
    log "Updated config → host = \"$VM_IP\""
fi

# --- Update info.env ---
write_info_env ubuntu "$VM_IP" ubuntu "VMware123!"

step "Modbus slave deployed!"
log "  Slave:  ubuntu@$VM_IP:502 (Modbus TCP)"
log "  Logs:   ssh ubuntu@$VM_IP 'sudo journalctl -u modbus-slave -f'"
log "  Stop:   ssh ubuntu@$VM_IP 'sudo systemctl stop modbus-slave'"
log "  Restart: bash infra/qemu/deploy-slave.sh"
