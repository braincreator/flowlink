import sys
import os

# Add graphify to path
sys.path.insert(0, '/Users/braincoder/.local/share/uv/python/cpython-3.13.11-macos-aarch64-none/lib/python3.13/site-packages')

from graphify.detect import detect
from graphify.extract import collect_files, extract
from pathlib import Path
import json

os.chdir('/Users/braincoder/Projects/flowlink')

print("Detecting files...")
result = detect(Path('.'))
print(json.dumps(result, indent=2))

print("\n" + "="*80 + "\n")

if result.get('files', {}).get('code'):
    code_files = []
    for f in result['files']['code']:
        code_files.extend(collect_files(Path(f)) if Path(f).is_dir() else [Path(f)])

    if code_files:
        print(f"Extracting {len(code_files)} code files...")
        ast_result = extract(code_files, cache_root=Path('.'))
        print(json.dumps(ast_result, indent=2))
    else:
        print("No code files found")
