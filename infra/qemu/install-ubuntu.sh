#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# Ubuntu 24.04 — Cloud Image Setup (No Install Required)
# ============================================================================
#
# Downloads the official Ubuntu cloud image (pre-built qcow2), resizes it,
# generates a cloud-init seed ISO, and boots. Ready to SSH in ~30 seconds.
#
# Prerequisites:
#   - qemu-system-aarch64, socat (brew install qemu socat)
#   - Internet access (to download cloud image on first run)
#
# Usage:
#   sudo bash infra/qemu/install-ubuntu.sh
#
# After setup:
#   source infra/qemu/vm/ubuntu/info.env
#   ssh ubuntu@$VM_IP   # password: VMware123!
# ============================================================================

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/_lib.sh"

MAC=$(vm_mac ubuntu)
CLOUD_INIT_DIR="$SCRIPT_DIR/cloud-init"
SEED_ISO="$CLOUD_INIT_DIR/seed.iso"
VM_DIR="$SCRIPT_DIR/vm/ubuntu"
DISK="$VM_DIR/disk.qcow2"

CLOUD_IMG_URL="https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-arm64.img"
CLOUD_IMG="$SCRIPT_DIR/iso/noble-server-cloudimg-arm64.img"

DISK_SIZE=${DISK_SIZE:-20G}

START_TIME=$(date +%s)
elapsed() { local e=$(( $(date +%s) - START_TIME )); echo "$((e/60))m$((e%60))s"; }

# --- Preflight ---
step "Preflight checks"

[[ $EUID -eq 0 ]] || die "Requires sudo (vmnet-shared). Run: sudo bash $0"
command -v qemu-system-aarch64 >/dev/null || die "qemu-system-aarch64 not found (brew install qemu)"
command -v qemu-img >/dev/null || die "qemu-img not found (brew install qemu)"
command -v socat >/dev/null || die "socat not found (brew install socat)"

[[ -f "$CLOUD_INIT_DIR/user-data" ]] || die "Missing $CLOUD_INIT_DIR/user-data"
[[ -f "$CLOUD_INIT_DIR/meta-data" ]] || die "Missing $CLOUD_INIT_DIR/meta-data"

# --- Download cloud image if needed ---
step "Preparing Ubuntu cloud image"

mkdir -p "$SCRIPT_DIR/iso"
if [[ -f "$CLOUD_IMG" ]]; then
    log "Cloud image already downloaded: $CLOUD_IMG"
else
    log "Downloading Ubuntu 24.04 cloud image (~600MB)..."
    curl -L -o "$CLOUD_IMG" "$CLOUD_IMG_URL"
    [[ -f "$CLOUD_IMG" ]] || die "Download failed"
    log "Downloaded: $(stat -f%z "$CLOUD_IMG" | awk '{printf "%.0fMB", $1/1048576}') "
fi

# --- Prepare disk from cloud image ---
if [[ -f "$DISK" ]]; then
    log "WARNING: Disk already exists at $DISK"
    log "  Delete it for a fresh setup: rm -f $DISK $VM_DIR/efivars.fd"
    log "  Continuing with existing disk..."
else
    mkdir -p "$VM_DIR"
    log "Copying cloud image → $DISK"
    cp "$CLOUD_IMG" "$DISK"
    log "Resizing disk to $DISK_SIZE..."
    qemu-img resize "$DISK" "$DISK_SIZE"
    log "Disk ready: $DISK ($DISK_SIZE)"
fi

# --- Build seed ISO ---
step "Building cloud-init seed ISO"

SEED_TMP=$(mktemp -d)
cp "$CLOUD_INIT_DIR/user-data" "$SEED_TMP/"
cp "$CLOUD_INIT_DIR/meta-data" "$SEED_TMP/"

rm -f "$SEED_ISO"
hdiutil makehybrid -o "$SEED_ISO" \
    -iso -joliet \
    -iso-volume-name "cidata" \
    -joliet-volume-name "cidata" \
    "$SEED_TMP" >/dev/null
rm -rf "$SEED_TMP"

[[ -f "$SEED_ISO" ]] || die "Failed to create seed ISO"
log "Seed ISO: $SEED_ISO ($(stat -f%z "$SEED_ISO") bytes)"

# --- Boot ---
step "Booting Ubuntu VM from cloud image"

bash "$SCRIPT_DIR/boot.sh" ubuntu --bg \
    --cdrom "$SEED_ISO"

# --- Wait for SSH ---
step "Waiting for cloud-init to finish and SSH to become available..."
log "You can watch the serial console:"
log "  socat -,rawer unix-connect:$VM_DIR/serial.sock"

UBUNTU_IP=$(wait_for_ip_and_ssh "$MAC" 300) || die "Timed out waiting for SSH (5 min). Check serial console."

# --- Verify ---
step "Verifying Ubuntu VM"

if command -v sshpass >/dev/null; then
    UNAME=$(sshpass -p 'VMware123!' ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        ubuntu@"$UBUNTU_IP" "uname -a" 2>/dev/null || echo "VERIFY FAILED")
    log "  $UNAME"
else
    log "Verify manually: ssh ubuntu@$UBUNTU_IP (password: VMware123!)"
fi

# --- Write connection info ---
write_info_env ubuntu "$UBUNTU_IP" ubuntu "VMware123!"

step "Ubuntu VM ready! ($(elapsed) total)"
log "  SSH: ssh ubuntu@$UBUNTU_IP (password: VMware123!)"
log "  SSH: ssh root@$UBUNTU_IP   (password: VMware123!)"
log ""
log "To boot this VM later:"
log "  sudo bash infra/qemu/boot.sh ubuntu --bg"
