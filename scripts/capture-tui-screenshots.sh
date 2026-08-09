#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_DIR="${1:-$ROOT/docs/screenshots}"
POSITION_X=40
POSITION_Y=40
WINDOW_WIDTH=1200
WINDOW_HEIGHT=800

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This capture script currently requires macOS." >&2
  exit 1
fi
for tool in wezterm screencapture osascript magick swift; do
  command -v "$tool" >/dev/null || {
    echo "Missing required tool: $tool" >&2
    exit 1
  }
done

if [[ "$(swift -e 'import CoreGraphics; print(CGPreflightScreenCaptureAccess())' 2>/dev/null)" != "true" ]]; then
  cat >&2 <<'MESSAGE'
Screen Recording permission is required.

Open System Settings → Privacy & Security → Screen & System Audio Recording,
enable the terminal application running this script, then restart that terminal.
MESSAGE
  exit 1
fi

mkdir -p "$OUTPUT_DIR"
CONFIG="$(mktemp -t kvui-wezterm).lua"
WINDOW_FINDER_SOURCE="$(mktemp -t kvui-window-finder).swift"
WINDOW_FINDER="$(mktemp -t kvui-window-finder-bin)"
trap 'rm -f "$CONFIG" "$WINDOW_FINDER_SOURCE" "$WINDOW_FINDER"' EXIT
cat >"$CONFIG" <<'LUA'
local wezterm = require 'wezterm'
return {
  initial_cols = 118,
  initial_rows = 41,
  font = wezterm.font_with_fallback({ 'JetBrains Mono', 'Menlo' }),
  font_size = 14.0,
  color_scheme = 'Builtin Solarized Dark',
  enable_tab_bar = false,
  window_decorations = 'NONE',
  window_padding = { left = 8, right = 8, top = 8, bottom = 8 },
  cursor_blink_rate = 0,
  animation_fps = 1,
}
LUA

cat >"$WINDOW_FINDER_SOURCE" <<'SWIFT'
import CoreGraphics
import Foundation

let targetPID = Int32(CommandLine.arguments[1])!
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as! [[String: Any]]
for window in windows {
    let ownerPID = window[kCGWindowOwnerPID as String] as? Int32
    let layer = window[kCGWindowLayer as String] as? Int
    let windowID = window[kCGWindowNumber as String] as? UInt32
    let bounds = window[kCGWindowBounds as String] as? [String: CGFloat]
    let width = bounds?["Width"] ?? 0
    if ownerPID == targetPID && layer == 0 && width > 200, let windowID {
        print(windowID)
        exit(0)
    }
}
exit(1)
SWIFT
swiftc "$WINDOW_FINDER_SOURCE" -o "$WINDOW_FINDER"

cd "$ROOT"
echo "Building kvui..."
cargo build -q -p kvui

capture() {
  local screen="$1"
  local output="$OUTPUT_DIR/kvui-$screen.png"

  wezterm --config-file "$CONFIG" start \
    --always-new-process \
    --position "$POSITION_X,$POSITION_Y" \
    --cwd "$ROOT" \
    -- "$ROOT/target/debug/kvui" --demo-screen "$screen" >/dev/null 2>&1 &
  local launcher_pid=$!

  local gui_pid=""
  for _ in {1..50}; do
    gui_pid="$(pgrep -n wezterm-gui || true)"
    [[ -n "$gui_pid" ]] && break
    sleep 0.1
  done
  if [[ -z "$gui_pid" ]]; then
    echo "Could not find the WezTerm GUI process." >&2
    kill "$launcher_pid" 2>/dev/null || true
    exit 1
  fi

  osascript <<APPLESCRIPT >/dev/null
    tell application "System Events"
      set targetProcess to first process whose unix id is $gui_pid
      tell targetProcess
        set frontmost to true
        repeat until (count of windows) > 0
          delay 0.1
        end repeat
        set position of window 1 to {$POSITION_X, $POSITION_Y}
        set size of window 1 to {$WINDOW_WIDTH, $WINDOW_HEIGHT}
      end tell
    end tell
APPLESCRIPT

  sleep 1
  local window_id
  window_id="$($WINDOW_FINDER "$gui_pid")"
  screencapture -x -l"$window_id" "$output"
  magick "$output" -strip "$output"
  echo "Captured $output"

  kill "$gui_pid" 2>/dev/null || true
  wait "$launcher_pid" 2>/dev/null || true
  sleep 0.3
}

for screen in menu songs song download complete; do
  capture "$screen"
done

echo "Screenshots written to $OUTPUT_DIR"
