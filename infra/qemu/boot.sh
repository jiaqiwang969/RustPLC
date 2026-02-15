#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# QEMU aarch64 Ubuntu VM — Boot Script
# ============================================================================
#
# Boots an Ubuntu VM with vmnet-shared networking (macOS vmnet.framework).
# VM gets a real IP on 192.168.2.0/24 via bridge100.
# Requires sudo (vmnet.framework needs root).
#
# Usage:
#   sudo bash infra/qemu/boot.sh ubuntu              # foreground (Ctrl-A X to quit)
#   sudo bash infra/qemu/boot.sh ubuntu --bg          # background
#   sudo bash infra/qemu/boot.sh ubuntu --bg --cdrom /path/to/iso  # with CD-ROM
#
# After boot:
#   ssh ubuntu@<ip>    # password: VMware123!
#   ssh root@<ip>      # password: VMware123!
# ============================================================================

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/_lib.sh"

# --- Parse args ---
NAME="${1:?Usage: sudo bash boot.sh ubuntu [--bg] [--cdrom path]...}"
shift

MAC=$(vm_mac "$NAME")

VM_DIR="$SCRIPT_DIR/vm/$NAME"
DISK="$VM_DIR/disk.qcow2"
EFIVARS="$VM_DIR/efivars.fd"
MONITOR_SOCK="$VM_DIR/monitor.sock"
SERIAL_SOCK="$VM_DIR/serial.sock"
PID_FILE="$VM_DIR/qemu.pid"
QEMU_LOG="$VM_DIR/qemu.log"

EFI_CODE="/opt/homebrew/share/qemu/edk2-aarch64-code.fd"
EFI_VARS_TEMPLATE="/opt/homebrew/share/qemu/edk2-arm-vars.fd"

# Tunable defaults
RAM_MB=${RAM_MB:-4096}
CPUS=${CPUS:-4}
DISK_SIZE=${DISK_SIZE:-20G}

BG=false
CDROMS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --bg)     BG=true; shift ;;
        --cdrom)  CDROMS+=("$2"); shift 2 ;;
        *)        die "Unknown arg: $1" ;;
    esac
done

# --- Sudo check ---
[[ $EUID -eq 0 ]] || die "vmnet-shared requires root. Run with: sudo bash $0 $NAME ..."

INVOKING_USER=${SUDO_USER:-}
INVOKING_GRP=""
if [[ -n "$INVOKING_USER" ]]; then
    INVOKING_GRP=$(id -gn "$INVOKING_USER" 2>/dev/null || true)
fi

# --- Preflight ---
[[ -f "$EFI_CODE" ]] || die "UEFI firmware not found: $EFI_CODE (brew install qemu)"
command -v qemu-system-aarch64 >/dev/null || die "qemu-system-aarch64 not found (brew install qemu)"
command -v socat >/dev/null || die "socat not found (brew install socat)"

# --- Create VM artifacts on first run ---
mkdir -p "$VM_DIR"

if [[ ! -f "$DISK" ]]; then
    log "Creating disk image: $DISK ($DISK_SIZE thin-provisioned)"
    qemu-img create -f qcow2 "$DISK" "$DISK_SIZE"
fi

if [[ ! -f "$EFIVARS" ]]; then
    log "Copying UEFI vars template → $EFIVARS"
    cp "$EFI_VARS_TEMPLATE" "$EFIVARS"
fi

# --- Kill existing instance if running ---
if [[ -f "$PID_FILE" ]]; then
    OLD_PID=$(cat "$PID_FILE")
    if kill -0 "$OLD_PID" 2>/dev/null; then
        log "Stopping existing $NAME VM (PID $OLD_PID)..."
        echo "quit" | socat - "UNIX-CONNECT:$MONITOR_SOCK" 2>/dev/null || true
        sleep 2
        kill -0 "$OLD_PID" 2>/dev/null && kill "$OLD_PID" 2>/dev/null || true
        sleep 1
        kill -0 "$OLD_PID" 2>/dev/null && kill -9 "$OLD_PID" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
