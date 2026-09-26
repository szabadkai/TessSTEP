#!/usr/bin/env python3
"""Generated, authored STEP corpus with independent graph and rejection oracles."""
import json
from pathlib import Path
import subprocess
import sys

from corpus import ROOT, atomic_write

HEADER = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('TessSTEP generated test'),'2;1');\nFILE_NAME('','',('test'),('test'),'','','');\nFILE_SCHEMA(('TESSSTEP_GRAPH'));\nENDSEC;\nDATA;\n"
FOOTER = "\nENDSEC;\nEND-ISO-10303-21;\n"


def fixtures():
    result = {}
    # Same directed cycle under ordering, ID relabeling, whitespace and string encoding.
    # Counts and reference closure follow from construction, not from parser output.
    for size in (1, 2, 7, 31, 127):
        for variant in ('plain', 'reversed', 'sparse', 'comments', 'crlf', 'unicode', 'sections'):
            ids = [10**12 + i * 7919 for i in range(size)] if variant == 'sparse' else list(range(1,size+1))
            label = r"'\X2\03A9\X0\'" if variant == 'unicode' else "'O''Brien #123 /* text */'"
            records = [f'#{ids[i]}=NODE({label},#{ids[(i+1)%size]},({i},1.25E+2,$,*));' for i in range(size)]
            if variant == 'reversed':
                records.reverse()
            if variant == 'comments':
                records = [r.replace('=', ' /* comment */ = ').replace(',', ',\n') for r in records]
            body = '\n'.join(records)
            if variant == 'sections':
                body = '\nENDSEC;\nDATA;\n'.join(records)
            text = HEADER + body + FOOTER
            if variant == 'crlf':
                text = text.replace('\n', '\r\n')
            result[f'graph/{size:03}-{variant}.step'] = (text.encode(), {'status':'clean','entity_count':size,'entity_counts':{'NODE':size},'missing_references':0,'diagnostic_codes':[]})
        # Several distinct dangling references; each use must remain visible.
        missing = '\n'.join(f'#{i+1}=NODE(#{size+i+1});' for i in range(size))
        result[f'graph/{size:03}-missing.step'] = ((HEADER+missing+FOOTER).encode(), {'status':'reference_errors','entity_count':size,'missing_references':size,'diagnostic_codes':['TS1103']})
    invalid = {
        'duplicate-id': ('#1=NODE();#1=NODE();','TS1101'),
        'zero-id': ('#0=NODE();','TS1010'),
        'id-overflow': ('#18446744073709551616=NODE();','TS1010'),
        'integer-overflow': ('#1=NODE(9223372036854775808);','TS1008'),
        'real-overflow': ('#1=NODE(1.E9999);','TS1008'),
        'real-underflow': ('#1=NODE(1.E-9999);','TS1008'),
        'bad-escape': (r"#1=NODE('\Q\');",'TS1006'),
        'surrogate': (r"#1=NODE('\X2\D800\X0\');",'TS1006'),
        'invalid-scalar': (r"#1=NODE('\X4\00110000\X0\');",'TS1006'),
        'binary-padding': ('#1=NODE("3F");','TS1009'),
        'duplicate-component': ('#1=(NODE() NODE());','TS1012'),
        'deep-aggregate': ('#1=NODE('+'('*128+'1'+')'*128+');','TS1201'),
        'deep-typed': ('#1=NODE('+'T('*128+'1'+')'*128+');','TS1201'),
        'scope': ('#1=&SCOPE','TS1301'),
    }
    for name,(body,code) in invalid.items():
        for style in ('lf','crlf'):
            text = HEADER+body+FOOTER
            if style=='crlf': text=text.replace('\n','\r\n')
            result[f'rejection/{name}-{style}.step'] = (text.encode(),{'status':'rejected','diagnostic_codes':[code]})
    # Byte-level corruption and trailing data are separate cases.
    for name,text in [('invalid-utf8', (HEADER+"#1=NODE('x');"+FOOTER).encode().replace(b"'x'",b"'\xff'")),
                      ('trailing-data',(HEADER+'#1=NODE();'+FOOTER+'GARBAGE').encode()),
                      ('missing-terminator',(HEADER+'#1=NODE();\nENDSEC;').encode())]:
        result[f'rejection/{name}.step']=(text,{'status':'rejected'})
    for size in (0, 1, 255, 4096, 65536):
        text=HEADER+"#1=NODE('"+'x'*size+"');"+FOOTER
        result[f'boundary/string-{size}.step']=(text.encode(),{'status':'clean','entity_count':1,'diagnostic_codes':[]})
    return result


def main():
    import tempfile
    with tempfile.TemporaryDirectory(prefix='tessstep-generated-') as directory:
        work=Path(directory)
        expected={}
        for name,(content,oracle) in fixtures().items():
            path=work/'inputs'/name
            path.parent.mkdir(parents=True,exist_ok=True)
            path.write_bytes(content)
            expected[name]=oracle
        atomic_write(work/'expected.json',json.dumps({'cases':expected},indent=2))
        subprocess.run([sys.executable,str(ROOT/'scripts/corpus.py'),'--corpus',str(work/'inputs'),
            '--output',str(ROOT/'reports/generated'),'--baseline',str(work/'no-baseline.json'),
            '--expectations',str(work/'expected.json'),'--repeat','2','--title','Authored STEP transformations and adversarial boundaries'],cwd=ROOT,check=True)
    print(f'Generated corpus: {len(expected)} reviewed file expectations, each parsed twice')


if __name__=='__main__': main()
