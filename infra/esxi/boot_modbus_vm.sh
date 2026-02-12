#!/usr/bin/env bash
set -euo pipefail

# Create and boot a lightweight Linux VM inside ESXi (via QEMU nested virt)
# for running the Modbus TCP slave. This is the Mode B "virtual factory" setup.
#
# Prerequisites:
#   - ESXi 8 ARM already installed in QCOW2 (196-ESXI-ARM project)
#   - Alpine Linux ISO downloaded (or Ubuntu cloud image)
#   - QEMU 10+ with aarch64 support
#
# Architecture:
#   Host (macOS) → QEMU → ESXi 8 ARM → Linux VM (Modbus slave)
#                                      ↕ Modbus TCP (port 502)
#                         RustPLC runtime (Modbus master)
#
# For local testing without ESXi, this script can also boot the Linux VM
# directly under QEMU with --direct flag.

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
INFRA_DIR=$(cd "$SCRIPT_DIR/.." && pwd)
PROJECT_ROOT=$(cd "$INFRA_DIR/.." && pwd)
ESXI_PROJECT="/Users/jqwang/196-ESXI-ARM/work"

# Defaults
MODE="direct"  # direct | esxi
ALPINE_ISO="${ALPINE_ISO:-}"
RAM_MB=512
CPUS=2
MODBUS_PORT=5502  # host-side forwarded port (avoid privileged 502)
SSH_PORT=2222

usage() {
    cat <<USAGE
Usage: $0 [OPTIONS]

Options:
  --direct          Boot Linux VM directly under QEMU (default)
  --esxi            Boot via ESXi nested virtualization
  --iso PATH        Path to Alpine Linux ISO
  --ram MB          RAM in MB (default: 512)
  --cpus N          Number of CPUs (default: 2)
  --modbus-port P   Host port forwarded to VM port 502 (default: 5502)
  --ssh-port P      Host port forwarded to VM port 22 (default: 2222)
  -h, --help        Show this help
USAGE
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --direct) MODE="direct"; shift ;;
        --esxi) MODE="esxi"; shift ;;
        --iso) ALPINE_ISO="$2"; shift 2 ;;
        --ram) RAM_MB="$2"; shift 2 ;;
        --cpus) CPUS="$2"; shift 2 ;;
        --modbus-port) MODBUS_PORT="$2"; shift 2 ;;
        --ssh-port) SSH_PORT="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

VM_DIR="$INFRA_DIR/esxi/vm"
mkdir -p "$VM_DIR"

DISK_IMG="$VM_DIR/modbus-slave.qcow2"

# Create disk if not exists
if [[ ! -f "$DISK_IMG" ]]; then
    echo "Creating VM disk: $DISK_IMG (2G)"
    qemu-img create -f qcow2 "$DISK_IMG" 2G
fi

if [[ "$MODE" == "direct" ]]; then
    echo "=== Mode B (direct): Linux VM with Modbus slave ==="
    echo "  Disk:        $DISK_IMG"
    echo "  RAM/CPUs:    ${RAM_MB}MB / $CPUS"
    echo "  Modbus TCP:  localhost:$MODBUS_PORT → VM:502"
    echo "  SSH:         localhost:$SSH_PORT → VM:22"
    echo ""

    CDROM_ARGS=()
    if [[ -n "$ALPINE_ISO" && -f "$ALPINE_ISO" ]]; then
        echo "  ISO:         $ALPINE_ISO"
        CDROM_ARGS=(-cdrom "$ALPINE_ISO" -boot d)
    else
        echo "  (no ISO — booting from disk)"
    fi

    echo ""
    echo "After Alpine install, run inside VM:"
    echo "  apk add python3 py3-pip"
    echo "  pip install pymodbus"
    echo "  python3 /mnt/modbus_slave.py --port 502"
    echo ""

    exec qemu-system-aarch64 \
        -accel tcg \
        -machine virt \
        -cpu max \
        -smp "$CPUS" \
        -m "$RAM_MB" \
        -drive if=virtio,file="$DISK_IMG",format=qcow2 \
        "${CDROM_ARGS[@]}" \
        -netdev user,id=net0,hostfwd=tcp::"$MODBUS_PORT"-:502,hostfwd=tcp::"$SSH_PORT"-:22 \
        -device virtio-net-pci,netdev=net0 \
        -nographic

elif [[ "$MODE" == "esxi" ]]; then
    echo "=== Mode B (ESXi): nested Linux VM ==="
    echo ""
    echo "This mode requires ESXi to be running. Steps:"
    echo "  1. Start ESXi:  cd $ESXI_PROJECT && bash scripts/run_esxi8_boot_installed.sh"
    echo "  2. SSH into ESXi and create a Linux VM via vSphere/govc"
    echo "  3. Deploy modbus_slave.py into the Linux VM"
    echo ""
    echo "For automated provisioning, use govc:"
    echo "  export GOVC_URL=https://<esxi-ip>/sdk"
    echo "  export GOVC_USERNAME=root"
    echo "  export GOVC_PASSWORD=<password>"
    echo "  export GOVC_INSECURE=true"
    echo "  govc vm.create -m $RAM_MB -c $CPUS -net='VM Network' -disk=2G modbus-slave"
    echo ""
    echo "Or use the direct mode for local testing: $0 --direct"
    exit 0
fi