fi
rm -f "$MONITOR_SOCK" "$SERIAL_SOCK"

# --- Build QEMU args ---
QEMU_ARGS=(
    -accel hvf
    -machine virt
    -cpu host
    -smp "$CPUS"
    -m "$RAM_MB"

    # UEFI firmware
    -drive "if=pflash,format=raw,readonly=on,file=$EFI_CODE"
    -drive "if=pflash,format=raw,file=$EFIVARS"

    # Virtio disk
    -drive "if=virtio,file=$DISK,format=qcow2"

    # vmnet-shared networking (macOS vmnet.framework)
    -netdev vmnet-shared,id=net0
    -device "virtio-net-pci,netdev=net0,mac=$MAC"

    # QEMU monitor (for graceful shutdown, status queries)
    -monitor "unix:$MONITOR_SOCK,server,nowait"

    # No graphics
    -nographic
)

# Attach CD-ROMs if provided (for install phase)
for iso in "${CDROMS[@]}"; do
    [[ -f "$iso" ]] || die "CD-ROM ISO not found: $iso"
    QEMU_ARGS+=(-drive "if=virtio,media=cdrom,file=$iso,readonly=on")
done

# --- Launch ---
if $BG; then
    step "Booting $NAME VM in background..."
    log "  Disk:    $DISK"
    log "  MAC:     $MAC"
    log "  RAM:     ${RAM_MB}MB, CPUs: $CPUS"
    [[ ${#CDROMS[@]} -gt 0 ]] && log "  CD-ROMs: ${CDROMS[*]}"

    # Add serial socket for background mode
    QEMU_ARGS+=(-serial "unix:$SERIAL_SOCK,server,nowait")

    nohup qemu-system-aarch64 "${QEMU_ARGS[@]}" \
        > "$QEMU_LOG" 2>&1 &
    QEMU_PID=$!
    echo "$QEMU_PID" > "$PID_FILE"

    # Wait for sockets to appear
    local_wait=0
    while [[ $local_wait -lt 10 ]]; do
        [[ -S "$MONITOR_SOCK" ]] && break
        sleep 1
        local_wait=$((local_wait + 1))
    done

    # Make sockets accessible to invoking user
    if [[ -n "$INVOKING_USER" && -n "$INVOKING_GRP" ]]; then
        for sock in "$MONITOR_SOCK" "$SERIAL_SOCK" "$PID_FILE" "$QEMU_LOG"; do
            [[ -e "$sock" ]] && chown "$INVOKING_USER:$INVOKING_GRP" "$sock" 2>/dev/null || true
        done
        # Make VM dir accessible
        chown -R "$INVOKING_USER:$INVOKING_GRP" "$VM_DIR" 2>/dev/null || true
    fi

    if kill -0 "$QEMU_PID" 2>/dev/null; then
        log "QEMU started (PID $QEMU_PID)"
        log "  Monitor: socat -,rawer unix-connect:$MONITOR_SOCK"
        log "  Serial:  socat -,rawer unix-connect:$SERIAL_SOCK"
        log "  Log:     $QEMU_LOG"
    else
        die "QEMU failed to start. Check $QEMU_LOG"
    fi
else
    step "Booting $NAME VM in foreground (Ctrl-A X to quit)..."
    log "  Disk:    $DISK"
    log "  MAC:     $MAC"
    log "  RAM:     ${RAM_MB}MB, CPUs: $CPUS"
    [[ ${#CDROMS[@]} -gt 0 ]] && log "  CD-ROMs: ${CDROMS[*]}"
    echo ""

    # Foreground: serial on stdio, monitor multiplexed
    exec qemu-system-aarch64 "${QEMU_ARGS[@]}" -serial mon:stdio
fi
