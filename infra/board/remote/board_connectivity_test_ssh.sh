#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  cat <<'USAGE'
Usage:
  bash infra/board/remote/board_connectivity_test_ssh.sh <ssh_target> [mount_hint] [tty_dev] [test_image] [confirm_image]

Examples:
  bash infra/board/remote/board_connectivity_test_ssh.sh root@192.168.0.106
  bash infra/board/remote/board_connectivity_test_ssh.sh root@192.168.0.106 /media/root/DAPLINK /dev/ttyACM0
  bash infra/board/remote/board_connectivity_test_ssh.sh root@192.168.0.106 /media/root/DAPLINK /dev/ttyACM0 /Users/jqwang/188-Gnucap/third_party/icesugar/demo/leds.bin
  bash infra/board/remote/board_connectivity_test_ssh.sh root@192.168.0.106 /media/root/DAPLINK /dev/ttyACM0 /Users/jqwang/188-Gnucap/third_party/icesugar/demo/leds.bin /Users/jqwang/199-rustplc/RustPLC/infra/board/icesugar-up5k/blink_green_10s/build/blink_green_10s.bin
USAGE
  exit 2
fi

SSH_TARGET="$1"
MOUNT_HINT="${2:-}"
TTY_DEV="${3:-/dev/ttyACM0}"
TEST_IMAGE="${4:-}"
CONFIRM_IMAGE="${5:-}"
REMOTE_IMAGE=""
REMOTE_IMAGE_NAME=""
REMOTE_CONFIRM_IMAGE=""
REMOTE_CONFIRM_IMAGE_NAME=""

if [[ -n "$TEST_IMAGE" ]]; then
  if [[ ! -f "$TEST_IMAGE" ]]; then
    echo "ERROR: test image not found: $TEST_IMAGE"
    exit 1
  fi
  REMOTE_IMAGE_NAME="$(basename "$TEST_IMAGE")"
  REMOTE_IMAGE="/tmp/board_conn_test_${REMOTE_IMAGE_NAME}"
  scp "$TEST_IMAGE" "$SSH_TARGET:$REMOTE_IMAGE" >/dev/null
fi

if [[ -n "$CONFIRM_IMAGE" ]]; then
  if [[ ! -f "$CONFIRM_IMAGE" ]]; then
    echo "ERROR: confirm image not found: $CONFIRM_IMAGE"
    exit 1
  fi
  REMOTE_CONFIRM_IMAGE_NAME="$(basename "$CONFIRM_IMAGE")"
  REMOTE_CONFIRM_IMAGE="/tmp/board_conn_confirm_${REMOTE_CONFIRM_IMAGE_NAME}"
  scp "$CONFIRM_IMAGE" "$SSH_TARGET:$REMOTE_CONFIRM_IMAGE" >/dev/null
fi

echo "== Board Connectivity Test (SSH) =="
echo "Target: $SSH_TARGET"
echo "Mount hint: ${MOUNT_HINT:-auto}"
echo "TTY: $TTY_DEV"
if [[ -n "$TEST_IMAGE" ]]; then
  echo "Programming test image: $TEST_IMAGE"
else
  echo "Programming test image: (none, marker file only)"
fi
if [[ -n "$CONFIRM_IMAGE" ]]; then
  echo "Programming confirm image: $CONFIRM_IMAGE"
fi
echo "Time: $(date '+%F %T %z')"
echo

ssh "$SSH_TARGET" "bash -s" -- "$MOUNT_HINT" "$TTY_DEV" "$REMOTE_IMAGE" "$REMOTE_IMAGE_NAME" "$REMOTE_CONFIRM_IMAGE" "$REMOTE_CONFIRM_IMAGE_NAME" <<'EOS'
set -euo pipefail

MOUNT_HINT="$1"
TTY_DEV="$2"
REMOTE_IMAGE="$3"
REMOTE_IMAGE_NAME="$4"
REMOTE_CONFIRM_IMAGE="$5"
REMOTE_CONFIRM_IMAGE_NAME="$6"

find_mount() {
  while IFS= read -r -d '' d; do
    echo "$d"
    return 0
  done < <(find /media /run/media -maxdepth 3 -type d \( -name DAPLINK -o -name iCELink -o -name ICELINK \) -print0 2>/dev/null)

  return 1
}

pass_usb=0
pass_serial=0
pass_mount=0
pass_program_cycle=0
pass_no_fail=0
pass_program_cycle_2=1
pass_no_fail_2=1

uid=""
board_type="unknown"
mount_dir=""
mount_dir_after=""
payload="marker"
payload_2="(none)"

if command -v lsusb >/dev/null 2>&1 && lsusb | grep -qi '0d28:0204'; then
  pass_usb=1
fi

if [[ -c "$TTY_DEV" ]]; then
  pass_serial=1
fi

if [[ -n "$MOUNT_HINT" ]]; then
  mount_dir="$MOUNT_HINT"
else
  mount_dir="$(find_mount || true)"
fi

if [[ -n "$mount_dir" && -d "$mount_dir" ]]; then
  pass_mount=1
fi

