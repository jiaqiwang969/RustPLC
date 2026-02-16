#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  cat <<'USAGE'
Usage:
  bash infra/board/remote/flash_daplink_ssh.sh <local_image> <ssh_target> [remote_mount] [remote_name] [wait_seconds]

Examples:
  bash infra/board/remote/flash_daplink_ssh.sh \
    /Users/jqwang/188-Gnucap/third_party/icesugar-pro/demo/blink_red.bit \
    root@192.168.0.106

  bash infra/board/remote/flash_daplink_ssh.sh \
    /tmp/leds.bin \
    root@192.168.0.106 \
    /media/wjq/DAPLINK \
    leds.bin \
    2
USAGE
  exit 2
fi

LOCAL_IMAGE="$1"
SSH_TARGET="$2"
REMOTE_MOUNT="${3:-}"
REMOTE_NAME="${4:-$(basename "$LOCAL_IMAGE")}" 
WAIT_SECONDS="${5:-6}"
REMOTE_MOUNT_ARG="${REMOTE_MOUNT:-__AUTO__}"

if [[ ! -f "$LOCAL_IMAGE" ]]; then
  echo "Image file not found: $LOCAL_IMAGE"
  exit 1
fi

REMOTE_TMP="/tmp/${REMOTE_NAME}"

echo "== DAPLink Drag-Drop (SSH) =="
echo "Image:  $LOCAL_IMAGE"
echo "Target: $SSH_TARGET"
echo "Name:   $REMOTE_NAME"
if [[ -n "$REMOTE_MOUNT" ]]; then
  echo "Mount:  $REMOTE_MOUNT (user supplied)"
else
  echo "Mount:  auto-detect"
fi
echo

scp "$LOCAL_IMAGE" "$SSH_TARGET:$REMOTE_TMP"

ssh "$SSH_TARGET" "bash -s" -- "$REMOTE_TMP" "$REMOTE_MOUNT_ARG" "$REMOTE_NAME" "$WAIT_SECONDS" <<'EOS'
set -euo pipefail

REMOTE_TMP="$1"
MOUNT_HINT="$2"
REMOTE_NAME="$3"
WAIT_SECONDS="$4"

if [[ "$MOUNT_HINT" == "__AUTO__" ]]; then
  MOUNT_HINT=""
fi

find_mount() {
  # Common Linux auto-mount paths for DAPLink/iCELink virtual disk.
  while IFS= read -r -d '' d; do
    echo "$d"
    return 0
  done < <(find /media /run/media -maxdepth 3 -type d \( -name DAPLINK -o -name iCELink -o -name ICELINK \) -print0 2>/dev/null)

  return 1
}

if [[ -n "$MOUNT_HINT" ]]; then
  MNT="$MOUNT_HINT"
else
  MNT="$(find_mount || true)"
fi

if [[ -z "$MNT" || ! -d "$MNT" ]]; then
  echo "ERROR: DAPLink mount not found."
  echo "Hint: check lsblk/mount and pass [remote_mount] explicitly."
  exit 1
fi

echo "Resolved mount: $MNT"
if [[ -f "$MNT/DETAILS.TXT" ]]; then
  echo "--- DETAILS.TXT (key lines) ---"
  grep -E 'Firmware|Build Time|Unique ID|Interface Version|URL' "$MNT/DETAILS.TXT" || true
fi
echo

rm -f "$MNT/FAIL.TXT" || true
cp "$REMOTE_TMP" "$MNT/$REMOTE_NAME"
sync

# DAPLink on Linux can report transient timeout when the host keeps the MSC
# mounted during programming. Best-effort: unmount, wait, then remount.
DEV_SRC="$(findmnt -nr -T "$MNT" -o SOURCE 2>/dev/null | head -1 || true)"
if [[ -n "$DEV_SRC" ]]; then
  if command -v udisksctl >/dev/null 2>&1; then
    udisksctl unmount -b "$DEV_SRC" >/dev/null 2>&1 || umount "$MNT" >/dev/null 2>&1 || true
  else
    umount "$MNT" >/dev/null 2>&1 || true
  fi
fi

sleep "$WAIT_SECONDS"

if [[ -n "$DEV_SRC" ]] && command -v udisksctl >/dev/null 2>&1; then
  udisksctl mount -b "$DEV_SRC" >/dev/null 2>&1 || true
fi

MNT2="$MNT"
if [[ -n "$DEV_SRC" ]]; then
  MNT_CAND="$(findmnt -nr -S "$DEV_SRC" -o TARGET 2>/dev/null | head -1 || true)"
  if [[ -n "$MNT_CAND" ]]; then
    MNT2="$MNT_CAND"
  fi
fi

if [[ ! -d "$MNT2" ]]; then
  echo "ERROR: DAPLink mount not available after programming cycle."
  exit 1
fi

echo "--- Mount listing ---"
ls -la "$MNT2" | sed -n '1,60p'

echo
if [[ -f "$MNT2/FAIL.TXT" ]]; then
  echo "RESULT: FAIL.TXT detected"
  sed -n '1,120p' "$MNT2/FAIL.TXT" || true
  exit 1
fi

echo "RESULT: programming cycle finished, FAIL.TXT not present."
EOS
