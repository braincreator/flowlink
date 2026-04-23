#!/usr/bin/env python3
"""Detect files for graphify - no LLM."""
import sys
import os

# Add graphifyy to path
sys.path.insert(0, os.path.expanduser("~/.local/share/uv/python/cpython-3.13.11-macos-aarch64-none/lib/python3.13/site-packages"))

from graphify.detect import detect
from pathlib import Path

os.chdir("/Users/braincoder/Projects/flowlink")
result = detect(Path("."))

import json
print(json.dumps(result, indent=2))
