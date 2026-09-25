#!/usr/bin/env python3
"""Validate CLI JSON with an independent standard-library decoder."""
import json
from pathlib import Path
import subprocess
import sys
ROOT = Path(__file__).resolve().parents[1]
exe = ROOT / "target" / "debug" / ("stepdump.exe" if sys.platform == "win32" else "stepdump")
for path in sorted((ROOT / "corpus" / "part21").glob("*/*.step")):
    result = subprocess.run([str(exe), "--json", str(path)], capture_output=True, check=False)
    payload = json.loads(result.stdout)
    assert payload["format_version"] == 1
    has_errors = any(d["severity"] == "error" for d in payload["diagnostics"])
    assert result.returncode == int(has_errors), path
    again = subprocess.run([str(exe), "--json", str(path)], capture_output=True, check=False)
    assert result.stdout == again.stdout, path
print("CLI JSON schema, exit codes, and byte determinism: all corpus files passed")
source = (ROOT / 'corpus/part21/valid/empty.step').read_text(encoding="utf-8")
source = source.replace("FILE_DESCRIPTION(('TessSTEP authored fixture')", "FILE_DESCRIPTION(('quote\" and \\X\\00')")
source = source.replace('ENDSEC;\nDATA;', "EXTRA('first');EXTRA('second');ENDSEC;\nDATA;")
result = subprocess.run([str(exe), '--json', '-'], input=source.encode(), capture_output=True, check=True)
payload = json.loads(result.stdout)
headers = payload['document']['headers']
assert headers[0]['parameters'][0][0] == 'quote" and \0'
assert [h['parameters'] for h in headers if h['name'] == 'EXTRA'] == [['first'], ['second']]
print('CLI JSON escaping and repeated extension headers: passed')
