#!/usr/bin/env bash
set -euo pipefail

cd /home/reken/Repos/ressim

echo "Building wasm package..."
npm run build:wasm >/dev/null

if [[ $# -eq 0 ]]; then
  set -- --preset water-pressure
elif [[ $# -eq 1 && "$1" =~ ^[0-9]+$ ]]; then
  set -- --preset water-pressure --nx "$1"
fi

echo "Running canonical wasm FIM diagnostic..."
echo "Use --list to see presets or --help for switches."

node scripts/fim-wasm-diagnostic.mjs "$@"