#!/bin/zsh
# Captures the real app during `local-stt demo` for the README tour (macOS, needs Screen Recording for your terminal).
set -u
ROOT="${0:A:h:h}"
BIN="${1:-$ROOT/target/release/local-stt}"
OUT="$ROOT/docs/screenshots/raw"
TOOLS="$(mktemp -d)"
mkdir -p "$OUT"

# Lists this app's on-screen windows as "id x y w h" (owner names need no permission).
cat > "$TOOLS/winlist.swift" <<'SWIFT'
import CoreGraphics
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
for w in list where (w[kCGWindowOwnerName as String] as? String) == "local-stt" {
    let id = w[kCGWindowNumber as String] as? Int ?? 0
    if let b = w[kCGWindowBounds as String] as? [String: Double] {
        print(id, Int(b["X"] ?? 0), Int(b["Y"] ?? 0), Int(b["Width"] ?? 0), Int(b["Height"] ?? 0))
    }
}
SWIFT
swiftc -O "$TOOLS/winlist.swift" -o "$TOOLS/winlist" || exit 1

main_window() { "$TOOLS/winlist" | awk '$4 > 500 { print $1; exit }'; }

# Region around the pill and the live popover, so the glass shows what is behind it.
pill_region() {
  "$TOOLS/winlist" | awk '$4 <= 500 {
    if (n == 0 || $2 < x0) x0 = $2; if (n == 0 || $3 < y0) y0 = $3;
    if (n == 0 || $2 + $4 > x1) x1 = $2 + $4; if (n == 0 || $3 + $5 > y1) y1 = $3 + $5; n++ }
    END { if (n) printf "%d,%d,%d,%d", x0 - 40, y0 - 40, x1 - x0 + 80, y1 - y0 + 80 }'
}

capture() {
  local name="$1"
  case "$name" in
    history|model|settings)
      local id; id="$(main_window)"
      [ -n "$id" ] && screencapture -x -l "$id" "$OUT/$name.png" && echo "captured $name" ;;
    recording|hands-free|transcribing|done)
      local region; region="$(pill_region)"
      [ -n "$region" ] && screencapture -x -R "$region" "$OUT/$name.png" && echo "captured $name" ;;
  esac
}

"$BIN" demo | while read -r tag step; do
  [ "$tag" = "DEMO" ] || continue
  [ "$step" = "end" ] && break
  capture "$step"
done
rm -rf "$TOOLS"
echo "Raw screenshots in $OUT. Check every image for anything personal before committing."
