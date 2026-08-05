#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-run}"
APP_NAME="Empathy"
PROCESS_NAME="empathy"
BUNDLE_ID="ai.empathy.desktop"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_DIR="$ROOT_DIR/frontend"
APP_BUNDLE="$ROOT_DIR/target/debug/bundle/macos/$APP_NAME.app"
APP_BINARY="$APP_BUNDLE/Contents/MacOS/$PROCESS_NAME"

pkill -x "$PROCESS_NAME" >/dev/null 2>&1 || true

cd "$FRONTEND_DIR"
# Local debug bundles must not require the private updater signing key used only
# by the reviewed release workflow.
pnpm exec tauri build --debug --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
test -x "$APP_BINARY"

open_app() {
  /usr/bin/open -n "$APP_BUNDLE"
}

case "$MODE" in
  run)
    open_app
    ;;
  --debug|debug)
    lldb -- "$APP_BINARY"
    ;;
  --logs|logs)
    open_app
    /usr/bin/log stream --info --style compact --predicate "process == \"$PROCESS_NAME\""
    ;;
  --telemetry|telemetry)
    open_app
    /usr/bin/log stream --info --style compact --predicate "subsystem == \"$BUNDLE_ID\""
    ;;
  --verify|verify)
    open_app
    sleep 2
    pgrep -x "$PROCESS_NAME" >/dev/null
    ;;
  *)
    echo "usage: $0 [run|--debug|--logs|--telemetry|--verify]" >&2
    exit 2
    ;;
esac
