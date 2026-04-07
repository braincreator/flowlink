#!/bin/bash
# Convert asciinema session to GIF for Telegram
# Usage: session_to_gif.sh input.cast output.gif [speed]
CAST="$1"
GIF="$2"
SPEED="${3:-1.5}"

/Users/braincoder/Projects/flowlink/target/release/flowlink-cast2gif \
  --input "$CAST" \
  --output "$GIF" \
  --speed "$SPEED" \
  --fps 12 \
  --bg "#0a0e1a" \
  --fg "#e1e4ed"
