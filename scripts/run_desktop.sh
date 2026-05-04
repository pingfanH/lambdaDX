#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
MANIFEST="$ROOT/demo/macroquad_sim/Cargo.toml"
ASSETS_DIR="$ROOT/demo/macroquad_sim/assets"
MP3="$ASSETS_DIR/demo.mp3"
WAV="$ASSETS_DIR/demo.wav"

if [[ ! -f "$WAV" && -f "$MP3" ]]; then
  if command -v ffmpeg >/dev/null 2>&1; then
    echo "[Mai2Chart] converting demo.mp3 -> demo.wav for speed-shift support..."
    ffmpeg -y -i "$MP3" -ac 2 -ar 44100 -sample_fmt s16 "$WAV" >/dev/null 2>&1
    echo "[Mai2Chart] created: $WAV"
  else
    echo "[Mai2Chart] ffmpeg not found. Audio speed-shift works best with assets/demo.wav"
  fi
fi

echo "[Mai2Chart] running desktop demo..."
cargo run --manifest-path "$MANIFEST"
