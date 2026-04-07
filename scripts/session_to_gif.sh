#!/bin/bash
# FlowLink — Convert .cast to GIF using agg (asciinema)
# Usage: session_to_gif.sh input.cast [output.gif] [speed] [fps] [font-size] [theme]
set -euo pipefail

CAST="$1"
GIF="${2:-${CAST%.cast}.gif}"
SPEED="${3:-1.5}"
FPS="${4:-12}"
FONT_SIZE="${5:-14}"
THEME="${6:-flowlink-dark}"

# Map theme names to agg theme args
case "$THEME" in
  flowlink-dark)   AGG_THEME="--theme monokai" ;;
  monokai)         AGG_THEME="--theme monokai" ;;
  dracula)         AGG_THEME="--theme dracula" ;;
  nord)            AGG_THEME="--theme nord" ;;
  solarized-dark)  AGG_THEME="--theme solarized-dark" ;;
  tokyo-night)     AGG_THEME="--theme tokyo-night" ;;
  catppuccin-mocha) AGG_THEME="--theme catppuccin-mocha" ;;
  gruvbox)         AGG_THEME="--theme gruvbox" ;;
  one-dark)        AGG_THEME="--theme one-dark" ;;
  github-dark)     AGG_THEME="--theme github-dark" ;;
  *)               AGG_THEME="" ;;
esac

agg "$CAST" "$GIF" --speed "$SPEED" --fps "$FPS" --font-size "$FONT_SIZE" $AGG_THEME
