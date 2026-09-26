"""Version 2 report aggregation and offline, searchable HTML presentation."""
from collections import Counter
import html
import json

STAGES = ("physical_parse", "references", "schema", "product", "geometry", "tessellation")
FAULTS = {"crash", "timeout", "runner_error", "nondeterministic"}
UNTESTED = {"not_run", "not_configured", "not_integrated", "not_implemented"}


def source_of(path):
    parts = path.split("/")
    return parts[1] if len(parts) > 2 and parts[0] == "vendor" else parts[0] if len(parts) > 1 else "authored"


def stages_of(case):
    stages = {stage: "not_run" for stage in STAGES}
    stages.update(case.get("stages", {}))
    for stage in ("schema", "product"):
        if stages[stage] == "not_implemented":
            stages[stage] = "not_configured"
    for stage in ("geometry", "tessellation"):
        if stages[stage] == "not_implemented":
            stages[stage] = "not_integrated"
    return stages


def verdict(case, baseline_present=False):
    if case.get("status") in FAULTS or case.get("expectation_failures"):
        return "failed"
    if case.get("baseline_change") in {"regression", "removed"}:
        return "failed"
    if case.get("expectations"):
        return "passed"
    if case.get("baseline_change") == "added" or not baseline_present:
        return "unreviewed"
    return "compatible"


def geometry_totals(cases):
    """Root-level solid import outcomes; categories count roots, not files."""
    results = [c["geometry_result"] for c in cases if c.get("geometry_result")]
    total = lambda key: sum(r[key] for r in results)
    merged = lambda key: dict(sum((Counter(r[key]) for r in results), Counter()).most_common(15))
    return {"files_surveyed": len(results), "files_with_roots": sum(r["root_count"] > 0 for r in results),
            "root_count": total("root_count"), "surveyed_roots": total("surveyed_roots"),
            "geometry_roots": total("geometry_roots"), "accepted_roots": total("accepted_roots"),
            "outcomes": merged("outcomes"), "categories": merged("categories"),
            "unsupported_entity_types": merged("unsupported_entity_types")}


