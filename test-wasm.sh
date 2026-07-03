#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

echo "Building wasm package..."
pnpm run build:wasm >/dev/null

if [[ $# -eq 0 ]]; then
  set -- --preset water-pressure
elif [[ $# -eq 1 && "$1" =~ ^[0-9]+$ ]]; then
  set -- --preset water-pressure --nx "$1"
fi

echo "Running canonical wasm FIM diagnostic..."
echo "Use --list to see presets or --help for switches."

node scripts/fim-wasm-diagnostic.mjs "$@"