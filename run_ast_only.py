#!/usr/bin/env python3
"""AST extraction only - no LLM, no API calls."""
import sys
import os

# Find graphify python
try:
    result = os.popen("which graphify").read().strip()
    if result:
        result = os.popen(f"{result} python -c 'import sys; print(sys.executable)'").read().strip()
except:
    result = None

if not result:
    result = "/usr/local/bin/python3"

# Build AST extraction
cmd = f'''{result} -c "
import sys
sys.path.insert(0, '/Users/braincoder/.local/share/uv/python/cpython-3.13.11-macos-aarch64-none/lib/python3.13/site-packages')
from graphify.detect import detect
from graphify.extract import collect_files, extract
from pathlib import Path
import json

os.chdir('/Users/braincoder/Projects/flowlink')
result = detect(Path('.'))
print(json.dumps(result, indent=2))
"'''

print(f"Running: {cmd[:80]}...")
print("="*80)

result = os.system(cmd)
