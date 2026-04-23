#!/usr/bin/env python3
"""Check graphify installation."""
import subprocess
import sys

# Find correct Python interpreter
try:
    result = subprocess.run(
        ["graphify", "--version"],
        capture_output=True,
        text=True
    )
    print(f"Graphify version: {result.stdout.strip()}")
    print(f"Graphify executable: {subprocess.check_output(['which', 'graphify']).decode().strip()}")
except Exception as e:
    print(f"Error: {e}")
