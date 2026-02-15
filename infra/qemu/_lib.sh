#!/usr/bin/env bash
# Shared helper functions for QEMU VM scripts.
# Source this file: source "$(dirname "$0")/_lib.sh"

log()  { echo "[$(date +%H:%M:%S)] $*"; }
step() { echo ""; echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"; echo "[$(date +%H:%M:%S)] >> $*"; echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"; }
die()  { log "ERROR: $*" >&2; exit 1; }

# MAC address map
vm_mac() {
    case "$1" in
        ubuntu) echo "52:54:00:12:34:01" ;;
        *)      die "Unknown VM '$1' (use: ubuntu)" ;;
    esac
}

# Find guest IP by MAC address in ARP table.
# Usage: find_guest_ip "52:54:00:12:34:01"
# Returns IP on stdout, or empty string if not found.
find_guest_ip() {
    local mac_addr="$1"

    # Get vmnet bridge subnet prefix
    local subnet_prefix
    subnet_prefix=$(/sbin/ifconfig bridge100 2>/dev/null \
        | awk '/inet[[:space:]]/ {print $2; exit}' \
        | sed 's/\.[0-9]*$/./') || true
    [[ -n "$subnet_prefix" ]] || return 1

    # Ping-scan to populate ARP cache
    for i in $(seq 2 20); do
        /sbin/ping -c 1 -W 100 "${subnet_prefix}${i}" >/dev/null 2>&1 &
    done
    wait 2>/dev/null

    # Normalize target MAC to lowercase zero-padded
    local target
    target=$(echo "$mac_addr" | awk -F: '{
        for (i=1;i<=NF;i++) {
            oct=tolower($i);
            if (length(oct)==1) oct="0"oct;
            out=(i==1 ? oct : out":"oct);
        }
        print out;
    }')

    # Search ARP table for matching MAC
    /usr/sbin/arp -a 2>/dev/null | awk -v target="$target" '{
        ip=$2; gsub(/[()]/,"",ip);
        mac=$4; n=split(mac,a,":");
        if (n < 6) next;
        norm="";
        for(i=1;i<=n;i++){
            oct=tolower(a[i]);
            if(length(oct)==1) oct="0"oct;
            norm=(i==1 ? oct : norm":"oct);
        }
        if(norm==target){ print ip; exit 0; }
    }'
}

# Check SSH banner on an IP.
# Usage: check_ssh "192.168.2.3"
check_ssh() {
    local ip="$1"
    local banner
    banner=$(printf '\n' | nc -G 2 -w 2 "$ip" 22 2>/dev/null | head -1 || true)
    [[ "$banner" == *"SSH"* ]]
}

# Wait for a VM's IP to appear and SSH to become available.
# Usage: wait_for_ip_and_ssh "52:54:00:12:34:01" [timeout_seconds]
# Prints the discovered IP on stdout.
wait_for_ip_and_ssh() {
    local mac="$1"
    local timeout="${2:-1800}"
    local elapsed=0
    local ip=""

    log "Waiting for VM (MAC $mac) to get IP and SSH... (timeout ${timeout}s)" >&2
    while [[ $elapsed -lt $timeout ]]; do
        ip=$(find_guest_ip "$mac" 2>/dev/null || true)
        if [[ -n "$ip" ]]; then
            if check_ssh "$ip"; then
                log "SSH ready at $ip (after ${elapsed}s)" >&2
                echo "$ip"
                return 0
            fi
            log "  IP $ip found but SSH not ready yet..." >&2
        fi
        sleep 10
        elapsed=$((elapsed + 10))
        log "  Waiting... $((elapsed/60))m$((elapsed%60))s / $((timeout/60))m" >&2
    done
    log "Timeout waiting for SSH (MAC $mac)" >&2
    return 1
}

# Write VM connection info to env file.
# Usage: write_info_env <vm_name> <ip> <user> <password>
write_info_env() {
    local name="$1" ip="$2" user="$3" password="$4"
    local script_dir
    script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
    local info_file="$script_dir/vm/$name/info.env"

    cat > "$info_file" <<EOF
VM_NAME=$name
VM_IP=$ip
VM_USER=$user
VM_PASSWORD=$password
VM_MAC=$(vm_mac "$name")
QEMU_PID=$(cat "$script_dir/vm/$name/qemu.pid" 2>/dev/null || echo "unknown")
MONITOR_SOCK=$script_dir/vm/$name/monitor.sock
SERIAL_SOCK=$script_dir/vm/$name/serial.sock
EOF
    log "Connection info written to $info_file"
}