if [[ "$pass_mount" -eq 1 ]]; then
  if [[ -f "$mount_dir/DETAILS.TXT" ]]; then
    uid="$(awk -F': ' '/^Unique ID:/ {print $2}' "$mount_dir/DETAILS.TXT" | tr -d '\r' | head -1)"
  fi

  case "${uid:0:4}" in
    0700) board_type="iCESugar (UP5K)" ;;
    0710) board_type="iCESugar-Pro (ECP5)" ;;
    0720) board_type="iCESugar-Nano" ;;
    *) board_type="unknown" ;;
  esac

  run_program_cycle() {
    local in_mount="$1"
    local payload_path="$2"
    local payload_name="$3"

    local out_mount="$in_mount"
    local p_cycle=0
    local p_no_fail=0

    rm -f "$in_mount/FAIL.TXT" || true
    cp "$payload_path" "$in_mount/$payload_name"
    sync

    local dev_src=""
    dev_src="$(findmnt -nr -T "$in_mount" -o SOURCE 2>/dev/null | head -1 || true)"
    if [[ -n "$dev_src" ]]; then
      if command -v udisksctl >/dev/null 2>&1; then
        udisksctl unmount -b "$dev_src" >/dev/null 2>&1 || umount "$in_mount" >/dev/null 2>&1 || true
      else
        umount "$in_mount" >/dev/null 2>&1 || true
      fi
      sleep 4
      if command -v udisksctl >/dev/null 2>&1; then
        udisksctl mount -b "$dev_src" >/dev/null 2>&1 || true
      fi
      out_mount="$(findmnt -nr -S "$dev_src" -o TARGET 2>/dev/null | head -1 || true)"
    fi

    if [[ -z "$out_mount" ]]; then
      out_mount="$in_mount"
    fi

    if [[ -d "$out_mount" ]]; then
      p_cycle=1
    fi
    if [[ "$p_cycle" -eq 1 && ! -f "$out_mount/FAIL.TXT" ]]; then
      p_no_fail=1
    fi

    echo "$p_cycle:$p_no_fail:$out_mount"
  }

  if [[ -n "$REMOTE_IMAGE" && -f "$REMOTE_IMAGE" ]]; then
    payload="$REMOTE_IMAGE_NAME"
    cycle_result="$(run_program_cycle "$mount_dir" "$REMOTE_IMAGE" "$REMOTE_IMAGE_NAME")"
  else
    stamp="$(date +%s)"
    marker="conn_test_${stamp}.txt"
    marker_path="/tmp/${marker}"
    echo "board-connectivity-test ${stamp}" > "$marker_path"
    payload="$marker"
    cycle_result="$(run_program_cycle "$mount_dir" "$marker_path" "$marker")"
  fi

  pass_program_cycle="${cycle_result%%:*}"
  cycle_rest="${cycle_result#*:}"
  pass_no_fail="${cycle_rest%%:*}"
  mount_dir_after="${cycle_rest#*:}"

  if [[ -n "$REMOTE_CONFIRM_IMAGE" && -f "$REMOTE_CONFIRM_IMAGE" ]]; then
    payload_2="$REMOTE_CONFIRM_IMAGE_NAME"
    cycle_result_2="$(run_program_cycle "$mount_dir_after" "$REMOTE_CONFIRM_IMAGE" "$REMOTE_CONFIRM_IMAGE_NAME")"
    pass_program_cycle_2="${cycle_result_2%%:*}"
    cycle_rest_2="${cycle_result_2#*:}"
    pass_no_fail_2="${cycle_rest_2%%:*}"
    mount_dir_after="${cycle_rest_2#*:}"
  fi
fi

status() {
  if [[ "$1" -eq 1 ]]; then
    echo "PASS"
  else
    echo "FAIL"
  fi
}

echo "USB_DAPLINK=$(status "$pass_usb")"
echo "SERIAL_${TTY_DEV##*/}=$(status "$pass_serial")"
echo "MOUNT_DAPLINK=$(status "$pass_mount")"
echo "PROGRAM_CYCLE=$(status "$pass_program_cycle")"
echo "NO_FAIL_TXT=$(status "$pass_no_fail")"
echo "PAYLOAD=$payload"
if [[ "$payload_2" != "(none)" ]]; then
  echo "PROGRAM_CYCLE_2=$(status "$pass_program_cycle_2")"
  echo "NO_FAIL_TXT_2=$(status "$pass_no_fail_2")"
  echo "PAYLOAD_2=$payload_2"
fi
echo "BOARD_TYPE=$board_type"
if [[ -n "$uid" ]]; then
  echo "UNIQUE_ID=$uid"
fi
if [[ -n "$mount_dir_after" ]]; then
  echo "MOUNT_PATH=$mount_dir_after"
fi

overall=1
if [[ "$pass_usb" -ne 1 || "$pass_mount" -ne 1 || "$pass_program_cycle" -ne 1 || "$pass_no_fail" -ne 1 || "$pass_program_cycle_2" -ne 1 || "$pass_no_fail_2" -ne 1 ]]; then
  overall=0
fi

if [[ "$overall" -eq 1 ]]; then
  echo "RESULT=PASS (board link from host side is healthy)"
  exit 0
else
  echo "RESULT=FAIL (board link is incomplete)"
  exit 1
fi
EOS
