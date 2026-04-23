#!/usr/bin/env python3
"""Run graphify pipeline on current directory."""
import subprocess
import sys

# Build graph
result = subprocess.run(
    [sys.executable, "-m", "graphify", "."],
    capture_output=True,
    text=True
)

print(result.stdout)
if result.stderr:
    print("STDERR:", result.stderr, file=sys.stderr)

sys.exit(result.returncode)
