#!/usr/bin/env python3
"""Run workspace Rust tests and retain per-test evidence separately from STEP inputs."""
import html
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]


def parse_tests(log):
    group = 'workspace'
    tests = []
    for line in log.splitlines():
        line = re.sub(r'\x1b\[[0-9;]*m', '', line).strip()
        if line.startswith('Running ') and '(' in line:
            group = line.rsplit('(',1)[1].rstrip(')').replace('\\','/').split('/')[-1]
            group = re.sub(r'-[0-9a-f]{16}(?:\.exe)?$', '', group)
        elif line.startswith('Doc-tests '):
            group = line.removeprefix('Doc-tests ') + ' doctests'
        match = re.fullmatch(r'test (.+) \.\.\. (ok|FAILED|ignored)(?:, .*)?', line)
        if match:
            tests.append({'suite':group,'name':match[1],'status':{'ok':'passed','FAILED':'failed','ignored':'skipped'}[match[2]]})
    return tests


def main():
    directory=ROOT/'reports/kernel'
    directory.mkdir(parents=True,exist_ok=True)
    started=time.monotonic()
    command=['cargo','test','--workspace','--locked','--no-fail-fast','--','--test-threads=1']
    log_path=directory/'tests.log'
    with log_path.open('w',encoding='utf-8') as out:
        try:
            result=subprocess.run(command,cwd=ROOT,stdout=out,stderr=subprocess.STDOUT,timeout=900,
                                  env={**os.environ,'CARGO_TERM_COLOR':'never'})
            code=result.returncode
        except subprocess.TimeoutExpired:
            code=124
    log=log_path.read_text(encoding='utf-8',errors='replace')
    print(log)
    tests=parse_tests(log)
    if not tests or (code and not any(t['status']=='failed' for t in tests)):
        tests.append({'suite':'workspace','name':'Build and test process','status':'failed'})
    summary={s:sum(t['status']==s for t in tests) for s in ('passed','failed','skipped')}
    report={'format_version':1,'scope':'constructed models and authored Rust tests; not external STEP geometry',
            'command':command,'exit_code':code,'seconds':round(time.monotonic()-started,3),'summary':summary,'tests':tests}
    (directory/'latest.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    suite=ET.Element('testsuite',name='Rust workspace',tests=str(len(tests)),failures=str(summary['failed']),skipped=str(summary['skipped']),time=str(report['seconds']))
    for t in tests:
        item=ET.SubElement(suite,'testcase',classname=t['suite'],name=t['name'])
        if t['status']=='failed': ET.SubElement(item,'failure',message='See tests.log for failure details')
        if t['status']=='skipped': ET.SubElement(item,'skipped')
    (directory/'junit.xml').write_text(ET.tostring(suite,encoding='unicode',xml_declaration=True),encoding='utf-8')
    groups=sorted({t['suite'] for t in tests})
    lines=['## Rust kernel test evidence','',f"**{summary['passed']} passed, {summary['failed']} failed, {summary['skipped']} skipped** in {report['seconds']:.1f}s.",'',
           'Constructed geometry/mesh tests and authored integration tests are separate from external STEP compatibility.','',
           '| Test executable | Passed | Failed | Skipped |','| --- | ---: | ---: | ---: |']
    for group in groups:
        counts=[sum(t['suite']==group and t['status']==s for t in tests) for s in ('passed','failed','skipped')]
        lines.append(f'| {group} | '+ ' | '.join(map(str,counts))+' |')
    markdown='\n'.join(lines)+'\n'
    (directory/'summary.md').write_text(markdown,encoding='utf-8')
    rows=''.join(f'<tr><td>{html.escape(t["suite"])}</td><td>{html.escape(t["name"])}</td><td>{t["status"]}</td></tr>' for t in tests)
    (directory/'index.html').write_text('<!doctype html><html lang="en"><meta charset="utf-8"><title>TessSTEP kernel test evidence</title><style>body{font:15px system-ui;margin:32px;color:#152435}td,th{padding:8px;border-bottom:1px solid #ddd;text-align:left}table{border-collapse:collapse}</style><h1>Constructed-model and Rust integration tests</h1><p>These tests exercise APIs and authored cases; they do not establish external STEP geometry support.</p><p><a href="tests.log">Complete log</a> · <a href="junit.xml">JUnit</a> · <a href="latest.json">JSON</a></p><pre>'+html.escape(str(summary))+'</pre><table><tr><th>Executable</th><th>Test</th><th>Verdict</th></tr>'+rows+'</table></html>',encoding='utf-8')
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'],'a',encoding='utf-8') as stream:stream.write(markdown)
    return int(code != 0 or summary['failed'] != 0)


if __name__=='__main__': sys.exit(main())
