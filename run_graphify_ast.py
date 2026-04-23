#!/usr/bin/env python3
"""Run AST extraction only - no LLM needed."""
import subprocess
import sys
import os

# Get correct Python from graphify
result = subprocess.run(
    ["bash", "-lc", "which graphify"],
    capture_output=True,
    text=True
)
python_path = result.stdout.strip().split('\n')[0]

# Run AST extraction
os.chdir("/Users/braincoder/Projects/flowlink")
result = subprocess.run(
    [python_path, "-m", "graphify", "detect", "."],
    capture_output=True,
    text=True
)

print(result.stdout)
if result.stderr:
    print("STDERR:", result.stderr, file=sys.stderr)

sys.exit(result.returncode)
