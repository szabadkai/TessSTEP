import copy
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

import check_kernel
import check_metamorphic
import corpus
import corpus_report
import corpus_summary


def case():
    return {'paths':['vendor/source/test.step'],'sha256':'abc','status':'clean','stages':{'physical_parse':'accepted','references':'resolved','schema':'not_configured','product':'not_configured','geometry':'not_integrated','tessellation':'not_integrated'},'diagnostic_counts':{},'diagnostics':[], 'seconds':1,'bytes':30,'missing_references':0,'entity_count':1}


def report(c):
    return {'format_version':2,'cases':[c],'baseline_present':False,'baseline_changes':[], 'summary':{'files':len(c['paths']),'unique_inputs':1,'unique_statuses':{c['status']:1},'seconds':1},'binary_sha256':'abc','source_fingerprint':'abc','run_id':'test','corpus_root':'/test','history':[]}


class ReportTests(unittest.TestCase):
    def test_sources_do_not_inflate_duplicate_coverage(self):
        c=case();c['paths'].append('vendor/other/alias.step')
        a=corpus_report.aggregate([c])
        self.assertEqual(sum(x['unique_inputs'] for x in a['sources'].values()),1)
        self.assertEqual(sum(x['paths'] for x in a['sources'].values()),2)
        self.assertEqual(a['stages']['schema']['tested'],0)
        self.assertEqual(a['stages']['geometry']['tested'],0)

    def test_unreviewed_inputs_are_skipped_in_junit(self):
        r=report(case());x=ET.fromstring(corpus_summary.junit(r))
        self.assertEqual(x.get('skipped'),'1')
        r['cases'][0]['expectations']={'test.step':{'status':'clean'}}
        self.assertEqual(ET.fromstring(corpus_summary.junit(r)).get('skipped'),'0')

    def test_html_escapes_external_names_and_diagnostics(self):
        c=case();c['paths']=['<script>alert(1)</script>.step'];c['stderr']='</pre><img src=x onerror=alert(1)>'
        rendered=corpus_report.render(report(c))
        self.assertNotIn('<script>alert(1)',rendered)
        self.assertNotIn('<img src=x',rendered)
        self.assertIn('&lt;img src=x',rendered)
        self.assertIn('not_integrated',rendered)

    def test_xml_controls_and_csv_formula_names_are_safe(self):
        c=case();c['paths']=['=cmd\x00.step'];c['status']='runner_error'
        x=ET.fromstring(corpus_summary.junit(report(c)))
        self.assertEqual(x.get('failures'),'1')
        self.assertIn("'=cmd",corpus_summary.csv_text(report(c)))

    def test_reviewed_rejection_passes_but_wrong_code_fails(self):
        c=case();c.update(status='rejected',diagnostic_counts={'TS1002':1})
        expected={c['paths'][0]:{'status':'rejected','diagnostic_codes':['TS1002']}}
        corpus.check_expectations(c,expected)
        self.assertEqual(corpus_report.verdict(c),'passed')
        expected[c['paths'][0]]['diagnostic_codes']=['TS1008']
        corpus.check_expectations(c,expected)
        self.assertEqual(corpus_report.verdict(c),'failed')
        self.assertEqual(ET.fromstring(corpus_summary.junit(report(c))).get('failures'),'1')

    def test_stage_only_regression_has_actionable_details(self):
        old=case();old['stages']['schema']='accepted'
        current=case();current['stages']['schema']='rejected'
        change=corpus.compare([current],{'cases':[old]})[0]
        self.assertEqual(change['kind'],'regression')
        self.assertEqual(change['details']['stages']['before']['schema'],'accepted')
        self.assertEqual(change['details']['stages']['after']['schema'],'rejected')

    def test_report_vocabulary_does_not_rewrite_baseline(self):
        old=case()
        for s in ('schema','product','geometry','tessellation'):old['stages'][s]='not_implemented'
        new=case()
        self.assertEqual(corpus.compare([new],{'cases':[old]}),[])
        self.assertEqual(old['stages']['geometry'],'not_implemented')

    def test_worsening_dangling_references_is_a_regression(self):
        old=case();old.update(status='reference_errors',missing_references=1)
        new=copy.deepcopy(old);new['missing_references']=2
        self.assertEqual(corpus.compare([new],{'cases':[old]})[0]['kind'],'regression')

    def test_added_inputs_are_unreviewed_even_with_a_baseline(self):
        c=case();c['baseline_change']='added'
        self.assertEqual(corpus_report.verdict(c,True),'unreviewed')

    def test_newly_accepted_rejection_with_null_measurements(self):
        old=case();old.update(status='rejected',missing_references=None)
        old['stages']['physical_parse']='rejected'
        self.assertEqual(corpus.compare([case()],{'cases':[old]})[0]['kind'],'changed')

    def test_numerical_product_expectations_appear_in_report_failures(self):
        c=case();c['product_result']={'count':2,'relationship_matrices':[[[1.0,0.0]]], 'values':[1e-5]}
        expected={c['paths'][0]:{'product_result_contains':{'count':2,'values':[1e-5]},'first_relationship_matrix':[[1.0,0.0]]}}
        corpus.check_expectations(c,expected)
        self.assertEqual(corpus_report.verdict(c),'passed')
        c['product_result']['relationship_matrices'][0][0][0]=1.1
        corpus.check_expectations(c,expected)
        self.assertEqual(corpus_report.verdict(c),'failed')
        self.assertIn('matrix',c['expectation_failures'][0])
        with self.assertRaises(ValueError):corpus.check_expectations(c,{c['paths'][0]:{}})

    def test_removed_inputs_fail_junit_and_dashboard_count(self):
        r=report(case());r['baseline_present']=True
        r['baseline_changes']=[{'kind':'removed','path':'missing.step','sha256':'def'}]
        x=ET.fromstring(corpus_summary.junit(r))
        self.assertEqual(x.get('failures'),'1')
        self.assertEqual(x.get('tests'),'2')
        self.assertIn('<strong>1</strong>Failed / missing inputs',corpus_report.render(r))

    def test_repeated_output_mismatch_and_timeout_fail(self):
        for again in ({'status':'clean','seconds':1,'output_sha256':'changed'}, {'status':'timeout','seconds':1}):
            c=case();c['output_sha256']='original'
            with patch.object(corpus,'inspect',return_value=again):
                corpus.repeat_inspection(Path('bin'),Path('input'),1,c,2)
            self.assertIn(c['status'],{'nondeterministic','timeout'})
            self.assertEqual(corpus_report.verdict(c),'failed')

    def test_physical_protocol_disagreement_is_not_clean(self):
        payload={'format_version':1,'scope':'physical-syntax','diagnostics':[],'document':None}
        def run(*args,**kwargs):
            kwargs['stdout'].write(json.dumps(payload).encode())
            return subprocess.CompletedProcess(args,0)
        with patch.object(corpus.subprocess,'run',side_effect=run):
            self.assertEqual(corpus.inspect(Path('bin'),Path('input'),1)['status'],'runner_error')

    def test_generated_fixtures_are_reproducible_and_have_oracles(self):
        fixtures=check_metamorphic.fixtures()
        self.assertEqual(fixtures,check_metamorphic.fixtures())
        self.assertGreaterEqual(len(fixtures),70)
        self.assertTrue(all(expected.get('status') for _,expected in fixtures.values()))

    def test_kernel_log_parser_preserves_failures_ignored_and_doctests(self):
        log='''    Running tests/example.rs (target/debug/deps/example-0123456789abcdef)
 test area ... ok
 test bad ... FAILED
 test optional ... ignored, expensive
   Doc-tests tessstep
 test crates/lib.rs - (line 5) ... ok
'''
        tests=check_kernel.parse_tests(log)
        self.assertEqual([t['status'] for t in tests],['passed','failed','skipped','passed'])
        self.assertEqual(tests[0]['suite'],'example')
        self.assertEqual(tests[-1]['suite'],'tessstep doctests')


if __name__=='__main__': unittest.main()
