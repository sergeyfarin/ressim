#!/usr/bin/env bash
set -euo pipefail

# Use the same layout for Vite preview and the browser suite. This script never deploys.
preview_port=${RESIM_PREVIEW_PORT:-4173}
export RESIM_PUBLIC_URL="http://127.0.0.1:${preview_port}${RESIM_BASE_PATH:-/ressim/}"
preview_log=$(mktemp)
node node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port "$preview_port" --strictPort > "$preview_log" 2>&1 &
preview_pid=$!
trap 'kill "$preview_pid" 2>/dev/null || true; rm -f "$preview_log"' EXIT

up=0
for _ in $(seq 1 60); do
  if ! kill -0 "$preview_pid" 2>/dev/null; then break; fi
  # Wait for our server to bind before probing. An existing listener must never satisfy
  # readiness while the new process is still starting (or about to fail --strictPort).
  if [[ $(<"$preview_log") == *"http://127.0.0.1:${preview_port}/"* ]] &&
    curl -sf "$RESIM_PUBLIC_URL" > /dev/null; then up=1; break; fi
  sleep 1
done
if [ "$up" != 1 ]; then
  echo "Preview did not answer at $RESIM_PUBLIC_URL within 60 s" >&2
  cat "$preview_log"
  exit 1
fi

if pnpm run test:deployed; then
  exit 0
else
  status=$?
  cat "$preview_log"
  exit "$status"
fi
