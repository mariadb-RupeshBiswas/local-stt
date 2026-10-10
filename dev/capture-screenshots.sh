#!/bin/zsh
# Captures the demo's own windows by id for the README tour (macOS, needs Screen Recording for your terminal).
set -u
ROOT="${0:A:h:h}"
BIN="${1:-$ROOT/target/release/local-stt}"
OUT="$ROOT/docs/screenshots/raw"
TOOLS="$(mktemp -d)"
trap 'rm -rf "$TOOLS"' EXIT
[ -x "$BIN" ] || { echo "No local-stt binary at $BIN" >&2; exit 1; }
mkdir -p "$OUT"
rm -f "$OUT"/*.png(N)

# Lists one process's on-screen windows as "id layer width height title".
cat > "$TOOLS/winlist.swift" <<'SWIFT'
import CoreGraphics
let pid = Int(CommandLine.arguments[1]) ?? -1
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
for w in list where (w[kCGWindowOwnerPID as String] as? Int) == pid {
    let id = w[kCGWindowNumber as String] as? Int ?? 0
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    let name = w[kCGWindowName as String] as? String ?? ""
    if let b = w[kCGWindowBounds as String] as? [String: Double] {
        print(id, layer, Int(b["Width"] ?? 0), Int(b["Height"] ?? 0), name)
    }
}
SWIFT
swiftc -O "$TOOLS/winlist.swift" -o "$TOOLS/winlist" || exit 1

PID=""
# Finds the demo's window with this title (set in src/app.rs); layer 25 is a menu bar item.
window_id() {
  "$TOOLS/winlist" "$PID" | awk -v want="$1" '$2 != 25 { id = $1; $1 = $2 = $3 = $4 = ""; sub(/^ +/, ""); if ($0 == want) { print id; exit } }'
}

# One window by id, never a screen region, so nothing behind or beside it lands in the image.
shoot() {
  local id; id="$(window_id "$2")"
  [ -n "$id" ] || return 1
  if [ "${3:-}" = "flat" ]; then screencapture -x -o -l "$id" "$OUT/$1.png"; else screencapture -x -l "$id" "$OUT/$1.png"; fi
}

capture() {
  case "$1" in
    history|model|settings) shoot "$1" "local-stt" ;;
    recording) shoot recording "local-stt recording" flat && shoot recording-live "local-stt live text" flat ;;
    hands-free|transcribing|done) shoot "$1" "local-stt recording" flat ;;
  esac
}

"$BIN" demo | while read -r tag step arg; do
  [ "$tag" = "DEMO" ] || continue
  case "$step" in
    pid) PID="$arg" ;;
    end) break ;;
    *) if capture "$step"; then echo "captured $step"; else echo "missed $step" >&2; fi ;;
  esac
done

missing=()
for name in history model settings recording recording-live hands-free transcribing done; do
  [ -s "$OUT/$name.png" ] || missing+=("$name")
done
if (( ${#missing} )); then
  echo "Missing captures: ${missing[*]}" >&2
  exit 1
fi
echo "Raw captures in $OUT (gitignored). Next: node dev/annotate/annotate.mjs, then review docs/screenshots/review/."