def aggregate(cases, baseline_present=False):
    sources = {}
    for case in cases:
        # Assign a primary source once. Keep aliases visible without inflating totals.
        source = source_of(case["paths"][0])
        row = sources.setdefault(source, {"unique_inputs": 0, "paths": 0, "bytes": 0, "statuses": Counter(), "verdicts": Counter()})
        row["unique_inputs"] += 1
        row["paths"] += len(case["paths"])
        row["bytes"] += case.get("bytes", 0)
        row["statuses"][case["status"]] += 1
        row["verdicts"][verdict(case, baseline_present)] += 1
    stages = {}
    for stage in STAGES:
        counts = Counter(stages_of(c)[stage] for c in cases)
        tested = sum(n for status, n in counts.items() if status not in UNTESTED)
        stages[stage] = {"tested": tested, "total": len(cases), "statuses": dict(sorted(counts.items()))}
    times = sorted(c.get("seconds", 0) for c in cases)
    return {"sources": dict(sorted(sources.items())), "stages": stages, "geometry": geometry_totals(cases),
            "verdicts": dict(Counter(verdict(c, baseline_present) for c in cases)),
            "diagnostics": dict(Counter(code for c in cases for code in c.get("diagnostic_counts", {}))),
            "performance": {"total_input_bytes": sum(c.get("bytes", 0) for c in cases),
                            "median_seconds": times[len(times)//2] if times else 0,
                            "p95_seconds": times[min(len(times)-1, int(len(times)*0.95))] if times else 0,
                            "slowest": [{"path": c["paths"][0], "seconds": c.get("seconds", 0)} for c in sorted(cases, key=lambda c: c.get("seconds", 0), reverse=True)[:10]]}}


def render(report):
    esc = lambda value: html.escape(str(value), quote=True)
    cases = report["cases"]
    stats = aggregate(cases, report.get("baseline_present", False))
    counts = report["summary"]["unique_statuses"]
    cards = [("Unique inputs", len(cases)), ("Paths", report["summary"]["files"]), ("Physically clean", counts.get("clean", 0)),
             ("Failed / missing inputs", stats["verdicts"].get("failed", 0) + sum(c["kind"] == "removed" for c in report.get("baseline_changes", []))), ("Unreviewed", stats["verdicts"].get("unreviewed", 0))]
    source_rows = ''.join(f'<tr><td>{esc(name)}</td><td>{s["unique_inputs"]}</td><td>{s["paths"]}</td><td>{s["statuses"].get("clean",0)}</td><td>{s["statuses"].get("reference_errors",0)}</td><td>{s["statuses"].get("rejected",0)}</td><td>{s["verdicts"].get("failed",0)}</td></tr>' for name,s in stats["sources"].items())
    stage_rows = ''.join(f'<tr><td>{esc(stage)}</td><td><meter min="0" max="{max(1,len(cases))}" value="{s["tested"]}"></meter> {s["tested"]} / {len(cases)}</td><td>{esc(", ".join(f"{k}: {v}" for k,v in s["statuses"].items()))}</td></tr>' for stage,s in stats["stages"].items())
    rows = []
    for case in cases:
        v = verdict(case, report.get("baseline_present", False))
        stages = stages_of(case)
        detail = {k: case[k] for k in ("expectations", "expectation_failures", "schemas", "entity_counts", "missing_references", "diagnostics", "diagnostic_counts", "stderr", "schema_result", "schema_stderr", "product_result", "product_stderr", "geometry_result", "geometry_stderr", "determinism", "baseline_change", "output_sha256", "sha256", "bytes") if k in case}
        name = case["paths"][0]
        paths = '<br>'.join(esc(p) for p in case["paths"])
        rows.append(f'<tr data-status="{esc(case["status"])}" data-verdict="{v}" data-source="{esc(source_of(name))}" data-stages="{esc(json.dumps(stages))}"><td><details><summary>{esc(name)}</summary>{paths}<pre>{esc(json.dumps(detail,indent=2,ensure_ascii=False))}</pre></details></td><td><span class="pill {v}">{v}</span></td><td>{esc(case["status"])}</td>'
                    + ''.join(f'<td class="stage">{esc(stages[s])}</td>' for s in STAGES)
                    + f'<td>{case.get("entity_count", "—")}</td><td>{case.get("seconds",0):.3f}</td></tr>')
    g = stats["geometry"]
    survey = report.get("geometry_survey") or {}
    table = lambda title, counts: f'<div class="panel"><table><tr><th>{esc(title)}</th><th>Roots</th></tr>' + ''.join(f'<tr><td>{esc(k)}</td><td>{v}</td></tr>' for k, v in counts.items()) + '</table></div>'
    geometry_panel = (f'<h2>Solid import outcomes</h2><p class="muted">Every MANIFOLD_SOLID_BREP, BREP_WITH_VOIDS and FACETED_BREP root, imported with the selected-root planar/faceted profiles at an assumed {esc(survey.get("metres_per_unit", "—"))} metres per source unit. '
                      f'{g["root_count"]:,} roots in {g["files_with_roots"]:,} files; {g["geometry_roots"]:,} passed geometry/topology and {g["accepted_roots"]:,} produced solid meshes.</p>'
                      + table("First failing stage : kind", g["outcomes"]) + table("Failure category (IDs as N)", g["categories"]) + table("Entity type outside the import profile", g["unsupported_entity_types"])) if g["files_surveyed"] else ""
    changes = report.get("baseline_changes", [])
    change_rows = ''.join(f'<tr><td>{esc(c["kind"])}</td><td>{esc(c["path"])}</td><td>{esc(json.dumps(c.get("details", {"before":c.get("before"),"after":c.get("after")})))}</td></tr>' for c in changes)
    history = ''.join(f'<tr><td>{esc(h["run_id"])}</td><td>{h["unique_inputs"]}</td><td>{h["unique_statuses"].get("clean",0)}</td><td>{h["unique_statuses"].get("rejected",0)}</td><td>{h["unique_statuses"].get("reference_errors",0)}</td><td>{h.get("seconds",0):.1f}</td></tr>' for h in report.get("history", [])[-30:])
    options = lambda values: ''.join(f'<option value="{esc(v)}">{esc(v)}</option>' for v in values)
    return '''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>TessSTEP test coverage</title><style>
:root{color-scheme:light}*{box-sizing:border-box}body{font:14px system-ui;margin:0;background:#f3f6fa;color:#152435}header{padding:28px 32px;background:#122b42;color:white}h1{margin:4px 0 12px;font-size:30px}main{padding:24px 32px}h2{font-size:20px;margin:28px 0 12px}.muted{color:#607087}.cards{display:flex;gap:12px;flex-wrap:wrap}.card{background:white;border:1px solid #dbe3ed;border-radius:10px;padding:18px;min-width:150px;flex:1}.card strong{display:block;font-size:28px;margin-bottom:5px}.panel{background:white;border:1px solid #dbe3ed;border-radius:10px;overflow:auto;margin:12px 0}table{border-collapse:collapse;width:100%}td,th{text-align:left;border-bottom:1px solid #e3e9f0;padding:10px;vertical-align:top}th{background:#eaf0f6;white-space:nowrap}.stage{font-size:12px}summary{cursor:pointer}pre{white-space:pre-wrap;overflow-wrap:anywhere;max-width:720px;font-size:12px}td:first-child{min-width:260px;max-width:560px;overflow-wrap:anywhere}meter{width:100px;vertical-align:middle}.pill{display:inline-block;padding:3px 7px;border-radius:12px;background:#eaf0f6}.passed,.compatible{background:#dff2e9;color:#155b3e}.failed{background:#ffe0e0;color:#9a1f1f}.unreviewed{background:#fff0cd;color:#694a00}input,select{font:inherit;padding:9px;border:1px solid #bccada;border-radius:6px;margin:4px}input{min-width:270px}.filters{display:flex;flex-wrap:wrap;align-items:center}a{color:#2564b2}.note{border-left:4px solid #378ab0;padding:10px 16px;background:#eaf3f9;line-height:1.6}button{padding:8px;border:1px solid #bccada;border-radius:6px;background:white;cursor:pointer}@media(max-width:700px){main,header{padding:18px}.card{min-width:130px}h1{font-size:25px}}
</style></head><body>''' + f'''<header><div>TEST EVIDENCE · REPORT V2</div><h1>{esc(report.get("title", "TessSTEP external STEP compatibility"))}</h1><div>{esc(report["run_id"])} · source {esc(report["source_fingerprint"][:12])} · {report["summary"]["seconds"]:.1f}s</div></header><main>
<div class="cards">{''.join(f'<div class="card"><strong>{n:,}</strong>{esc(label)}</div>' for label,n in cards)}</div>
<p class="note">Physical acceptance is not CAD conformance. Rejected adversarial files may be correct. <b>Passed</b> means reviewed expectations matched; <b>compatible</b> means no regression against an observation baseline; <b>unreviewed</b> has no oracle.<br>Schema/product checks require configured validators. Geometry and tessellation import each solid root with bounded selected-root profiles; <b>partial</b> means only some roots in the file imported. Profile rejection is not an AP validity verdict. <b>Not configured, not integrated and not run are not passes.</b></p>
<p><a href="latest.json">JSON</a> · <a href="summary.md">Markdown summary</a> · <a href="junit.xml">JUnit</a> · <a href="cases.csv">CSV</a></p>
<h2>Coverage by stage</h2><div class="panel"><table><tr><th>Stage</th><th>Executed / unique inputs</th><th>Outcomes</th></tr>{stage_rows}</table></div>{geometry_panel}
<h2>Coverage by source</h2><p class="muted">One primary source per unique content; duplicate aliases remain listed per file.</p><div class="panel"><table><tr><th>Source</th><th>Unique</th><th>Paths</th><th>Clean</th><th>Missing references</th><th>Rejected</th><th>Failed checks</th></tr>{source_rows}</table></div>
<details><summary>Baseline changes ({len(changes)})</summary><div class="panel"><table><tr><th>Change</th><th>File</th><th>Changed measurements</th></tr>{change_rows}</table></div></details>
<details><summary>Diagnostics and performance</summary><pre>{esc(json.dumps({"files_by_diagnostic":stats["diagnostics"],"performance":stats["performance"]},indent=2))}</pre></details>
<details><summary>Run history (latest 30)</summary><div class="panel"><table><tr><th>Run UTC</th><th>Unique</th><th>Clean</th><th>Rejected</th><th>Reference errors</th><th>Seconds</th></tr>{history}</table></div></details>
<h2>Per-file evidence</h2><div class="filters"><input id="query" aria-label="Search files and diagnostics" placeholder="Search filename, schema, diagnostic…"><select id="status" aria-label="Physical outcome"><option value="">All physical outcomes</option>{options(sorted(counts))}</select><select id="verdict" aria-label="Check verdict"><option value="">All verdicts</option>{options(sorted(stats["verdicts"]))}</select><select id="source" aria-label="Source"><option value="">All sources</option>{options(stats["sources"])}</select><select id="stage" aria-label="Stage"><option value="">All stages</option>{options(STAGES)}</select><select id="stage-status" aria-label="Stage outcome"><option value="">All stage outcomes</option>{options(sorted({s for c in cases for s in stages_of(c).values()}))}</select><button id="reset">Reset</button><span id="visible" aria-live="polite"></span></div>
<div class="panel"><table id="files"><thead><tr><th>File / details</th><th>Check verdict</th><th>Physical outcome</th>{''.join(f'<th>{s}</th>' for s in STAGES)}<th>Entities</th><th>Seconds</th></tr></thead><tbody>{''.join(rows)}</tbody></table></div><p class="muted">Root: {esc(report["corpus_root"])} · baseline {esc(report.get("baseline_sha256") or "not configured")}</p></main>''' + '''<script>
const rows=[...document.querySelectorAll('#files tbody tr')],ids=['query','status','verdict','source','stage','stage-status'],controls=Object.fromEntries(ids.map(id=>[id,document.getElementById(id)]));
const cache=rows.map(r=>({r,text:r.textContent.toLowerCase(),stages:JSON.parse(r.dataset.stages)}));
function filter(){let n=0;for(const {r,text,stages} of cache){const stage=controls.stage.value,ss=controls['stage-status'].value;r.hidden=!(text.includes(controls.query.value.toLowerCase())&&['status','verdict','source'].every(k=>!controls[k].value||r.dataset[k]===controls[k].value)&&(!ss||(stage?stages[stage]===ss:Object.values(stages).includes(ss))));if(!r.hidden)n++;}document.getElementById('visible').textContent=n+' / '+rows.length+' unique inputs';}
for(const c of Object.values(controls))c.addEventListener('input',filter);document.getElementById('reset').addEventListener('click',()=>{for(const c of Object.values(controls))c.value='';filter();});filter();
</script></body></html>'''
