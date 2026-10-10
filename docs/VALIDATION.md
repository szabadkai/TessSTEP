# HOOPS Exchange parity on the PreForm collection — 2026-10-10

An internal, non-redistributable collection of 54 STEP files used by PreForm (NIST
D2MI, PMI and UUID models, user samples and one 55 MB, 646-root support attachment)
was surveyed with the release `survey` example and compared with a HOOPS Exchange
probe of the same files (its unsimplified mesh volumes). Nothing from the collection is
in this repository. Before the parity work, at 1.0.0: 51 of 54 files parsed and 44
meshed every root. After the parser, tessellation, loop-healing and assembly-linking
changes above: all 54 parse and 53 mesh every root; the 646-root attachment, for which
the HOOPS probe recorded no mesh, meshes 545 roots (the rest are deviations beyond the
survey chord or the 16× policy, unsewn shells and other named failures). Where both
mesh every root, 47 of 53 files agree with HOOPS within 0.1% in volume and 51 within
0.6%. Three of the larger gaps are HOOPS meshes that are open (10 to 524 boundary
edges), where TessSTEP agrees with the closed exporter variants of the same parts; the
other is a 270 mm³ part at −0.56%. HOOPS's assembly volume for one Shapr3D export counts
every body twice; TessSTEP's per-body total matches half of it within 0.13%.

The reviewed corpus baseline was saved after these changes: 3,227 compatible inputs,
13 more physically accepted files (tolerated exporter slips), 778 meshed shape roots and
1,130 meshed surface models, no lost mesh.

# HOOPS parity: input-sized parse budgets and two exporter slips — 2026-10-10

Three PreForm CAD collection files failed at the physical parser. HOOPS Exchange opens
the two NIST files and has no result for the first (its probe exits 1):

| File (PreForm collection) | 1.0 rejection | Cause |
| --- | --- | --- |
| `jira/PP-30107/Bad_STEP.stp` (55.7 MB) | `TS1201` total value budget exceeded (limit 4000000) | 951,613 entities, 961,634 records and 4,582,879 values, 15 % over the default value budget |
| `nist/d2mi/827-9999-904 rev c.stp` | `TS1006` unknown string directive, line 45751 | one unescaped backslash in a material path string |
| `nist/pmi/nist_stc_10_asme1_ap242-e2.stp` | `TS1011` FILE_NAME structure | `$` as FILE_NAME authorisation |

**Budgets sized from the input.** `ParseLimits::for_input(bytes)` keeps every default
and raises `max_input_bytes` to the length and the entity, record, total value and
aggregate element budgets to the most of each that many bytes can hold (7, 3, 2 and 2
bytes per item: `#1=A();`, `A()`, `$,`), never below the defaults
([PART21.md](PART21.md#budgets)). The budgets stay explicit and named in their
diagnostics. Across the 2,913 parseable inputs of both collections the densest holds a
value per 5.45 bytes (`nist_stc_09`, tessellated), the largest entity count is
Bad_STEP's, and the longest list has 44,848 elements. `stepdump` sizes a file's budgets
by default (`--default-budgets` and standard input keep the defaults; `--max-<name>=N`
applies after sizing), as do the survey, the assembly example, generated validators and
so the corpus runner, and C callers through `ts_parse_options_for_input` (ABI 1.1).

**Memory.** At 1 GB peak RSS for a 55.7 MB file, parsing Bad_STEP spent most of its
memory on two avoidable allocations. `tessstep_model::parse` kept a second map of every
ID with its source span only to report duplicates, now found through the entity index
and the external references (−99 MB). List elements were pushed into growing vectors,
leaving 2.53 million unused slots of 80 bytes behind; they now collect on one shared
stack and move into an exactly sized vector when the list closes (−185 MB; trimming
each vector afterwards freed nothing, because the allocator kept the fragments). The
rest is the public 1.x representation: 80 bytes per value, of which the source span
takes 48, 144 per entity. `Document::diagnostics` and `stepdump` now collect only
missing references instead of all 2–3 million reference uses. The survey prepared
every root's shared decode at once (350 MB for this file); it now prepares 64 solid
roots at a time, just before importing them, with identical results.

| Bad_STEP (release, this machine, other agents running) | 1.0 (budget raised) | Now |
| --- | ---: | ---: |
| Parse only | 853 MB, 1.81 s | 569 MB, 1.66 s |
| `stepdump --json` | 991 MB, 1.72 s | 568 MB, 1.6–2.2 s |
| Survey (every root) | rejected at parse (716 MB) | 790 MB, 8.1 s (999 MB preparing every root at once) |

**Exporter slips.** The parse is tolerant by default ([PART21.md](PART21.md#exporter-slips)):
a string backslash whose following characters complete no directive opening is a
literal backslash, and `$` stands for a FILE_DESCRIPTION or FILE_NAME string or list
element. Each kind is counted (`Document::deviations`, with its first location) and
reported as one warning with the code a strict parse fails with (`TS1006`, `TS1011`).
A complete directive opening with a malformed payload still fails, as do backslashes
outside strings and `$` in FILE_SCHEMA. `Conformance::Strict`, `stepdump --strict` and
`survey --strict-syntax` reject both slips; a strict parse accepts exactly what 1.0
accepted. The authored fixture `corpus/part21/tolerated/exporter-slips.step` and an
authored variant of the cylinder fixture reproduce both slips; the cylinder imports
with the same mesh as the conforming file. The C entry point `ts_document_parse` stays
strict, because its ABI 1.0 contract rejects both slips; `ts_document_parse_with_policy`
(ABI 1.1, the existing `ts_import_policy`, `TS_IMPORT_STRICT`) has the tolerant
default, and `ts_document_deviation_count` reads the counts
([C API](C_API.md#abi-11-parse-budgets-and-exporter-slips)).

| File | Survey now | HOOPS (`simplifyMm=-1`) | Δ volume |
| --- | --- | --- | ---: |
| `827-9999-904 rev c.stp` (1 literal backslash) | 1 root, closed, 20,828 triangles, 22,862.98 mm³, 0.14 s | 20,442 triangles, boundary 0, 22,868.865 mm³ | −0.026 % |
| `nist_stc_10_asme1_ap242-e2.stp` (1 `$`) | 1 root, closed, 85,808 triangles, 188,407.39 mm³, 0.37 s; 68 of 68 annotation occurrences | 201,114 triangles, boundary 0, 188,338.903 mm³ | +0.036 % |
| `Bad_STEP.stp` | 646 solid roots, 545 closed, 862,042 triangles, 8,124,245 mm³; 8.1 s, 790 MB | no result (exit 1) | — |

`Bad_STEP.stp` now imports end to end. Its 209 products place 302 occurrences of 646
solid roots (643 `MANIFOLD_SOLID_BREP`, 3 `BREP_WITH_VOIDS`); the 1,300+ bodies count
occurrences. The survey now surveys every root unless `--max-roots=N` or `MAX_ROOTS`
bound it (1,000 before; no corpus file reaches it), so nothing is truncated. No stage
meets a budget: import budgets follow the entity count and each group's shared decode
stays within its budget. Of the 8.1 s, parsing takes about 1 s, discovery, selection
and the shared decodes about 1.5 s, and the 646 roots 4.9 s. Of the 101 roots that do
not mesh, none fails on scale. 65 accept an edge-to-surface deviation within 16 times
their model tolerance but larger than the survey's chord, 1e-3 of the root's diagonal
(clamped to 1 µm–1 mm), so the mesh could not stay within it. 15 have an edge or vertex
beyond 16 times the tolerance. 10 are unsewn shells, which `--repair` sews (555 roots),
and 1 has a non-manifold vertex. 9 fail tessellation (boundary resolution, UV trimming,
a duplicate triangle), 1 contradicts its `same_sense` and 1 is a horn torus. On
`90c06b6` with this change 536 roots meshed; the fillet-band fix of `572a847` adds 9.
The other 51 files of the collection survey exactly as before this change; their peak
RSS falls with the parse memory (keycaps 315 → 187 MB at 1.37 s).

**Corpus movements and the default.** The full corpus check against the saved 1.0.0
baseline moves 14 inputs, all physical-parse outcomes. 13 rejected inputs now parse
with counted slips (67 literal backslashes, 4 `$` header strings) and one keeps its
rejection with another code:

| Input | Slip | Now | The file's own expectation |
| --- | --- | --- | --- |
| NIST `nist_stc_10_asme1_ap242-e2.stp` | `$` authorisation | every stage accepted (1 root, 68 annotations) | — (HOOPS opens it) |
| OCCT `bug32049_sp7_10nx_242.stp` (NX via ST-Developer) | `$` authorisation | geometry and tessellation accepted, 78 annotations; schema `RequiredValue` | — |
| OCCT `mat.stp` (ST-Developer 1.6) | `$` originating system and authorisation | schema `RequiredValue`, product rejected | — |
| OCCT `bug29478.stp` | `\\tsclient\D\…` in FILE_NAME | product accepted; schema `AttributeCount`; its surface model fails in the profile | — |
| OCCT `trj9_ap209-mp.stp` (MSC Patran) | `D:\TEMP\…` in FILE_NAME | AP209 not configured; its 4 roots fail in geometry and topology | — |
| dodgy `Le012` | `\Z\`, `\M\` | parsed, warning | "a tolerant receiver must pass the bytes through literally with a warning" |
| dodgy `Le016` | Windows paths including `\X1` | parsed, warning | tolerant readers warn and keep literal backslashes |
| dodgy `Le040`–`Le042` | `\Q\NNN\Q\` references | parsed literally, warning | a tolerant mode may pass them through with a warning; never substitute |
| dodgy `Le046`, `Le049` | `\]`, `\n` | parsed, warning | heal and accept |
| dodgy `Le009` | `\Pg\`, `\P1\`, `\P!\` | parsed literally, warning | asks receivers to reject out-of-range page selectors |
| dodgy `Le030` | `\Q\'` | still rejected: `TS1002` after the apostrophe ends the string, not `TS1006` | warn and accept |

The dodgy encoding files' schema stage rejects their own `PERSON` instances
(`AttributeCount`). Every other dodgy encoding, header and syntax defect keeps its
rejection: malformed hexadecimal groups, surrogates, unterminated or nested `\X2\`,
`\X\` with bad digits, control directives outside strings (`Wr049`, `Ls045`), missing
or misshaped header records (FILE_DESCRIPTION without its level, eight FILE_NAME
parameters, a bare FILE_SCHEMA string, integer implementation levels), invalid UTF-8,
and the nesting-depth bombs `Ad002` and `Xp023`, which input-sized budgets leave alone.
The tolerance is on by default: 12 of the 13 newly parsed inputs are exporter files
that HOOPS or OCCT open, or test files whose stated tolerant expectation this behaviour
meets. `Le009` is accepted against its stated expectation, with a warning, because its
selectors complete no directive opening and the rule is that such a backslash is
literal (a later `\S\` then decodes in the unchanged page); the strict mode rejects it.

The run used a validator built from this commit (`build_ap_validator.py`, the same
schemas and map arguments). With the shared validator, built before this change, its
own strict parse rejects the 12 configured moved files and reports `not_run`, which the
runner counts as 12 runner errors; nothing else differs. Rebased on `3b845e2`, the check
moves 32 inputs: these 14 and the 18 of the keycaps, tessellation and
loop-healing work. A run of this change on `90c06b6` moved only the 14, with the same
signatures, and differs from the rebased run in exactly those 18 inputs.

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | passed (93 test binaries, 366 tests), including `tolerant_strings_keep_backslashes_that_start_no_directive`, `tolerant_headers_accept_null_strings_in_file_description_and_file_name`, `input_sized_budgets_cover_what_the_input_can_hold`, `tolerated_slips_are_counted_and_reported_as_warnings`, `exporter_slips_import_like_the_conforming_file` and the stepdump CLI test |
| fmt, Clippy (`-D warnings`, all targets and features), `cargo doc` (`-D warnings`) | passed |
| `check_capi.py --library-dir target/release` (no sanitizers) | passed: exactly the 98 recorded exports (95 of 1.0, 3 of 1.1), shared and static consumers, including the new C and C++ parse policy contracts |
| `check_architecture.py`, `check_corpus.py` (143 fixtures), `check_cli.py`, `conformance.py --check`, script unit tests (75) | passed |
| `package_release.py` (aarch64-apple-darwin) | passed; `PACKAGE.json` records ABI level 1.1 |
| `corpus.py --check --repeat 2`, validator built from this commit, saved 1.0.0 baseline | 3,227 compatible, no regressions, 368 s; 32 inputs changed (14 here, 18 from main) |
| the same with the shared validator | 12 runner errors (the stale validator's strict parse), otherwise identical |
| PreForm bench (54 files) | 54 parse (51 before), 53 mesh every root |

# Loop healing for HOOPS parity on the PreForm collection — 2026-10-10

Two NIST PMI bodies in the PreForm CAD collection are meshed closed by HOOPS Exchange
(SUMMARY lines, `simplifyMm=-1`). TessSTEP did not mesh them, even with the opt-in
repair. Each defect was located in its STEP file (face, loop and edge) and then
reproduced with an original authored fixture. No collection file was copied.

| File | Where | STEP says | Geometry implies | Cause | Handling |
| --- | --- | --- | --- | --- | --- |
| `nist_stc_06_asme1_ap242-e3` | planar face #412 (x = 31.75 mm) | `FACE_OUTER_BOUND` #333 is one 10.32 mm circle (#330); `FACE_BOUND` #406 is an 11-edge rim | the circle runs clockwise in the chart (-3.3e-4 m², a hole); the rim runs counterclockwise (+7.5e-3 m²) and encloses it | exporter label on the wrong loop | the default import uses the rim as the outer bound (counted); strict import keeps the label |
| `nist_stc_07_asme1_ap242-e3` | half drill-point cone #2277 (`CONICAL_SURFACE` #2250), generator `LINE` #2271 | origin 196,850 in (5.0 km) from the apex; the line passes 7.3e-9 m from it | the exact image ends at v = -9.09e-13 m at the apex | ours: see below | an end within the model tolerance below the apex is placed on it |
| same | cylinder face #8915 (r = 0.12 in), B-spline `EDGE_CURVE` #2204 | — | peaks 5.19 µm off the cylinder at 41% of its span (1.038 × the 5 µm tolerance); 4.93 µm at the fit's mid sample | ours: see below | the peak between samples is refined and counted as an off-surface edge (working tolerance 1.54e-5 m) |

Causes of the two `nist_stc_07` defects, both ours:
* **Generator `LINE` #2271.** The cone's `v >= 0` domain rejected the exact image.
  The fitted fallback then took its apex coordinate from a point 3.7e-10 m along the
  line, where the 7.3e-9 m miss dominates. So its apex u was off by π, and the apex
  join became a full turn.
* **`EDGE_CURVE` #2204.** The fit measured the edge within the tolerance, so the
  working tolerance was not widened. Trimming then sampled the edge more densely and
  rejected it (`CurveSurfaceMismatch`).

**Why the tolerant default, not the repair stage.** The `FACE_OUTER_BOUND` rule
changes no loop, sense, orientation, vertex or coordinate. It only chooses which loop
bounds the face from outside, the same kind of choice as the existing inferred outer
bound. That inferred bound is also a counted default adaptation
([curved import](CURVED_IMPORT.md#face-charts)). The opt-in repair stage edits
topology: it merges, flips and reverses ([topology](TOPOLOGY.md#opt-in-repair)). Its
loop reversal would not help here: reversing face #412 makes the circle the
counterclockwise loop, with the rim outside it.

The rule applies only when the geometry admits one reading:
* the declared loop runs clockwise;
* exactly one other loop runs counterclockwise;
* that loop encloses every other loop's sampled chart polygon.

`ImportOptions::strict` keeps the label, as for other exporter deviations that the
default accepts without changing geometry, and trimming rejects it as before. Loops
written against their face are not relabelled and stay rejected:
* with one bound reversed, no loop runs counterclockwise;
* with every bound reversed, the counterclockwise loop encloses nothing.

The two pcurve changes are importer fixes, not adaptations, and also apply under
strict import. There, the refined peak of #2204 is now a geometry-stage rejection that
names the distance, where trimming used to reject the edge.

**Outcomes against HOOPS** (release survey and `bench_preform.py`, on 572a847 with and
without this change):

| File | 572a847 | With this change | HOOPS |
| --- | --- | --- | --- |
| `nist_stc_06_asme1_ap242-e3` | tessellation: outer bound of #412 | closed, 32,864 triangles, 3,322,664.55 mm³, 0.16 s | 3,323,476.49 mm³ (-0.024%); TessSTEP's `ftc_06_asme1_rd`, 3,322,666.29 mm³ (-0.00005%) |
| `nist_stc_07_asme1_ap242-e3` | geometry unsupported (#2277) | closed, 39,972 triangles, 1,726,334.27 mm³, 8 off-surface edge uses, 0.20 s | 1,726,232.53 mm³ (+0.0059%); TessSTEP's `ftc_07` variants, 1,726,297.95 and 1,726,372.41 mm³ |

On 90c06b6, `stc_06` got past its loops and then stopped at toroidal face #2952 on the
`max_rounds` refinement budget, like both `ftc_06` variants. That face is the same
fillet (R = 14.605, r = 3.175 mm) between the inner equator and the 140° parallel. Its
chart was dumped, and it is the expected 40° band. The refinement fix in 572a847
resolves it. Across the collection, 51 of 54 files now mesh every root (49 on
572a847). The two files above are the only outcomes that differ.

**Fixtures** ([`brep_fixtures.py`](../corpus/geometry/brep_fixtures.py),
`tests/loop_healing.rs`) each fail at 90c06b6 with the PreForm file's message, and now
mesh:

| Fixture | What it is | Failure at 90c06b6 |
| --- | --- | --- |
| `brep-hole-outer-bound.step` | a washer whose top face labels its bore `FACE_OUTER_BOUND` | `the outer bound winds the wrong way` |
| `brep-far-generator.step` | a cone in two half faces; one generator `LINE` has its origin 5 km away and passes 10 nm from the apex, under a 1 µm tolerance | `a face loop winds around a periodic surface direction ...` |
| `brep-sampled-peak.step` | a cylinder whose top edge is four Bézier quarter arcs, peaking 2.7253 µm off the side and at most 2.668 µm at the fit's samples, under 2.7 µm | `Boundary(CurveSurfaceMismatch)` |

The tests also cover strict import of each fixture and both reversed variants of the
washer. `outer::tests` and `pcurve::tests` cover the rule, the clamp and the
golden-section search. `check_geometry.py` adds:
* the three fixtures;
* a `relabelled_outer_bounds` column;
* a strict check.

That brings it to 129 reviewed outcomes. `Adaptations::relabelled_outer_bounds` is a new
Rust field. The C ABI is unchanged: `ts_solid_info` and `ts_surface_model_info` keep
their eleven counters.

**Nothing that meshed changes.** Survey builds of 572a847 and of this change were run
on these inputs:
* the 704 files with a meshed shape or surface-model root;
* the 2,740 dodgy-step-files, including their reversed-loop cases;
* the 54 PreForm files.

No root that meshed before changed its vertices, triangles or volume. No
dodgy-step-files root changed at all. Every difference is a rejected root that now
meshes.

| Check | Result |
| --- | --- |
| fmt, Clippy (`-D warnings`, all targets and features), rustdoc (`-D warnings`), release and debug builds | passed |
| `check_kernel.py` (`cargo test --workspace --locked --no-fail-fast`) | passed |
| `check_architecture.py`, `check_corpus.py`, script unit tests, `check_cli.py`, `check_express.py`, `check_schema.py`, `check_product.py` | passed |
| `check_geometry.py` (129 outcomes), `check_metamorphic.py`, `conformance.py --check`, `check_capi.py --library-dir target/release` | passed |
| `corpus.py --check --repeat 2` with the AP validator and schema map, against the saved 1.0.0 baseline | 3,227 compatible, no regressions, 342 s. 18 inputs changed: the 11 of 572a847 and e23c9ed, and the 7 below |

Reviewed movements, isolated against the same corpus run on a 572a847 build. All 7
inputs that differ come from this change:
* **Five shape roots now mesh** (771 → 776 against the 1.0.0 baseline's 762):
  * `nist_stc_06_asme1_ap242-e3` and `nist_stc_07_asme1_ap242-e3`;
  * `sp7_04-dx-242` and `bug32049_sp7_04dx_242`, at 0.017520 m³. Their face #325 had
    the same mislabelled bound. The meshing sibling `sp7_04-do-242` is also
    0.017520 m³ (+0.0028%);
  * `bug22680_C5-390.410-…`, at 1.942e-4 m³. Its planar annulus #881 labels its
    r = 24 circle `FACE_OUTER_BOUND` inside its r = 31.5 `FACE_BOUND`.
* **Two surface-model shells now mesh** (1,128 → 1,130), one each in
  `trj4_k1_top-tc-214` and `trj5_k1-tc-203`. They were `CurveSurfaceMismatch`
  rejections on a surface of revolution, resolved by the peak refinement.

The baseline was not saved.

# HOOPS parity: refinement walks on fillet bands and tangent boundary chords — 2026-10-10

Eight PreForm CAD collection files failed tessellation where HOOPS Exchange meshes a
closed body. In `nist_ftc_06_asme1_rd`/`_ap242-e2` and `nist_ftc_11_asme1_rb`/`_ap242-e2`
(all four also in the corpus's NIST PMI set), one face ran out of the 32 refinement rounds.
The same happened to the three `nist_ctc_02` variants on the assembly path's fixed
0.1 mm chord. In `nist_ctc_04_asme1_ap242-e1`, face #14302 failed the mesh-boundary
resolution check as "tangent edges at a vertex".

**Round exhaustion is a walk, not a cycle.** The faces are torus fillets: FTC 11 face
#175 (R 30 mm, r 1.5 mm, from the outer equator to v = 163.5°) and FTC 06 face #366
(R 0.575 in, r 0.125 in, inner side from 140° to 180°). CTC 02 face #1473 is a rational
degree 3 × 2 B-spline fillet. In the last rounds, one facet per walk front fails, always
at a corner, by 0.1015–0.1202 rad against the 0.1 rad tolerance. Its corners lie on
three successive rows of the band (v = 2.854, 2.765 and 2.676 on FTC 11). Facets spanning
two rows pass, because the normal turns just under the tolerance across them. Splitting
the failing facet's longest edge, across the band, puts a vertex between two rows.
After the flips, the facet beyond it spans three rows and fails in the next round.
So the refinement walked around the ring one facet per round: 0.0245 rad per round on
FTC 11, which would need about 250 rounds for 2π, and two opposite walks of 0.049 rad per
round on FTC 06. No metric or flip cycle repeats a state, and no singularity is
involved: facets spanning one row pass. A face that runs out of rounds is now refined
again from the start with split closure, and keeps closure in later passes. In closure
mode, the pieces of facets that passed and were split for a neighbour are tested at
once within the round, and split again while they fail. Closing every piece also split
the coarse initial triangulation into slivers: FTC 11 face #175 exceeded the 100,000
vertices of `planar.max_vertices` in round 0. Pieces of failing facets therefore still
wait for the next round's flips. Faces that converge within their rounds are never
retried, so their meshes are unchanged. FTC 06 and FTC 11 now mesh in 32,674–35,566
triangles.

**The CTC 04 edges are tangent in the input.** Face #14302 lies on a 5 mm sphere.
Its arc #13914 and B-spline #14293 meet at vertex #13908. Away from the vertex, the
B-spline (352 control points) follows the great circle in y = 337.18 mm, which crosses
the arc at 18.7°. Its last control leg, from (−149.0057, 337.1787) to (−149.0000,
337.1768) mm, lies along the arc's tangent (0.947, −0.321, 0). The last few µm of the
B-spline therefore form a hook that ends tangent to the arc. The trim reconstruction
accepts the loop at the model tolerance. At mesh resolution, though, the B-spline is
sampled at its dense knots beside the vertex, and the arc's first chord (the quarter
arc has 8 samples) crosses segment 186 of the B-spline. Since trimming already validates
the loops, a resolution failure in `tessellate_faces` now requests a boundary refinement:
the longer chord of each crossing pair is split at its edge-parameter midpoint, for up to
8 passes, after which the failure is reported as before. Splitting both chords chased the
pair toward the vertex: over eight passes the arc's request crept from parameter 1.5585 to
1.5700, against the vertex's π/2, without resolving. Splitting only the longer chord meshes the face in one
pass. Edges that actually cross are still rejected by the trim reconstruction.

| File (PreForm collection) | Before | Survey now (mm³) | HOOPS (mm³) | Δ |
| --- | --- | ---: | ---: | ---: |
| `nist_ftc_06_asme1_ap242-e2.stp` | `max_rounds`, face 1 | 3,322,600.7 | 3,323,492.0 | −0.027 % |
| `nist_ftc_06_asme1_rd.stp` | `max_rounds`, face 104 | 3,322,666.3 | 3,323,482.2 | −0.025 % |
| `nist_ftc_11_asme1_ap242-e2.stp` | `max_rounds`, face 4 | 5,128.28 | 5,131.12 | −0.055 % |
| `nist_ftc_11_asme1_rb.stp` | `max_rounds`, face 4 | 5,123.23 | 5,125.29 | −0.040 % |
| `nist_ctc_04_asme1_ap242-e1.stp` | tangent edges, face #14302 | 17,520,897.9 | 17,519,939.2 | +0.005 % |

The CTC 04 siblings mesh at 17,520,405.6 mm³ (AP203) and 17,520,200.5 mm³ (`rd`), so the
spread is 0.004 %. On the assembly path (`assembly --meshes AP242`, 0.1 mm chord), the
`nist_ctc_02` files now place their root. The AP203 file meshes at 47,105,888 mm³ against
HOOPS 47,101,979 (+0.008 %), and `ap242-e2` at 47,110,452 against 47,107,226 (+0.007 %).
`rc` meshes at 47,105,425; its HOOPS mesh is open (384 boundary edges, 48,297,787 mm³).
The five files above place within 0.06 % of HOOPS on that path too. Of the collection's
54 files, 49 now mesh every root in the survey (44 before), and the other 49 survey
results are identical to main's. On the assembly path, only `nist_stc_06` and
`nist_stc_07` still fail to import (UV trimming: wrong orientation; a non-contractible
loop), and the three parser rejections remain.

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | passed (92 test binaries, 353 tests), including `fillet_bands_converge_where_splits_walk_along_them` (both band shapes fail with `max_rounds` without the retry) and `chords_crossing_a_tangent_edge_are_refined` (a planar horn whose arc chord crosses a tangent B-spline meshes; a variant whose spline crosses the arc still fails in trimming) |
| fmt, Clippy (`-D warnings`, all targets and features), `cargo doc` (`-D warnings`) | passed |
| `check_architecture.py`, `check_geometry.py` (125 outcomes), `check_metamorphic.py`, `check_product.py`, `check_corpus.py`, `conformance.py --check`, script unit tests | passed |
| `check_capi.py --library-dir target/release` (no sanitizers) | passed; the C ABI is unchanged |
| `corpus.py --check`, shared AP validator, saved 1.0.0 baseline | 3,227 compatible, no regressions, 270 s; 11 inputs changed (8 below, and the 3 product-stage changes of the keycaps work) |

Reviewed corpus movements, per root, against a run of `90c06b6` (baseline not saved):
9 roots move from `tessellation:invalid_geometry` to accepted and none move the other
way. The 1,456 roots accepted by both runs have identical vertex counts, triangle counts
and volumes. Seven of the nine had exhausted `max_rounds`: the four NIST FTC roots above,
`bug31685_2.stp` #8084 (an FTC 06 copy), `trj6_pm4-ec-203.stp` #803 and `bug33261.stp`
#46406. Two had tangent-edge resolution failures: CTC 04 and `bug33261.stp` #23856
(face #13310). No `max_rounds` failure remains in the corpus.

# HOOPS parity: the Shapr3D keycaps assembly — 2026-10-10

`samples/keycaps.step` of the PreForm CAD collection (a Shapr3D AP242 export written by
HOOPS Exchange 2024.8; 236,704 entities, 265 `MANIFOLD_SOLID_BREP`s) meshed every root
in the survey (42,326 mm³), while the HOOPS probe reports 84,761 mm³, and the assembly
path rejected it with `invalid product graph: MissingLink`.

**The 2× is HOOPS double counting.** The product structure is 14 product definitions and
13 `NEXT_ASSEMBLY_USAGE_OCCURRENCE`s, each with a different child definition (`root` →
`SYMB`, `MAIN`, `FUN`, `DRAFT`; each of the first three → letter, body and stem parts), so
no body is instanced twice; all 13 placements are identity. The 14 `MAPPED_ITEM`s each
sit in a `DRAUGHTING_MODEL` mapping one product's shape representation (presentation, not
an occurrence). Per body (curved import, 0.01 mm chord): all 265 are closed, single
components with positive volume, and no two coincide. Of the 62 keycap shells the 1u
keys measure 18.53 × 18.53 × 5.40 mm at about 552 mm³ (2V/A ≈ 0.97 mm walls), up to a
117 mm spacebar of 3,100.5 mm³; 141 legends are 0.16 mm inlays in the key tops (141.2 mm³), and 62 stems are
16.18 mm³ each. Legends and stems overlap the shells; both readers count overlaps alike.
The bodies total 42,377.1 mm³ at that chord. HOOPS's 84,760.65 mm³ is 2.0001 times this
(half of it, 42,380.3 mm³, is 0.008 % above ours), and its 768,360 triangles are 2 ×
384,180. The probe (`hoops_probe.cpp`) sums untransformed part tessellations while
visiting each occurrence's prototype and its child occurrences, so parts reached both
ways are counted twice; TessSTEP's single count is the instanced volume.

**The MissingLink was ours to accept.** HOOPS Exchange writes every
`ITEM_DEFINED_TRANSFORMATION` with the parent's placement first and the child's frame
(here one identity frame shared by every shape representation in 14 contexts) second,
while the relationship's rep_1 is the child: the reverse of AP242 WR2. The adapter now
reads such a pair by membership when only the exchanged order fits
(`Policy::accept_reversed_transform_items`, enabled by the linker unless strict, counted
as `reversed_transform_items`); strict mode and the default `adapt` report a located
error naming the relationship. The authored `assembly-reversed-items.step` reproduces the
layout (per-product contexts, shared identity frame, B-rep representation behind each
shape representation, draughting-model mapped items) with non-identity placements: its
world boxes equal the conformant nested fixture's, where the literal reading would move a
block by 40 mm. Keycaps now links 14 products and 13 occurrences and imports 265 asset
instances, all placed, at 42,325.7 mm³ (0.1 mm chord) in 1.7 s; the scene's instanced
volume equals the survey's sum, half the HOOPS figure.

Siblings through the assembly path: `Core__2_.step` (Onshape) 5,849.8 mm³ against HOOPS
5,848.7 and `G_pro_side_V2.step` (Autodesk) 1,025.2 against 1,025.4, each one part. Over
the whole collection the assembly path matches HOOPS within 1.1 % wherever it places
instances; the three `nist_ctc_02` files place nothing because their root fails the fixed
0.1 mm assembly chord with an exhausted refinement round budget on one face (the survey's
relative chord meshes them), seven NIST files fail import as in the survey, and three
files are rejected by the parser in both.

| Check | Result |
| --- | --- |
| `cargo test --workspace` (351), fmt, Clippy (`-D warnings`, all targets), rustdoc | passed |
| `check_geometry.py` (125 outcomes, with the new fixture), `check_product.py` (11), `check_architecture.py`, `check_corpus.py`, `conformance.py --check`, script unit tests | passed |
| `corpus.py --check` with the shared AP validator against the saved 1.0.0 baseline | 3,227 compatible, 3 changed, no regressions |

Reviewed movements: three product stages move from `invalid product graph: MissingLink`
to accepted, all through reversed item transformations: `bug33410.step` (HOOPS Exchange
2020.1; 3 definition nodes, 1 placed root) and `trj3_s1-ct-203.stp` / `trj3_s1-ct-214.stp`
(CATIA V4; 17 nodes and 8 placed roots each). Their placements are not identity; read by
membership the CAx-IF S1 spaceship is arranged as in the NX and Pro/ENGINEER exports of
the same model (feet below the body, heads forward, turbines mirrored in y). Accepted
structures now hold 1,454 products, 1,185 occurrences in 103 assemblies and 705 placed
shape roots. The baseline was not saved.

# Milestone 22.11: CI and the 1.0.0 release workflow — 2026-10-10

`main` at 86f2536 was pushed with the maintainer's approval. CI run 38025959160
passed every job: build and test on ubuntu-24.04, windows-2022 and macos-15 (with the
C/C++ sanitizer consumers on Linux and macOS and the distributable package smoke test),
the Rust 1.85 MSRV build, dependency licenses and advisories, and the external corpus
regression against the saved 1.0.0 baseline. The annotated tag `v1.0.0` (86f2536) then
ran the Release workflow (run 38026639055): the tag/version check, the same CI verify,
release-mode tests and packages for x86_64-unknown-linux-gnu, x86_64-pc-windows-msvc,
x86_64-apple-darwin and aarch64-apple-darwin, each installed and consumer-tested
outside the source tree, and a draft GitHub release "TessSTEP v1.0.0" with the four
archives (8.6–10.7 MB, now including the static library) and their SHA-256 files. The
release log shows, for every target, exactly the 95 symbols of `abi/exports.txt`
exported with no Rust exports, and the relocated installed-package consumers passing
in Debug and Release with shared and static linkage. The 1.0.0 draft awaits the
maintainer's publication. The `v0.1.0` pre-release draft (run 36303712652, reviewed on
2026-10-02) was published as a GitHub pre-release on 2026-10-10.

# Milestone 22.4 follow-up: triangulator failures — 2026-10-10

Delivered on 42366af: a hole's bridge leaves the copy of a repeated polygon vertex
whose interior angle faces it; ear clipping that finds no ear is redone once with a
blocking slack of 1e-9 of the boundary extent; and the importer names a triangulator
failure as `unresolved triangulation` at its face, with the mesh-boundary classes
stating that they count at floating-point resolution
([tessellation](TESSELLATION.md), [curved import](CURVED_IMPORT.md#uv-loop-failures)).

Measurement first. The 27 roots of the 1.0.0 baseline whose first failure was
`Polygon(UnresolvedGeometry)` (25 files) were rerun with the triangulator's failure
sites instrumented and every failing face's loops dumped, then replayed in isolation.
Every one of the 56 failing triangulations in those runs had simple, correctly nested
loops at floating-point resolution and ran out of ears in clipping; each face is
classified by the fix that resolves it alone, and 14 dumped faces were replayed:

| Cause | Ours | First failures | Surfaced | Representative faces |
| --- | --- | ---: | ---: | --- |
| A hole bridged to the first of two copies of a vertex that an earlier bridge repeated, although only the other copy's interior angle faces it: the bridged polygon folds there (two NIST slots side by side, both bridged at one corner) and clipping runs out of ears | yes | 23 | | `nist_ctc_04_asme1_rd` #10102 (two slots 2 mm apart, bridges at corner (0.408, 0.0075)), `bug23202_Leuchte` #20206, `sp7_04-do-242` #1345 |
| Clipping across a boundary run straight only to its fitting noise (a fitted circle on a cylinder band, straight to about 1e-10 over 1.4 rad) leaves a remainder within that noise of one line, with no ear at the floating-point tolerance | yes | 4 | 3 | `trj4_pm2-ec-214` #11028 and #29404, `trj6_pm4-ec-203` (same faces), `nist_ctc_02_asme1_rc` #16187, `sp7_04-do-242` #11034, `trj12_b3-tu-203` #23248 |

Both fixes act only where the old triangulator failed: the bridge test only skips a
candidate at a repeated vertex that does not face it, and the slack retry only runs
after a failed clipping. Both survey builds were run on the 704 files with a meshed
shape or surface-model root. Every root that meshed before has the same vertex and
triangle counts and volume. The only other difference is wording: a mesh-boundary
failure in `bug32239.STEP` now says it counts at floating-point resolution.

| Check | Result |
| --- | --- |
| `cargo test --workspace`, fmt, Clippy (`-D warnings`, all targets), rustdoc | passed, including `planar::tests` (both fail without their fix) and `uv_loops::tests` |
| `check_geometry.py` (124 outcomes), `check_metamorphic.py`, `check_architecture.py`, `conformance.py --check` | passed |
| `corpus.py --check --repeat 2` with a rebuilt AP validator, against the 1.0.0 baseline | 3,227 compatible, 21 changed, no regressions |

Reviewed movements, all from this change: 20 shape roots now mesh (742 → 762; 18 files
from tessellation rejected to accepted, and one more root each in `trj4_pm2-ec-214` and
`trj6_pm4-ec-203`); `bug33261.stp` meshes one more surface-model shell (1,127 →
1,128). Volumes agree across the copies of each NIST model (CTC 02 0.0471 m³ in four
files, CTC 04 0.01752 m³ in four, CTC 03 3.32e-4 m³ in two). The other 7 roots now
fail later, all named: `bug24055_8MTF-75LS05` at face #82327 (wrong orientation),
`bug33261` and `nist_ctc_04_asme1_ap242-e1` at the mesh boundary (tangent edges at a
vertex), `trj4_pm2-ec-214` and `trj6_pm4-ec-203` at the mesh boundary (an edge whose
samples double back), and `bug31685_2` and `nist_ftc_06_asme1_ap242-e2` on the
`max_rounds` refinement budget. No shape root fails with `Polygon(UnresolvedGeometry)`
any more. The baseline was not saved.

# Milestone 22 integration and the 1.0.0 corpus baseline — 2026-10-10

Every Milestone 22 item (22.2–22.10 and the 22.2 follow-up) was developed on its own
branch from c80d0ff and integrated on `main` one at a time, with the reviewed corpus
movements of each branch. Integration fixes made while merging: opt-in sewing in the
planar and faceted profiles compacts the vertex entities that locate UV loop failures
(22.4 with 22.5); the curved adapter carries the surface-model shell list through the
session's decode/adapt split (22.6 with 22.8); the tolerance policy is built in that
adapter, and its working tolerance (at least the sewing tolerance) is what the repair
stage validates at (22.3 with 22.5); the tessellation entry points allow Clippy's
large-error lint now that errors carry the located trim failure and the budget (22.2
with 22.4); surface models ignore the opt-in repair stage and validate each shell at
the working tolerance (22.6 with 22.3 and 22.5); and the coverage matrix and a repair
fixture's expected diagnostic were regenerated for the new product profile and loop
messages.

Checks on the integrated `main` (macOS arm64, version 1.0.0), all passed:
`cargo fmt --check`; workspace Clippy with `-D warnings`; `cargo test --workspace
--locked --no-fail-fast`; rustdoc with `-D warnings`; the release build; the 74 script
unit tests; `check_architecture.py` (19 packages); `check_geometry.py
--require-external` (124 reviewed outcomes); `check_coverage.py` (501 AP242 entities);
`check_metamorphic.py` (76 expectations); `check_product.py` (11 outcomes);
`check_corpus.py` (138 authored fixtures); `conformance.py --check`; and
`check_capi.py --library-dir target/release` (exactly the 95 ABI 1.0 exports, shared and
static linkage, Debug and Release consumers). Not observed locally: the sanitizer
consumer run (it hangs in the AddressSanitizer runtime on this macOS release), the
MSRV build, Linux and Windows, and the nightly fuzzing and benchmark jobs. `main` has
not been pushed, so no CI run covers this state yet.

The Onshape export imports as a closed solid: 65,766 vertices, 131,556 triangles,
5.85e-6 m³, 0.8 s in the release survey.

External corpus (`corpus.py --check --repeat 2` with the AP validator and schema map):
3,227 compatible inputs, no crash, timeout or nondeterminism, 353 s. Against the
2026-09-27 baseline, 1,622 inputs changed; 1,530 of them only gained the surface-model
root class. The reviewed totals, saved as the 1.0.0 baseline with `--save-baseline`
(that run matched the reviewed run on every input):

| Measure | 2026-09-27 | 2026-10-10 |
| --- | ---: | ---: |
| Shape roots meshed (of 1,197) | 577 | 742 |
| Shape roots past geometry and topology | 745 | 799 |
| Accepted product structures | 585 | 596 |
| Placed shape roots in accepted structures (unplaced) | 607 (9) | 688 (11) |
| Surface-model roots meshed as shells (of 2,242) | not surveyed | 1,127 |
| `tessellation:resource_limit` roots | 54 | 3 |
| Slowest input | `bug33261.stp`, 13 s | `bug30790.step`, 20 s |

No file lost a meshed root. Stage movements other than the new class: 21 geometry
rejected → accepted, 6 partial → accepted and 2 unsupported → accepted; 60 file-level
tessellation outcomes became accepted (18 from partial, 16 from not run, 14 from
rejected, 12 from resource limit), for 165 more meshed roots in all; 11 product
rejected → accepted; 11 geometry unsupported → rejected (nine solids whose `OUTER` is an
`OPEN_SHELL` or that reach a missing reference, now typed by the surface-model profile,
and two `trj*_b1-ec` roots rejected for a face loop running against its surface); 7
tessellation resource limit → rejected (NIST CTC roots now ending in
`Polygon(UnresolvedGeometry)`, which also fail with a 2·10⁹ budget); 7 tessellation not
run → rejected (roots newly past geometry). One reviewed regression:
`bug32596_abc_0_13.step` geometry accepted → rejected. Discovery now reads its context
units (metres, uncertainty 1e-17 m floored at 1e-7 m) instead of assuming millimetres,
and an edge lies 1.624e-6 m from its surface, just beyond the 16 × 1e-7 m policy bound;
the root never meshed.

Remaining named categories with the most roots: `OpenShell` (95), `BrokenWire` (39),
`InconsistentOrientation` (35) and `UnsewnShell` (29), all strict topology rejections
that the opt-in repair addresses; edges (41) and vertices (10) beyond the deviation
policy. The 27 roots whose face triangulation ended in `Polygon(UnresolvedGeometry)`
in this baseline were resolved by the 22.4 follow-up above (20 mesh, 7 fail later
with named errors); that change is reviewed against this baseline and not yet saved.

# Milestone 22.10 final pass: folded revisions, failure text, ABI 1.0 freeze — 2026-10-10

Delivered on main at c51e109:

- **Folded revisions.** No release shipped a revision of the extensible records
  (v0.1.0 had 64 exports and none of them), so ABI 1.0 publishes exactly one revision
  of each: `ts_import_options` 168 bytes (the second pass's fields, the assembly
  budgets renamed `max_assembly_instances` / `max_assembly_depth`, and the presentation
  limits `max_polyline_points` / `max_presentation_depth`), `ts_solid_info` 216 bytes,
  `ts_surface_model_info` 136 bytes. The draft sizes (96 and 152 bytes, 104 bytes) are
  rejected; the revision mechanism stays for 1.x.
- **Session presentation and surface-model import**:
  `ts_import_session_import_presentation` (the Rust session's shared tessellated
  decode) and `ts_import_session_import_surface_model` (the free import, without
  sharing; the Rust session has no surface models).
- **Failure text without global state.** The header promises "No callbacks, global
  error state", so instead of a per-thread last diagnostic the text is handle state:
  `ts_import_session_last_message` and the per-root reports
  `ts_assembly_root_report_at` / `ts_discovery_root_report_at` (which also carry the
  22.7 tolerated encodings of each root's context). C++ `Error::message`,
  `AssemblyRoot::{message, tolerated}`, `DiscoveredRoot::tolerated`.
- **Freeze.** `abi/exports.txt` records the 95 functions of ABI 1.0;
  `tessstep-capi/tests/abi.rs` checks exports, header and record and pins the 1.0
  set; `check_capi.py` checks the installed shared library and static archive against
  the record; the release archive ships it. Policy in C_API.md.

| Check | Result |
| --- | --- |
| `cargo test --workspace`, fmt, Clippy (`-D warnings`, all targets), `cargo doc -D warnings` | passed (345 tests), including the new `--test abi`, `c_abi_session_keeps_the_last_failure_message`, `c_abi_session_imports_presentations_and_surface_models` and `c_abi_root_reports_carry_tolerated_encodings_and_failure_text` |
| `check_capi.py --library-dir target/release` (no sanitizers; local ASan hang) | passed: shared consumers in Debug, Release and C-only, exactly the 95 recorded exports, static C and C++ consumers against a package copy without the shared library, the archive defining exactly the 95 symbols |
| `package_release.py --target aarch64-apple-darwin` | passed; the archive ships `abi/exports.txt` |
| `check_architecture.py`, `conformance.py --check`, script unit tests (74) | passed |

No `tessstep-import` code changed in this pass, so the external corpus was not rerun.
Linux and Windows results are CI-only.

# Milestone 22.10 second pass: repair, tolerance policy, surface models, sessions — 2026-10-09

Delivered on top of 22.2–22.8 (main at 5a9879a): revision 2 of `ts_import_options`
(152 bytes: sewing and deviation factors, assembly expansion and session budgets;
`TS_IMPORT_REPAIR_SEW`, `TS_IMPORT_REPAIR_ORIENT` and `TS_DISCOVER_SURFACE_MODELS`
flags) and of `ts_solid_info` (216 bytes: the 22.3 counters, largest deviation, working
tolerance and the 22.5 repair report); surface-model import with a retained
`ts_surface_model` and surface-model roots in discovery on request; the
`ts_import_session` handle; `ts_document_import_assembly_with_options` and
`ts_assembly_get_adaptations`; the matching C++ API. Sixteen new functions: 90 C
exports. No `tessstep-import` code changed, so the external corpus was not rerun.

| Check | Result |
| --- | --- |
| `cargo test --workspace`, fmt, Clippy (`-D warnings`, all targets), `cargo doc -D warnings` | passed (339 tests), including six new `tessstep-capi --test session` contract tests and `default_options_are_the_rust_defaults` |
| `check_capi.py --library-dir target/release` (no sanitizers; local ASan hang) | passed: Debug, Release and C-only shared consumers, exactly 90 exports; Release C and C++ consumers linked statically against a package copy without the shared library; the archive defines exactly the 90 C symbols |
| `package_release.py --target aarch64-apple-darwin` | passed (9.6 MB archive) |
| `check_architecture.py`, `conformance.py --check`, script unit tests (74) | passed |

Defaults after 22.2: the initialized options still map to exactly
`TessellationOptions::default()` (planar work 10M, trim samples raised to 1M by 22.4,
sampling 1M samples and 10M evaluations), `ImportOptions::default()` (deviation factor
16, repair off) and `AssemblyImportOptions::default()`; the unit test compares them
field by field. Writing it found one difference, now fixed: the frozen
`ts_assembly_options` copies its instance budget into the scene limit (100,000), while
Rust keeps 1,000,000 there; the new assembly entry point leaves the scene limits at
the Rust defaults. Apple `nm` cannot parse the newer-LLVM bitcode embedded in the Rust
standard library's archive members; the static symbol check ignores those members'
errors and still requires exactly the 90 `ts_` symbols. Linux and Windows results are
CI-only.

# Milestone 22.2 follow-up: evaluation reuse and the `bug30790.step` deadline — 2026-10-10

On `5a9879a`, `bug30790.step` (OCCT dataset, 26 `MANIFOLD_SOLID_BREP` roots) took
124 s in the survey and timed out in the corpus: each of its 24 B-spline solids (77
or 86 rational B-spline surfaces each) exhausted the 10M `max_evaluations` after 5.2 s.
Diagnosis with a scratch driver: the meshes are genuinely large. Root #7174 has
217,116 triangles at the survey tolerance (chord 7.7 µm, 0.1 rad); its heaviest
faces are 0.7 mm blends with radii near 0.35 mm, where the angle tolerance needs
edges near 31 µm (3,400 triangles, about 1.45 times the equilateral count). Given
10–15M evaluations `5a9879a` meshes it identically in 7.4 s. The excess was repeated
work: probes evaluated corners and shared edge midpoints again, each boundary pass
remeshed a changed face from scratch and probed its passed facets again, each round
rebuilt the edge incidence, and the importers validated each mesh twice (once more
to replace face labels).

Delivered: probe reuse of vertex and shared midpoint evaluations; per-face memory of
evaluations and passed facets across boundary passes; index-stable triangles with
maintained edge incidence and a flip queue limited to the edges a split or flip can
have changed; `Mesh::relabel_faces`; opt-in `TessellationOptions::threads` with
thread-independent results (the survey uses up to eight threads; C keeps one); a
B-spline basis recurrence on two alternating buffers and one finiteness check per
surface evaluation (both bit-identical). Budget defaults are unchanged and match the
C defaults (`max_evaluations` 10M, `max_tessellation_work` 50M).

| Check | Result |
| --- | --- |
| `survey bug30790.step 0.001` | all 26 roots accepted (profile, geometry, tessellation), 18.8–19.4 s with eight threads; 82.7 s on one thread with the scratch driver (80 s tessellating; 124 s and 24 `resource_limit` roots before) |
| Budgets of `bug30790.step` roots | #7174 (217,116 triangles): 5–6M evaluations (10–15M before), 30–40M work; #7182 (250,686 triangles): 6–7M evaluations, 35–40M work |
| Onshape export | identical mesh (65,766 vertices, 131,556 triangles, 5.8498449326681915e-6 m³); 2.5–2.6M evaluations (7.65M before) and 15–17.5M work (27.8M); 1.91 s on one thread, 0.47 s on eight |
| Identity | every root of `bug30790.step` and the Onshape export: the same vertex and triangle counts and volumes after each change and on one or eight threads; `5a9879a` with raised budgets gives the same mesh for root #7174 and the Onshape export |
| `cargo test -p tessstep-tessellate --test anisotropic` | passed, including `boundary_passes_reuse_evaluations_and_threads_change_nothing` (the torus band within 75,000 evaluations, about 109,000 needed before; one and four threads give equal meshes and equal budget failures) |
| `cargo test -p tessstep-curves --test nurbs` | passed, including `spline_basis_matches_the_full_recurrence_bit_for_bit` (degrees 1–7 and 16, repeated knots, both sides) |
| `cargo test --workspace --locked` | passed (90 test binaries, 334 tests) |
| fmt, Clippy (`-D warnings`, all targets and features), `cargo doc` (`-D warnings`) | passed |
| `check_architecture.py`, `check_geometry.py`, `check_metamorphic.py`, `conformance.py --check` | passed (124 reviewed geometry outcomes, 76 generated expectations) |
| `check_capi.py --library-dir target/release` (no sanitizers) | passed |
| `corpus.py --check` with a freshly built `target/release/ap-validator` and `corpus/ap-schema-map.json` | 3,227 inputs, no crash or timeout, 310 s (479 s for the `5a9879a` run); `bug30790.step` 20.1 s (121.8 s timeout before); one baseline regression, present on `5a9879a` too (below) |

**Reviewed corpus movements** (per root, against the `5a9879a` run of 2026-10-09;
baseline not saved): no root lost its outcome; accepted roots 706 → 728.
`bug30790.step` moved from `timeout` to 26 accepted solids (the report lists 20 roots);
its one `SHELL_BASED_SURFACE_MODEL` (#7166, a declared `CLOSED_SHELL` with a single
open face) now reaches the surface-model stage and is rejected as `OpenShell`, as
22.6 specifies. `trj6_k1-al-214.stp` #830 and `bug33261.stp` #46035 newly mesh (both
exhausted `max_evaluations`); no `max_evaluations` failure remains. The remaining
tessellation budget failures are unchanged: five `max_rounds`, two `max_work`
(`bug24593_281946.stp` #30, now at face 14 rather than 11, and
`bug26889_export.step` #34) and one `planar.max_work` (`bug24024_slow_import.stp`
#76237). The single regression, `bug32596_abc_0_13.step` (geometry accepted →
rejected: `EDGE_CURVE` #2776 lies 1.624e-6 m off its surface, beyond the 1.600e-6 m
the 22.3 policy accepts), is identical in the `5a9879a` run: the reviewed baseline
predates 22.3, and its 1,622 other changed inputs are mostly the `surface_model` stage
it does not record. The slowest inputs are now `bug30790.step` (20.1 s),
`OCC5157.stp` (16.8 s, 23.3 s before) and `bug31711.stp` (9.8 s, 13.2 s before).

# Milestone 22.10 first pass: discovery, solid import, import options, static linkage — 2026-10-02

Delivered: the extensible `ts_import_options` (revision 1, 96 bytes) with
`ts_import_options_init` and `ts_import_options_for_document`;
`ts_document_import_solid` (curved or faceted profile, explicit length and plane-angle
units, extensible 104-byte `ts_solid_info`); `ts_document_discover` and the retained
`ts_discovery` with root, presentation and choice queries; their C++ RAII wrappers;
and the `TessSTEP::TessSTEP_static` package target. Ten new functions: 74 C exports.
Import behaviour is unchanged (no `tessstep-import` code changed), so the external
corpus was not rerun.

| Check | Result |
| --- | --- |
| `cargo test --workspace`, fmt, Clippy (`-D warnings`, all targets), `cargo doc -D warnings` | passed (273 tests), including the six new `tessstep-capi --test discovery` contract tests |
| `check_capi.py --library-dir target/release` (no sanitizers; local ASan hang) | passed: Debug, Release and C-only shared consumers, exactly 74 exports, and Release C and C++ consumers linked with `TessSTEP::TessSTEP_static` against a package copy without the shared library; the archive defines exactly the 74 C symbols |
| `check_capi.py` building the package from source through Cargo | passed |
| `package_release.py --target aarch64-apple-darwin` | passed; the archive ships `libtessstep_capi.a` (23 MB) beside the 2.5 MB dylib, 8.9 MB compressed |
| `check_architecture.py`, `conformance.py --check`, script unit tests (45) | passed |

Static link dependencies come from `rustc --print native-static-libs` at configure
time: `c;m` here (libSystem omitted, the Apple driver links it). The Linux and Windows
lists, the Windows static consumers and the sanitizer runs are CI-only; this
validation is macOS arm64.

# Milestone 22.9: extended fuzzing — 2026-10-02

Delivered: a nightly matrix that builds the fifteen `fuzz/Cargo.toml` targets once with
AddressSanitizer, coverage and debug assertions and fuzzes each for a fixed budget
(240 s, `workflow_dispatch` adjustable) with per-target limits; deterministic seed
assembly (`scripts/fuzz_seeds.py`); minimization and replay of every failing input, a
per-target and aggregated report that marks instrumented runs and stable driver smoke
(`scripts/fuzz_report.py`); and committed findings under `fuzz/regressions/<target>/`
replayed on stable by `cargo test -p tessstep --test fuzz_regressions`. It replaces the
14 sequential 120 s sessions, which did not run `brep_import` and gave the five binary
harnesses no seeds. The six targets whose bodies were inline (`part21_lexer`,
`part21_parser`, `product_adapter`, `tessellated_import`, `presentation_import`,
`brep_import`) now share them through `fuzz/support/`, so every target body is compiled
and linted by the workspace build.

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked`, fmt, Clippy (`-D warnings`, all targets) | passed (270 tests), including `every_fuzz_target_has_a_stable_replay`, `committed_fuzz_findings_replay_without_failure` (one finding) and `regression_replay_names_every_failing_finding_and_unknown_target` |
| Script unit tests (67; 22 new in `test_fuzz.py`) | passed; the PyYAML structure check is skipped without PyYAML, and the workflow was parsed with Ruby's YAML loader instead |
| `check_architecture.py`, `conformance.py --check`, `check_corpus.py` | passed |
| Fuzz package in an unnested copy: `cargo check --locked`, Clippy `-D warnings`, fmt | passed after refreshing `fuzz/Cargo.lock`, which lacked the `tessstep-ap242` and `tessstep-product` edges of `tessstep-import`, so `--locked` failed at the previous commit; the nightly build now requires a current lockfile |
| `fuzz_report.py smoke` | 14 of 14 stable smoke tests passed |
| `fuzz_report.py run` with all fifteen stable-built drivers (2 s each) | every report `instrumented: false`; 14 clean, `brep_import` one finding (below), clean after the fix with all 61 geometry seeds |
| Crash, minimization and replay with real libFuzzer (a throwaway stable-built probe target) | a 42-byte crash was minimized to 4 bytes and reproduced; the report named the panic location and message |

Observed with libfuzzer-sys 0.4.13: `-minimize_crash` combined with `-max_len` aborts on
libFuzzer's `MaxInputLen == 0` assertion and writes an empty "minimized" file, so
minimization and replay run without `-max_len` (as `cargo fuzz tmin` does), and a
minimized input that does not fail again is discarded. A stable-built driver logs no
`inline 8-bit counters` line and no `cov:`/`ft:` fields, and its binary contains neither
`__asan_init` nor the ASan runtime's `heap-buffer-overflow` string. The report needs
both kinds of evidence to mark a run `instrumented: true`.

First finding, from stable driver smoke rather than an instrumented run: the
`brep_import` target asserted `require_solid()`, which demands one mesh component, but a
`BREP_WITH_VOIDS` cavity meshes as an inward component
(`brep_cavity_shells_mesh_as_inward_components`), so the authored
`brep-sphere-cavity.step` seed failed it. The oracle now requires a watertight mesh with
positive volume and one component per shell. The input is committed as
`fuzz/regressions/brep_import/sphere-cavity-second-component.step`; the replay test
failed on it, naming the file, before the fix, and passes after it. No library code
changed.

Not observed here: the instrumented nightly matrix (no nightly toolchain or cargo-fuzz
locally), so no coverage, feature or execution figures are claimed yet. The MSRV 1.85
compile of the bodies now in `fuzz/support/` was not run locally.

# Milestone 22.6: surface models as shell meshes — 2026-10-09

Delivered: `discover_surface_models` and `import_surface_model` import every
`SHELL_BASED_SURFACE_MODEL` with the curved profile's face and edge adapters (the B-rep
profile gains `OPEN_SHELL`, `ORIENTED_OPEN_SHELL`, the `shell` select and the root).
Each declared shell is validated open or closed as declared, without a solid, kept as
its own B-rep and meshed through `tessellate_shell`: one connected component, watertight
when closed, never checked for volume or orientation. The survey reports surface models
as a separate `surface_models` class; the runner adds `surface_model_result`, a
`surface_model` stage and the `surface_model_accepted_roots` /
`surface_model_closure_types` baseline fields. A reference or SELECT rejection now names
the attribute and the referenced record (`OUTER references OPEN_SHELL #16`).

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` (274 tests), fmt, Clippy (`-D warnings`, all targets), `cargo doc` (`-D warnings`) | passed, including the seven `tests/surface_model.rs` tests |
| `check_geometry.py --require-external` | 92 reviewed outcomes passed: five authored surface-model roots, the survey class over both fixtures, and two hash-pinned exporter files |
| `check_coverage.py`, `conformance.py --check`, `check_architecture.py`, `check_corpus.py`, `check_metamorphic.py`, script unit tests (49) | passed; 501 AP242 entities classified (71 adapted: `SHELL_BASED_SURFACE_MODEL`, `OPEN_SHELL`, `ORIENTED_OPEN_SHELL` added) |
| `corpus.py --check --repeat 2` with the AP validator | 3,227 compatible, 1,539 changed, no regressions; 508 s (8 min 31 s wall) on a shared machine, 702 s when rerun beside the workspace tests |

Authored fixtures (`corpus/geometry/surface_fixtures.py`): a half-cylinder trough between
a planar and a bilinear B-spline flange as one `OPEN_SHELL` (three faces, eight boundary
edges, area `200π + 200` mm² within the chord bound) and through
`ORIENTED_OPEN_SHELL(.F.)` (identical positions, every triangle and normal reversed); a
model with a closed cylinder shell (`2000π` mm³) and an inward
`ORIENTED_CLOSED_SHELL(.F.)` box (signed volume `-1000` mm³, accepted as a shell); a
declared `CLOSED_SHELL` whose faces are open (`OpenShell`, topology) and a `VERTEX_SHELL`
(named, profile).

Exporter evidence, unmodified and hash-pinned in `corpus/geometry/external-surface.json`:
`trj5_k1-ct-214.stp` (CATIA V5 R5 surface export) is one surface model of 58
single-face open shells on planes, cylinders, tori, spheres, cones and extrusions, with
294 open boundary edges; all 58 shells mesh as single open components.
`trj7_k1-pe-214.stp` (Pro/ENGINEER 2001) is one surface model holding one
`CLOSED_SHELL` of 52 faces in an inch and degree context; it meshes watertight with
positive signed volume (8.54e-6 m³ at a 1 mm chord and 0.5 rad) and is reported as a
shell.

Corpus class (surveyed in context units with the shape-root tolerance and chord rules):
2,242 roots in 1,531 physically accepted files; 1,092 pass geometry/topology and 893
produce shell meshes. By source: OpenCASCADE 519 of 780 roots (106 files), NIST 29 of
31 (11 files), `dodgy-step-files` 345 of 1,431 (1,414 files). Shells: 1,285 open
(1,093 meshed) and 13 closed (5 meshed watertight), 2,507 faces, 5,962 open boundary
edges. File stage: 400 accepted, 24 partial, 944 rejected, 154 unsupported, 9 resource
limit. First failing stage and kind: 475 geometry, 323 topology, 180 profile invalid,
168 tessellation, 111 profile unsupported, 58 geometry unsupported, 31 tessellation
resource limit, 3 missing entities. The roadmap's 4,127 roots count paths, including
duplicates and the 1,756 roots of `bug26451_Test_STEP.stp`, which the physical parser
rejects (`entity zero is invalid`). The exporter failures are dominated by edges lying
off their face surfaces beyond the model tolerance (148 OpenCASCADE roots; Milestone
22.3) and tessellation refinement (`UnresolvedTolerance`, trim limits, self-intersecting
UV polygons, resource limits; Milestones 22.2 and 22.4); the `dodgy-step-files` add
broken wires, disconnected and open shells, and invalid references (22.5 and named
rejections).

Reviewed baseline movements: 1,530 files changed only by gaining the class
(`surface_model` stage, accepted-root count and closure types). Nine files changed a
solid stage, none a regression: eight `MANIFOLD_SOLID_BREP` roots whose `OUTER`
references an `OPEN_SHELL` (`bug25169_box-openshell.stp` and seven
`dodgy-step-files` inputs) move from `profile:unsupported` (`OPEN_SHELL` was outside the
profile) to a typed `profile:invalid_geometry` rejection naming the open shell, because
an open shell cannot bound a solid; `bug33665.step` moves from `profile:unsupported` to
`profile:missing_entity`, its closure now decoding past the open shell to a missing
face reference. The solid class is otherwise unchanged (1,197 roots, 745 passing
geometry/topology, 577 meshed). The baseline was not saved.

Runtime: surface models add about 116 s of survey time over the 1,675 files that hold
them (their solid roots take 31 s), measured with the release survey. The slowest
inputs of the first run were `bug33261.stp` (15.1 s), `trj6_k1-td-214.stp` (14.4 s),
`bug31711.stp` (14.4 s) and `trj3_s1-id-203.stp` (13.7 s); `trj6_k1-td-214.stp` (59
surface models, four ending in `UnresolvedTolerance`) and `trj5_k1-pe-203.stp` /
`-214.stp` (a tessellation resource limit after about 9 s of refinement) are new among
the slowest, all far below the 120 s per-input deadline.

# Milestone 22.7: mapped-item placements and tolerated unit encodings — 2026-10-09

Delivered: occurrences placed by MAPPED_ITEMs of their own shape representations
(`Model::expand`, linker decoding of those items), `CARTESIAN_TRANSFORMATION_OPERATOR_3D`
placements (relationship operator or mapping target, reflection and uniform scale), and
counted tolerances of exporter unit encodings in the context and product profiles
(`ToleratedEncodings`; decoder `Policy::accept_untyped_select` and
`decode_reachable_profile_policy`). Strict mode rejects every tolerated encoding.

Measurement first: none of the 9 unplaced roots in accepted structures is placed by a
mapped item or operator. `A116` holds a five-parameter `PRODUCT_DEFINITION` (stays
rejected); `Pmi049` has a `TESSELLATED_SHELL` that no representation holds; `bug29241`
relates its B-rep to the part's shape representation only by a
`MECHANICAL_DESIGN_AND_DRAUGHTING_RELATIONSHIP`; the two NIST FTC 08 copies hold a
supplemental `TESSELLATED_SHELL` in a constructive geometry representation; `trj2_f1` and
`trj3_t2` hold two `ROUND_HOLE` feature tool bodies each. Mapped-item assemblies were
rejected outright with `MissingPlacement`: `trj3_s1-db-214.stp` (CATIA V4 via debis,
10 occurrences) and `bug33261.stp` (Creo, 47). The real-exporter unit rejections were
explicit SI dimensions (PSStep, older OpenCASCADE, an unnamed exporter; 4 files), the same
with unordered unit components (XStep; 3 files) and untyped uncertainty values (debis;
2 files). No other product result gained mapped placements: the mapped items of the
other corpus assemblies (for example the draughting-model views of `trj4_s1-tu-214`) are
outside occurrence shape representations and stay opaque links.

| Check | Result |
| --- | --- |
| `cargo test --workspace`, fmt, Clippy (`-D warnings`, all targets), rustdoc | passed; new evidence in `tests/assembly.rs` (four tests), `tessstep-product` and decoder tests |
| `check_geometry.py`, `check_product.py`, `check_architecture.py`, `conformance.py --check`, script unit tests | passed; 87 reviewed geometry outcomes including four new authored assemblies |
| AP214 validator on the new fixtures | mapped and operator fixtures accepted (confirms the 8-parameter operator layout); the two unit fixtures rejected, as the encodings are nonconformant |
| `corpus.py --check` with a validator rebuilt from this branch | 3,226 compatible, 15 changed, 1 reviewed regression (below) on 2026-10-02 and again with `--timeout 300` on 2026-10-09; with the default 120 s deadline the 2026-10-09 run under load also timed out `bug30790.step` (below) |

Reviewed movements against the baseline: 11 product stages move from rejected to
accepted (`trj3_s1-db-214` with 10 and `bug33261` with 47 mapped placements, all 9 and
24 discovered roots placed; `2.stp`, `OCC2368`, `bug28414` (explicit SI dimensions and
unordered components); `bug24601`, `bug25013`, `bug30790`, `bug32596` (explicit SI
dimensions); `trj8_c2-db-214` and `trj8_k1-db-214` (untyped uncertainty)). Five accepted
structures only gain a `tolerated` count of starred conversion-unit dimensions (three
NIST AP203 files, `bug28345_30338`, `bug33569`). Accepted structures: 585 → 596; placed
shape roots 607 → 688; unplaced 9 → 11, the two new ones `bug24601`'s solids, whose
representations reach the part only through untransformed relationships across
contexts (unchanged rule). With their declared units, 44 roots in seven files that
discovery had left on assumed millimetres now use their context units: meshed roots
577 → 580 (`2.stp` 11 → 12, `bug30790` 0 → 2), and `bug30790`'s other 24 roots,
centimetre-sized parts declared in metres, now pass geometry (stage partial → accepted)
and stop at a tessellation face budget (`tessellation:resource_limit` 78 roots).

Run time: those 24 budget failures cost several seconds each, so `bug30790.step`
(198,667 entities) becomes the slowest corpus input: 164 s in the 2026-10-09 run under a
load average of about 15 from parallel builds (`bug33261.stp`, 13 s unloaded, took 21 s
there), above the runner's 120 s deadline, while the 2026-10-02 run finished it within
the deadline. Budget behaviour on such faces is Milestone 22.2; until then the deadline
is marginal for this file on loaded or slower machines.

Regression, justified: `bug32596_abc_0_13.step` geometry accepted → rejected. The file
declares a conversion-based `METRE` and a 1e-17 m uncertainty (floored to 1e-7 m).
Before, its unit failed discovery and the survey assumed millimetres, so the 1e-7 m
tolerance was a thousand times looser relative to the geometry; in its declared unit an
edge lies 4.2e-7 m off its face surface and the root is rejected with that distance. It
never meshed (tessellation was rejected before), so no meshed root is lost; the edge
distance policy is Milestone 22.3. The baseline was not saved.

# Milestone 22.4: UV loop validity — 2026-10-09

Delivered: located trim and boundary failures (`trim::Error::{coedge, other, wire,
budget}`, `BoundaryError::Resolution`), mesh-boundary checks at floating-point
resolution, overlap-only polygon work, trim sampling and wobble rules, `.U.` closure
checks, turning-point seam crossings and sphere re-charts through a loop; the
importer names each loop failure's `EDGE_CURVE`(s), vertex or loop and a cause class
([curved import](CURVED_IMPORT.md#uv-loop-failures)).

Measurement first. Every root of the 2026-09-27 baseline whose first failure was an
unlocated polygon error after topology passed (92 roots in 44 files) was rerun with
a probe that reconstructed its failing face's trim at larger budgets and finer UV
tolerances, revalidated its first-pass mesh boundary in the unscaled and the metric
chart, and printed the conflicting samples. Classes, by baseline face and by faces
that surfaced behind the first fixes:

| Class (cause) | Ours | Baseline | Surfaced | Representative faces | Outcome |
| --- | --- | ---: | ---: | --- | --- |
| Mesh-boundary check used the 1e-5 UV chord tolerance as a distance (10 µm in metre plane charts): edges and sample spacings under it flagged | yes | 55 | | `OCC5099` #4207 (34 `SelfIntersection`, 21 `DegenerateLoop`; `bug24383` 15, `bug30790` 14, `trj*` 14) | fixed: checked at floating-point resolution, located |
| Polygon checks charged all n² segment pairs (5M work): 4,096-sample loops, 32 holes | yes | 21 | | `as1_motor` #145, `nist_ctc_04_asme1_rd` #10102 | fixed: box sweep |
| Trim `max_samples` (100,000) below a plane with 1,116 holes | yes | 1 | 1 | `bug24024_slow_import` #69299, `as1_motor` #5376 | fixed: default 1M (the cap) |
| Knot seed a few ulps from an edge end: zero-length trim segment | yes | 1 | | `bug23202_Leuchte` #472 | fixed: noise seeds dropped |
| Inserted seam crossed by a B-spline dipping 0.7 µm across it between scan samples (seam choice) | yes | 1 | | `bug25823_280612` #116 | fixed: turning-point scan |
| Fitted-pcurve hook or sub-nanometre span at an edge end, within the model tolerance (pcurve fit) | yes | 1 | 2 | `bug27693_cornetta` #140, `trj4_pm2-ec` #21161 | fixed: wobble rule |
| Closed B-spline with `U_CLOSED .U.` in a non-periodic chart: closing-line edges took the other preimage | yes | 1 | | `trj3_s1-al` #1306 | fixed: `.U.` checked |
| Sub-micrometre loops sampled at a 10 µm UV chord: fewer than 3 points | yes | | 12 | `bug30790` #6143 | fixed: quarter-chord rule |
| Sphere face outside its only loop (a cap cut away): clockwise in every frame tried | yes | 1 | | `nist_ftc_06_asme1_rd` #304 | fixed: re-chart through the loop |
| `FACE_OUTER_BOUND` on a hole loop, or a reversed loop | input | 10 | | `nist_stc_06` #412, `sp7_04-dx-242` #325, dodgy `M008` #5850 ×7 | named: wrong orientation |
| Loop passes through a vertex twice (a sub-loop hanging at a boundary vertex) | input | | 2 | `OCC5099` #7746 (VERTEX_POINT #4280), #18012 | named: self-touching loop |
| Adjacent edges leave a vertex tangentially, 22 nm apart at 0.4 µm under a 0.1 µm tolerance; trim chords cross | input (cusp) | | 1 | `bug32596_abc_0_13` #145 | named: tangent edges at a vertex |

No class was a collapsed-edge (pole) failure. The cusp would need trimming refinement
on conflicting segments, which is not implemented; the input is ambiguous at its model
tolerance there.

| Check | Result |
| --- | --- |
| `cargo test --workspace`, fmt, Clippy (`-D warnings`, all targets), rustdoc | passed |
| `check_geometry.py` (3 new authored fixtures), `check_metamorphic.py`, `check_architecture.py`, `conformance.py --check`, script unit tests (45) | passed (86 reviewed geometry outcomes) |
| `corpus.py --check --repeat 2` with the AP validator | 3,227 compatible, 13 changed, no regressions |

Reviewed movements (13 changed files, roots counted across them): 30 more roots mesh
(577 → 607: `as1_motor` 6 → 16, `bug24383_224013` 0 → 15, `bug30790` 0 → 2, `pr1sy`,
`pr_3-sy` and `trj3_s1-ac` one each); every new mesh is one closed component with
positive volume, filling 15–98 % of its bounding box. `trj3_s1-al` passes geometry for
2 more roots (745 → 747; they fail at tessellation). Of the 94 roots that failed with
or behind an unlocated polygon error, 30 mesh, 13 fail with a named loop error (the
input classes above) and 51 fail later in tessellation: 24 `ResourceLimit`, 13
`UnresolvedTolerance`, 6 `Polygon(ResourceLimit)` (Milestone 22.2) and 8
`Polygon(UnresolvedGeometry)` (the ear-clipping triangulator cannot resolve a
boundary that is simple at floating-point resolution; `bug24593_281946` #50,
`nist_ctc_04_asme1_ap203` #1250). No root lost its mesh or an accepted stage, and no
category reads `coedge: None` any more. The baseline was not refreshed.

# Milestone 22.5: opt-in sewing and orientation repair — 2026-10-09

Delivered: the `UnsewnShell` topology defect for disconnected closed shells; the repair
stage in `tessstep_topology` (`plan_sewing`, `SewPlan::apply`, `orient_faces`,
`diagnose_gap`); `ImportOptions::repair` (`RepairOptions { sew, orient, sewing_factor }`,
off by default) in the curved, planar and faceted importers, with loop reversal and
outward shell orientation after validation; `ImportedSolid::repairs()`
(`RepairReport`); `brep --repair`, `survey --repair` and `corpus.py --geometry-repair`;
ten generated fixtures (`corpus/geometry/repair_fixtures.py`). See
[TOPOLOGY.md](TOPOLOGY.md#opt-in-repair) and
[CURVED_IMPORT.md](CURVED_IMPORT.md#opt-in-topology-repair).

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | 280 passed, including `tests/repair.rs` in `tessstep-topology` (6) and `tessstep-import` (7) |
| fmt, Clippy (`-D warnings`, all targets and features), `cargo doc` (`-D warnings`) | passed |
| `check_geometry.py --require-external` | 104 reviewed outcomes passed: the ten repair fixtures strictly (each rejected, with its strict diagnostic) and with `--repair` (counts, stages, volumes, gap diagnostics), the inverted-cavity fixture repaired |
| `check_metamorphic.py`, `check_architecture.py`, `conformance.py --check`, script unit tests (48) | passed |
| `check_capi.py --library-dir target/release` | passed (the C imports keep repair off; no ABI change) |
| `corpus.py --check` with the AP validator (strict) | 3,227 compatible, no baseline change; 577 meshed, 745 passing geometry/topology, as in the baseline |
| `corpus.py --check --geometry-repair` (separate output) | 3,227 compatible, 60 changed inputs, no regression; 642 meshed, 800 passing geometry/topology |

Strict classification: the 123 `OpenShell` roots of the baseline now split into 94
`OpenShell` and 29 `UnsewnShell` (all in `dodgy-step-files`: the generator's cubes whose
six faces own their vertices and edges, Pf008/Pf009/Pf017/Pf033 coplanar face grids,
Tsh066, Tfa024/100/245, A102, P023). No other message, stage or count moved; the
baseline stores no messages, so `--check` reports no change.

Repair-on survey by strict failure class (roots in per-file details; the meshed totals
come from per-file counts):

| Source | Strict outcome | Roots | Repair-on outcome |
| --- | --- | ---: | --- |
| dodgy-step-files | `InconsistentOrientation` | 29 | meshed |
| | | 2 | `DisconnectedShell` (two cubes in one shell) |
| | | 2 | polygon `SelfIntersection` at tessellation |
| | | 1 | `OpenShell` with its gap distance |
| | `UnsewnShell` | 18 | meshed (sewn: 16 vertices and 12 edges per cube) |
| | | 8 | `OpenShell` with the gap distance (sewn face grids that are open sheets) |
| | | 3 | `UnsewnShell` with the gap distance (faces apart beyond the tolerance) |
| | `OpenShell` | 5 | meshed |
| | trim `WrongOrientation` | 7 | meshed (loops reversed) |
| | `BrokenWire` | 1 | `UnsewnShell` with its gap distance |
| opencascade-dataset | `CavityOrientation` | 4 | meshed: `brep_wv1.stp` #935, `tr8_bv1_db.stp` #935, `tr8_bv1_sy.stp` #486, `tr8_bv1_tc.stp` #461 (two cavity shells each reversed) |
| | `NonPositiveVolume` | 1 | meshed: `PRO7071.stp` #213 (inside-out shell reversed) |
| | `InconsistentOrientation` | 1 | meshed: `ec_exhaust-A.stp` #2632 (one face whose loop ran against its surface) |
| nist-pmi | trim `WrongOrientation` | 2 | still rejected: `nist_ftc_06_asme1_rd.stp` #101 reverses two faces and then fails `UnresolvedTolerance` on another (22.2); `nist_stc_06_asme1_ap242-e3.stp` #4078 reverses two faces, and face #7's reversal is not accepted by the trimmer |

Meshed roots: +59 in `dodgy-step-files`, +6 in `opencascade-dataset`, none in
`nist-pmi`, `stepcode` or `foxtrot`; no root lost. The remaining 37 `BrokenWire` roots
are not duplicates (their breaks are 0.12 mm to 1 m apart, typically a reversed edge in
a loop) and now name that distance; the remaining open shells are genuinely open
(missing faces, open sheets, `bug28414_bug_toroidal_surface.stp` #472, a single torus
face whose free edges are 9.5 mm apart) or hold coincident free edges that cannot be
paired (three or more, or within one face). `Tsh005.stp`, a cube with one vertex
written 1.1 µm off, is rejected with `needs 1.100e-6 m; sewing tolerance 1.000e-6 m`. Repair totals: 288 merged vertices and 288
merged edges, all at distance zero (18 disconnected shells), 83 flipped faces, 243
faces with reversed loops, 57 reoriented shells; 112 shells were undecided (their coarse
tessellation failed, mostly roots that also fail or exhaust budgets at tessellation). The
repair-on survey took 758 s against 539 s strict.

Review notes. A first repair-on run reported 31 strict `BrokenWire` roots as
non-orientable shells: an orientation conflict found before validation hid the earlier
defect. A non-orientable shell is now reported only when validation itself fails on
orientation. The same run moved 51 roots from topology to a trim `WrongOrientation`:
their faces' loops run against their surfaces (the synthetic cubes write every loop
clockwise about its outward normal), which face flips cannot fix; the loop reversal
pass meshes them. Its first outward-orientation rule (four chords times the curved
area) left 78 meshed roots undecided, for example an exporter rod 10 m long and 12 mm
in radius; the deviation is now measured per triangle.

# Milestone 22.8: document import sessions and scaling benchmark — 2026-10-09

Delivered: `ImportSession` (`ImportProfile`, `SessionBudgets`, `SessionStatistics`):
discovery once per document, `prepare` decoding each profile's roots together, imports
adapting from the shared records with the free functions' results, and per-session
decode, adapter and triangle budgets named in diagnostics. `import_assembly` and
`link_assembly` use a one-shot session; the survey and assembly examples (with styles)
use one session per document. The `session` benchmark runs nightly with the corpus;
`examples/session.rs` runs the same report on chosen files. See
[STEP_IMPORT.md](STEP_IMPORT.md#document-import-sessions).

Measurement first, before the session (release survey example with temporary stage
timers, macOS arm64 shared with about eight concurrently compiling agents, so every
timing here is under load): `bug33261.stp` (77,956 entities, 24 B-rep roots) parsed in
0.13 s, decoded all closures (60,433 records) in 0.17 s, adapted in 0.5 s and spent the
rest tessellating, where most of its 12 failing roots fail (`UnresolvedTolerance`,
`SelfIntersection`, `ResourceLimit`); `trj3_s1-id-203.stp` (5,262 entities, 8 roots)
spent 21 s tessellating, mostly three roots that exhaust their budgets, and 0.01 s
decoding; `bug31711.stp` (one root, 396k closure records) decoded in 1.1 s and
adapted for 20 s. Decoding was already closure sized (2026-09-26), and the roots'
closures were nearly disjoint, so per-root decoding already cost the document's size
once. What a document repeated per root was discovery: a whole-document scan plus
representation and context decodes for each caller that needs a root's units.

| Check | Result |
| --- | --- |
| `cargo test --workspace`, fmt, Clippy (`-D warnings`, all targets), rustdoc (`-D warnings`) | passed (276 tests), including `tests/session.rs` (9 tests) |
| `check_architecture.py`, `check_geometry.py`, `check_product.py`, `check_metamorphic.py`, `conformance.py --check` | passed (83, 11 and 76 reviewed outcomes) |
| survey JSON (timing fields removed) and `assembly --meshes` output, base `c80d0ff` against the session, every unique corpus input | byte-identical with identical exit codes on all 3,227, twice |
| `corpus.py --check --repeat 2` with the AP validator | 3,227 compatible, no baseline change and no stage or outcome movement |

Session statistics over the corpus, every root imported (survey flow) and the assembly
flow, with unlimited session budgets: shared decodes in 135 files (137 groups), 8
fallbacks when a root of the group leaves its profile, at most 1.0 decoded and 1.86
adapted records per document entity, and at most 99,434 triangles per document (62.6
per entity, at the 0.1 mm assembly chord). Roots shared 1,281 of 2,327,197 decoded
records, mostly styles of NIST annotation occurrences and synthetic dodgy-step files.
`SessionBudgets::for_document` therefore allows 4 decoded and 16 adapted records and
256 triangles per entity, above floors of 1,000,000, 1,000,000 and 20,000,000.

Local stage medians of three runs of `examples/session.rs` (seconds, every selected
root at the assembly tolerances; peak memory is Linux-only and recorded nightly):

| Input | Parse | Session: discover / decode / adapt / tessellate | Independent: discover / decode+adapt / tessellate |
| --- | --- | --- | --- |
| `bug33261.stp`, 24 roots | 0.14 | 0.002 / 0.18 / 0.42 / 21.9 | 0.044 / 0.56 / 20.8 |
| `trj3_s1-id-203.stp`, 8 roots | 0.01 | 0.000 / 0.009 / 0.034 / 19.7 | 0.004 / 0.043 / 19.1 |
| `bug32239.STEP`, 15 roots | 0.35 | 0.003 / 0.48 / 0.32 / 1.08 | 0.041 / 0.66 / 1.11 |
| `bug31711.stp`, 1 root | 0.85 | 0.009 / (alone) / 16.2 / 0.03 | 0.003 / 15.8 / 0.03 |

Generated documents of K washers, decode and adapt per root without tessellation:
session 251, 294, 292 and 293 µs at K = 1, 8, 64 and 256; independent per-root import
251, 399, 1,636 and 5,844 µs, 20× the session at 256 roots, because each root
rediscovers its units across the whole document.

Sharing a decode of disjoint closures is not free. With temporary timers, the union
decode of `bug32239.STEP`'s 15 roots took 0.367 s against 0.340 s for the 15 per-root
decodes (`bug33261.stp`: 0.126 against 0.112 s), and recovering each root's closure
from the shared records 0.09 s (0.015 s), after looking each position up once and
sorting only for the curved adapter had cut the session's adapt stage on
`bug32239.STEP` from 0.28 to 0.20 s. The shared records stay resident until
the last root is imported: survey peak RSS rose from 180 to 198 MiB on `bug32239.STEP`
and from 113 to 129 MiB on `bug33261.stp` (unchanged at 588 MiB on single-root
`bug31711.stp`). Summed corpus wall time of the survey and assembly examples did not
change beyond noise (376 and 356 s base, 366 and 362 s session, four workers). The
survey and `import_assembly` prepare by default so that every multi-root corpus file
exercises shared decoding; whether to keep that default is left for review.

# Milestone 22.2: curvature-anisotropic refinement and tessellation budgets — 2026-10-09

Delivered: refinement and Lawson flips in a per-vertex curvature metric; explicit polar
caps around pole runs; deferred boundary requests, faces reused across boundary passes
and facets accepted once per face; facets measured through the surface lift of their
corners and, below the model tolerance, by chord only; pole edges split halfway in 3D
along their ray; a length-ordered bridge search and a grid-indexed ear test for planar
triangulation; bisected knot lines and skipped single-cell knot probes for B-spline
faces; and `TessellationError::budget`, naming every exhausted tessellation budget and
its value. Tessellation defaults are unchanged. See
[TESSELLATION.md](TESSELLATION.md#curvature-anisotropic-refinement-milestone-222).

Diagnosis on the Onshape export (`~/Downloads/Core__2_.step`, not redistributable),
face by face with a scratch driver: the torus band #2168 failed as described in the
roadmap; a pure normal-turn metric (third fundamental form) made the band facets
seven times longer than wide, whose planes tilt out of the angle (the shape has to
follow |II|); the 36 two-pole B-spline faces folded their pole fans in the raw-chart ear
clipping (11 folded wedges in face #2076's first triangulation) and also evaluated a
noise normal exactly at a pole their control row collapses only within tolerance;
splits of pole edges from two copies of one pole created coincident vertices; face
#2157's chart is so slow next to its poles that the chart midpoint of a pole edge lay
0.5 nm from the pole while the other end was 85 µm away, and its surface folds within
30 µm of the poles (below the 20 µm-scale resolution the metric now caps); boundary
samples sit up to 10 µm off the B-spline surfaces (model tolerance 20 µm); the
36-hole cylinder #2164 spent 24.7M work units in bridge search and 3.4M in ear tests;
and the whole solid then exhausted `max_evaluations` because every pass remeshed
every face and re-probed accepted facets, and B-spline probes scanned whole knot
vectors.

| Check | Result |
| --- | --- |
| `cargo run --release -p tessstep-import --example survey -- ~/Downloads/Core__2_.step 1` | accepted: profile, geometry and tessellation; closed solid with 65,766 vertices, 131,556 triangles, volume 5.8498e-6 m³, 7.4 s for the root (chord 4.36e-5 m, 0.1 rad); 7.65M of the 10M evaluations and 27.8M of the 50M work units |
| `cargo test -p tessstep-tessellate --test anisotropic` | passed: `narrow_torus_band_refines_across_the_band`, `planar_face_with_many_holes_meshes_within_default_budgets`, `pole_caps_keep_fans_monotone_beside_seams_and_holes`, `tessellation_budgets_are_named_in_diagnostics`; on `c80d0ff` the first three fail (`UnresolvedTolerance`, `ResourceLimit`, `ResourceLimit`) |
| `cargo test --workspace --locked` | passed (80 test binaries) |
| fmt, Clippy (`-D warnings`, all targets), `cargo doc` (`-D warnings`) | passed |
| `check_architecture.py`, `check_geometry.py`, `check_metamorphic.py`, `conformance.py --check` | passed (83 reviewed geometry outcomes, 76 generated expectations) |
| `corpus.py --check --repeat 2` with the AP validator | 3,227 compatible inputs, no crash, timeout or nondeterminism, 272 s; 47 inputs changed against the baseline (reviewed below) |

All existing adaptive tests (`solid_torus_welds_both_periodic_seams`,
`pole_and_apex_charts_close_with_collapsed_edges` and the others) hold their sampled
bounds. The bridge order, the ear grid and the knot-line bisection evaluate the same
predicates on the same candidates as the scans they replace; the knot change and the
skipped single-cell probes left the Onshape mesh unchanged (65,766 vertices and the
same volume before and after), while acceptance work fell from 33M to 14M units and
acceptance evaluations from 8.8M to 7.2M.

**Reviewed corpus movements** (per root, against the 2026-09-27 22.1 run; baseline not
saved): 1,197 shape roots, 745 passing geometry/topology (unchanged), 643 meshed (577
before). 66 roots newly mesh: 25 failed with `UnresolvedTolerance` (round or pass
limits), 24 with the adaptive `ResourceLimit`, 5 with a planar `Polygon(ResourceLimit)`
and 12 with a polygon-validity error (`SelfIntersection`, one `DegenerateLoop`). The
last group failed only after about a dozen trickling boundary passes had refined
their boundaries far enough to self-intersect at mesh resolution (on `c80d0ff`,
`trj3_s1-tc-214.stp` root #2249's face 1 merely runs out of passes when limited to
12 or fewer and fails polygon validation with 14 or more); with deferred requests they never get there, which is a
consequence rather than a guarantee (with polar caps disabled they still mesh). By
file: `bug33261.stp` (+8), `trj3_s1-tc-214.stp` (+4), three each in
`trj3_s1-id-203`, `trj3_s1-ec-214`, `trj3_s1-ec-203`, `trj4_s1-ai-214` and
`trj4_s1-ec-214`, two in `bug24601_216523_2_03.stp` and `pr1db.stp`, and one each in
the NIST CTC/FTC/STC files, `bug22715_cube.stp`, `bug28389_CONFIDENTIAL_SHEET_METAL_F3D.stp`,
`bug32239.STEP`, `pr1ct.stp`, `io1-cm-214.stp` and 20 others. No root lost its
outcome. Four roots changed failure category, each reviewed:

* `nist_ctc_02_asme1_rc.stp` #20700 and `nist_ctc_02_asme1_ap242-e2.stp` #14911:
  `Polygon(ResourceLimit)` → `Polygon(UnresolvedGeometry)` (planar face #9481 and
  model face 164). The cheaper triangulation now reaches the polygon itself; `c80d0ff` with
  a 2·10⁹ planar budget fails the same faces the same way.
* `bug33261.stp` #46035: `UnresolvedTolerance` → `ResourceLimit (budget max_evaluations
  10000000 exhausted)`; it meshes with 10–15M evaluations (178,946 vertices, 5.3 s).
* `bug24593_281946.stp` #30: face 4's `SelfIntersection` no longer occurs; face 11 then
  exhausts `max_work` (50M); it meshes with 50–75M (24,837 vertices, 6.7 s).

`tessellation:resource_limit` roots fell from 54 to 25, and every one names its
budget: 22 `planar.trim.max_work` (trim reconstruction, quadratic loop checks in
`tessstep-trim`; `as1_motor.step` root #143 still fails at 50M and meshes at 500M),
two `max_work` (`bug24593_281946.stp` #30 above; `bug26889_export.step` #34 then needs
more than 100,000 vertices on one face) and one `max_evaluations`. `UnresolvedTolerance`
roots fell from 29 to 3, which now name `max_rounds`. The baseline comparison reports
47 changed inputs (stages and accepted-root counts only).

# Milestone 22.3: tolerance policy for edges off their surfaces — 2026-10-09

Delivered: `ImportOptions::deviation_factor` (default 16, `strict` forces 1);
`Adaptations::off_surface_edges`, `off_curve_vertices` and `uncollapsed_sides`;
`ImportedSolid::largest_deviation` and the working tolerance; typed rejections naming
the measured distance and the bound; B-spline sides that collapse only within the
model tolerance charted as regular sides; survey `--deviation-factor=F` and per-root
`tolerance_adaptations` ([curved import](CURVED_IMPORT.md#tolerance-policy)).

Measurement first. Every root of the reviewed baseline that failed with one of the
three kinds was re-imported by the survey (each root's model tolerance is its context
uncertainty floored at 1e-7 m): with the baseline binary for the first failing
distance, and with an unbounded multiple for the largest distance the root would
need. Ratios are distance over model tolerance; the first failing distance is a lower
bound, and two baseline distances (CATIA through Theorem, 160×; Alias, 8.9×; I-DEAS,
10.2×) were inversion artefacts: a NURBS inversion stopping short of the closest point
near a pole or seam, which an accepted import no longer measures.

| Kind | Exporter (`FILE_NAME` originating system) | Roots | Model tolerance (m) | Baseline ratio | Largest ratio | Outcome with the default policy |
| --- | --- | ---: | --- | --- | --- | --- |
| edge off surface | SolidWorks 2012/2020, HOOPS for SolidWorks MBD | 12 | 1e-7, 2.5e-7, 1e-6 | 1.06–2.09 | 1.2–7.4; one 17.4 | 3 meshed, 7 tessellation, 1 rejected (17.4×), 1 multiple outer bounds |
| | CATIA V5 R19 (stepcode) | 3 | 5e-6 | 1.01–1.15 | 1.26, 1.26, 15.9 | 3 meshed |
| | CATIA through Theorem AP209 | 2 | 1e-7 | 160 (artefact) | 3.5 | 2 tessellation |
| | NX/UG (Theorem, ST-DEVELOPER) | 2 | 3.1e-5, 5.2e-5 | 1.00, 8.25 | 1.23, 8.84 | 1 tessellation, 1 refused chord (0.46 mm deviation) |
| | XStep 1.0 | 3 | 1e-7 | 1.45–2.79 | 2.65–5.86 | 1 meshed, 2 tessellation |
| | AutoCAD 2000i | 3 | 1e-7, 3e-7 | 3.11–31.1 | 7.30; 16.7, 31.1 | 1 tessellation, 2 rejected |
| | Inventor 4.0 | 2 | 1e-7 | 3.07, 472 | 7.79 | 1 meshed, 1 rejected |
| | Alibre | 1 | 1e-7 | 1.40 | 7.79 | 1 meshed |
| | Alias AutoStudio | 2 | 2e-7, 2e-6 | 1.41, 8.89 (artefact) | 4.50 | 2 tessellation |
| | EUCLID | 3 | 1e-7, 1e-6, 1.5e-5 | 1.14–12.6 | 4.14; 29.5, 203 | 1 tessellation, 2 rejected |
| | Open CASCADE (MATRA-DATAVISION) | 3 | 1e-7, 2e-7 | 14.7, 102,350 (2) | 28.8; 102,350 | 3 rejected |
| | Datakit CrossCadWare | 2 | 5e-6 | 1.20, 133 | 1.95; 133 | 1 tessellation, 1 rejected |
| | none written (NIST PMI test files) | 5 | 5e-6, 1.7e-5 | 1.03–2.41 | 1.25–10.0 | 5 tessellation |
| | Creo, ESSENCE, ST-DEVELOPER step.cde, I-DEAS | 4 | 1e-7–1e-5 | 1.09–10.2 | 1.09–4.05 | 4 tessellation |
| | synthetic (`dodgy-step-files`) | 30 | 1e-7 | 1.74–995,000 | beyond 16 | 30 rejected |
| vertex off curve | MicroStation/J | 2 | 1e-7 | — | 9.79, 9.83 | 2 meshed |
| | Datakit, AutoCAD, FPX Expert, Open CASCADE 6.6 | 4 | 1e-7, 5e-6 | — | 2.89; 53.5–5,592 | 1 tessellation, 3 rejected |
| | Open CASCADE 5.2, SolidWorks 2011 | 2 | 1e-6, 1e-7 | — | edges 19.4, 45 | 2 rejected (edge off surface) |
| | Alias (closed B-spline edges) | 2 | 1e-3 | — | within 16 | 2 topology (non-manifold vertex) |
| | synthetic | 10 | 1e-7 | — | — | 7 rejected, 3 later stages |
| singular crossing | NX 6.0 (`bug24383`) | 15 | 5.08e-5 | — | 46 µm side | uncollapsed; 15 rejected (edges 34 µm from exact poles) |
| | NIST PMI, Inventor 10 | 2 | 1.27e-4, 1e-5 | — | 0.10 mm, 5 µm sides | 2 tessellation |
| | Open CASCADE 4.0 (`trj7`/`trj8_b1-ec`) | 2 | 1e-3 | — | 0.39 mm side | 2 "face loop runs against its surface along a pole" |
| | Open CASCADE 7.5 (`as1_motor`) | 1 | 1e-7 | — | no surface normal | 1 rejected (singular parameterization) |

The baseline vertex message named no distance, hence the dashes. Exporter deviations
cluster below 10 times the tolerance; the default 16 admits them with margin (the
largest meshed root, CATIA `HEAD_BACK.stp`, needs 15.9) while the next exporter
roots need 16.7 to 102,350 times. Synthetic files place deviations at exactly 5, 10,
20, 25, 80 and 100 times the floored tolerance; a bound on 10 or 20 would make their
outcomes depend on the last bit of a distance.

| Check | Result |
| --- | --- |
| `cargo test --workspace`, fmt, Clippy (`-D warnings`, all targets), `cargo doc` (`-D warnings`) | passed, including the four `tests/tolerance.rs` tests |
| `check_geometry.py --require-external` | 87 reviewed outcomes passed (four new tolerance fixtures) |
| `check_capi.py --library-dir target/release` | passed (the C assembly import now uses the default policy); the sanitizer variant hangs on macOS 27 and was not run |
| `check_metamorphic.py`, `check_architecture.py`, `check_corpus.py`, `conformance.py --check`, script unit tests (45) | passed |
| `corpus.py --check` with the AP validator | 3,227 compatible, 34 changed, no regressions |

Reviewed corpus movements (every changed root re-imported with the baseline and the
new survey): 46 roots in 34 files changed and none lost a mesh; 41 more roots pass
geometry/topology (745 → 786). 11 roots newly mesh (577 → 588): CATIA
`MAINBODY_BACK`, `MAINBODY_FRONT` and `HEAD_BACK` (largest deviations 6.3, 6.3 and
80 µm under 5 µm), SolidWorks `bug33398` #13357 and #13360 and `bug32239` #68579,
XStep `2.stp` #937, Alibre `trj4_s1-ai` #3138, Inventor `trj5_s1-in` #4186 and
MicroStation `trj3_s1-md` #1708 and #2469 (off-curve vertices and off-surface edges up
to 0.98 µm under 0.1 µm). All are closed single-component meshes. The Alibre and
Inventor parts measure 2.9627e-4 and 2.9639e-4 m³ against 2.9591e-4 (UG), 2.9598e-4
(I-DEAS) and 2.9612e-4 m³ (Pro/ENGINEER) for the same part of the `s1` assembly from
exporters without deviations (the Alibre part is 1.4 mm taller); the two
MicroStation roots agree with each other to 3e-6 and converge under chord refinement.
30 roots move from the geometry stage to tessellation: `SelfIntersection`,
`UnresolvedTolerance`, trim `Limit` and `ResourceLimit` faces (Milestones 22.2 and
22.4), and `OCC5157.stp`, whose 0.46 mm deviation exceeds its chord. The two
`trj7`/`trj8_b1-ec` roots move from the singular crossing to the existing "face loop
runs against its surface along a pole or apex line" rejection, `bug32239` #68354 to
"multiple outer bounds", and two roots stay off-surface rejections with the bound in
their message (`bug33398` #13351 at 17.4×, `trj5_s1-in` #1480 at 29,500×). Unchanged
files only extend their rejection messages with the bound.

# Milestone 22.1: named budgets and document-sized import defaults — 2026-09-27

Delivered: every parse, decode, adapter, discovery, assembly, style, pcurve and mesh
budget failure names the budget and its configured value; `ImportOptions::for_document`
sizes `max_work` (512 units per entity) and `max_records` (one per entity) with the
default floors; `stepdump --max-<budget>=N`; survey `--max-work=N` / `--max-records=N`
and corpus `--geometry-max-work` / `--geometry-max-records`, with the budgets used
recorded per file in the corpus report.

Measurement first: the physical parse defaults never limited a corpus file. Every
well-formed input up to 30.7 MB and 413,209 entities (`bug31711.stp`) parses within
them; the larger files (59 MB, 24 MB, 10 MB, 7.7 MB) are rejected for invalid UTF-8
or hexadecimal, not budgets. The 11 `profile:resource_limit` roots all failed the
import work budget of 5,000,000 units during closure decoding. Single-root closures
need between 128 and 256 units per entity (`bug31711` 413k entities: 128× fails,
256× passes in 10 s; `tr9_r1001-ec` 112k: 64× fails, 128× passes; `OCC8228` 92k:
128× passes; `bug24055` 91k: 128× fails, 256× passes), so the rule uses 512.

| Check | Result |
| --- | --- |
| `cargo test --workspace`, fmt, Clippy (`-D warnings`, all targets) | passed, including `every_resource_budget_is_enforced` (message names the limit) and `import_budgets_are_sized_by_the_document_and_named_in_diagnostics` |
| `check_geometry.py`, `check_cli.py`, `conformance.py --check`, script unit tests (45) | passed |
| `corpus.py --check` with the AP validator | 3,227 compatible, 10 changed, no regressions |

Reviewed movements, all from `profile:resource_limit` to a later stage: `bug31711.stp`
and `bug23202_Leuchte.stp` reach tessellation (`tessellation:invalid_geometry`);
`bug24024_slow_import.stp`, `bug26689_nist_ctc_02_asme1_ap242-2.stp` and
`bug29531_nist_ctc_02_asme1_ap242-2.stp` reach tessellation and hit its face budget
(Milestone 22.2); `OCC5157.stp`, `OCC8228.stp` and `trj12_b3-tu-203.stp` fail geometry
(`invalid_geometry`); `bug24055_8MTF-75LS05.step` and `tr9_r1001-ec.stp` name an
unsupported entity. No meshed root was lost (577). The baseline was saved after this
review.

# CI repair: platform-independent fixtures and corpus deadline — 2026-09-27

Main CI had failed on every push since 2026-09-26 while every check passed on the
macOS development machine. Two causes, both in the test infrastructure:

- `check_geometry.py` requires regenerated fixtures to be byte-identical to
  `corpus/geometry/*.step`. On Linux `brep-cone.step` differed by one ulp in
  `10 + 10 * tan(radians(30))` (libm), and on Windows every generated fixture differed
  because `write_text` translated `\n` to CRLF. `num()` in `brep_fixtures.py` now
  rounds to 15 significant digits before `repr`, and both generators write with
  `newline="\n"`. Thirteen fixtures changed only in their last one or two digits
  (`corpus/manifest.json` hashes updated); `check_geometry.py` still passes its 83
  reviewed outcomes, and `cargo test --workspace` (266 tests) and the script unit tests
  (44) pass.
- The corpus runner's default per-input deadline was 30 s. `bug33261.stp` and
  `trj3_s1-id-203.stp` take 13 s and 12 s here but crossed 30 s on hosted runners in
  consecutive runs, each reported as a regression. The default is now 120 s; the local
  `corpus.py --check` with the AP validator reports 3,227 compatible inputs and no
  baseline change.

# Milestone 21 completion: assemblies, colours, supplied pcurves, C/C++ — 2026-09-27

Delivered: assembly linking and import (`link_assembly`, `import_assembly`), surface
colours (`import_appearance`), schema selection and assembly import in C/C++ (64
exports), the corpus product stage, the closure-type coverage survey, the AP-schema
join of profile rejections, `UNIFORM_*`/`BEZIER_*` knots, `PARABOLA`/`HYPERBOLA`
edges, supplied pcurves and named rejections of rare geometry.

| Check | Result |
| --- | --- |
| `cargo test --workspace`, fmt, Clippy (`-D warnings`, all targets) | passed |
| `check_geometry.py --require-external` | 83 reviewed outcomes passed, including four authored assembly fixtures and four hash-pinned exporter assemblies |
| `check_capi.py` (installed C/C++ consumers) | passed; the sanitizer variant hangs on macOS 27 and was not run locally |
| `check_coverage.py`, `conformance.py --check`, other check scripts | passed; 501 AP242 entities classified (68 adapted) |
| `corpus.py --check` | 3,227 compatible inputs, no regressions |

Corpus outcomes: 1,197 shape roots in 584 files, 740 passing geometry/topology and 577
meshed (566 after the seam fix). The product stage links 2,652 inputs with a mapped
schema: 585 accepted (1,330 products, 1,096 occurrences in 97 assemblies, 2,260 placed
definition nodes, 607 placed shape roots), 1,807 rejected, 23 unsupported and 237
without product records. About 1,740 rejections are synthetic `dodgy-step-files`
inputs that write five-parameter `PRODUCT_DEFINITION`s; the real-exporter rejections
are typed nonconformances (explicit SI dimensions, untyped measures in selects, unset
required names, unordered complex instances) or genuine graph errors.

Reviewed baseline movements before saving: 2,645 inputs gain the newly measured product
stage and 207 only the closure-type survey; NIST AP203 inch files (`nist_ctc_02`,
`nist_ctc_05`, `nist_ftc_09`) now use their context units after the tolerated
`NAMED_UNIT(*)` on conversion-based units and reach tessellation; `bug28345_30338.stp`
(+2), `bug33261.stp` (+5) and `trj3_s1-db-214.stp` (+4) gain meshed roots;
`OCC8228.stp` moves from unsupported to a resource limit. An earlier supplied-pcurve
rule lost roots on `-ec` exporter files (rounded periods such as `6.28318530718`, and
mixed supplied and computed corners); the final rule (B-spline faces only, every use
supplied and agreeing, knot-span probes at half the model tolerance) loses none.

# Curved import: spindle tori, seam placement, tolerance-consistent meshing — 2026-09-27

`DEGENERATE_TOROIDAL_SURFACE` imports as a spindle torus (apple or lemon part as a
closed v domain ending at the axis points); seams are placed by ray casting along
candidate isoparametric lines; a two-edge face whose edges coincide within the model
tolerance is collapsed (`collapsed_faces`); the edge sampler accepts vertices within
the model tolerance of their curve end and drops seed parameters within numerical
noise; the trimmer decides joins, closure and polygon validity in metres through the
largest surface derivative per chart axis, accepting joins that coincide in space;
and the pcurve work budget applies per planning pass (passes stay capped at 8). See
[SURFACES.md](SURFACES.md), [CURVED_IMPORT.md](CURVED_IMPORT.md#face-charts),
[TESSELLATION.md](TESSELLATION.md) and [TRIMMING.md](TRIMMING.md).

| Check | Result |
| --- | --- |
| `cargo test --workspace` | passed, including `analytic_spindle_torus_parts_end_at_the_axis_points`, `brep_rejections_are_typed_and_located` (horn torus and invalid degenerate torus) and the reworked `shared_edge_collapsed_curves_and_endpoint_snaps_are_diagnosed` |
| Workspace fmt and Clippy (`-D warnings`, all targets) | passed |
| `check_geometry.py` | 61 reviewed outcomes passed (new: spindle apple and lemon zones with closed-form volumes) |
| Onshape export (`~/Downloads/Core__2_.step`, not redistributable) | passes profile, geometry and topology for all 149 faces; tessellation stops at the 36-hole cylinder (default 10M planar work budget) and, with larger budgets, at torus band #2168 (`UnresolvedTolerance`, see ROADMAP) |

**Seam regression resolved.** The WIP slice lost one meshed root each in
`bug32239.STEP` and `trj3/4/5_s1-ug-214.stp` to a planar
`Polygon(InvalidPolygon(SelfIntersection))`. Diagnosis on `trj3_s1-ug-214.stp` root
#563 (face #667, a cylinder annulus whose loops overlap along the axis): the first
planning pass split the loop-b B-spline where it crosses the seam line; after the
split halves' pcurves were refitted, the new vertex sat 3e-6 rad (1.1e-7 m) off the
line, above the vertex-hit threshold of 1e-3 of the model tolerance, so the next pass
split again beside it and left a 1e-7 m sliver edge whose corner backtracked in UV.
A vertex within the model tolerance of the candidate line is now on it, and a crossing
beside such a vertex hit of the same loop is dropped instead of split
(`seam`, [CURVED_IMPORT.md](CURVED_IMPORT.md#face-charts)). All four roots mesh
(root #563: one split, one closed component).

`python3 scripts/corpus.py --check` then passed with 3,227 compatible inputs: 1,197
shape roots in 584 files, 720 passing geometry/topology and 566 meshed (564 in the
Milestone 20 baseline). Reviewed baseline movements, after which the baseline was
saved: `trj4_s1-ai-214.stp` gains two meshed roots; `bug31685_1.stp` moves from
unsupported to a typed rejection (its degenerate tori now import, and edge #4213 lies
1.2e-6 m off its face, a Milestone 22 tolerance item); `bug25281_tess_infloop_extract.step`
and `bug28389_CONFIDENTIAL_SHEET_METAL_F3D.stp` move from a tessellation rejection to
the tessellation resource limit (they now reach refinement; tessellation budgets are
Milestone 22). The refinement experiments for the torus-band failure (turning-edge
splits, a flip guard, transitive splitting, thin-facet acceptance) were reverted: each
either diverged on `solid_torus_welds_both_periodic_seams` or broke
`pole_and_apex_charts_close_with_collapsed_edges`.

# Milestone 21: AP schema compilation, corpus schema stage and coverage matrix — 2026-09-27

The EXPRESS frontend now compiles the AP242 (WG12 N11521 MIM long form, 2,407
entities), AP214 edition 3 (`AUTOMOTIVE_DESIGN`), AP203 edition 2 MIM long form and
AP203 edition 1 (`CONFIG_CONTROL_DESIGN`) schemas that stepcode distributes, pinned by
revision and SHA-256 in `corpus/ap-schemas.json`. Four frontend gaps blocked all of
them: `SELF\supertype.name` redeclarations (1,011 in AP242), identifiers beginning
with `END_` inside opaque expressions, defined types recursing through an aggregate
(`maths_value`/`maths_tuple`) and same-named attributes inherited from different
supertypes (88 in AP203e2). All four compile structurally with retained (unevaluated)
rules, functions and DERIVE expressions: AP242 resolves 3,363 declarations with 6,916
unsupported diagnostics. One generated validator holding all four schemas is 19.7 MB
of Rust, builds in about 20 s and produces an 8.9 MB executable
(`scripts/build_ap_validator.py`).

Decoder changes for AP metadata, all with authored tests: a redeclared explicit
attribute occupies no physical slot and a DERIVE redeclaration requires `*` in the
ancestor's slot; the retaining policy (`decode_policy`) counts rules, DERIVE and
INVERSE attributes instead of rejecting the schema set; `$` in DERIVE-redeclared
slots and unordered external complex components are explicit, counted tolerances;
tagged SELECT values with different type tags are distinct SET members (the NIST
AP242 files hold `(U_DIRECTION_COUNT(2), V_DIRECTION_COUNT(2))`, and CAx-IF files
hold `BOX_SLANT_ANGLE(0.)` beside `BOX_ROTATE_ANGLE(0.)`, both aliases of one angle
type); entity names are resolved by binary search.

| Check | Result |
| --- | --- |
| `cargo test --workspace --release` | passed, including `express_redeclarations_qualified_names_and_end_identifiers`, `schema_decode_retains_unevaluated_semantics_only_by_policy`, `schema_decode_tolerances_are_explicit_and_counted` and the regenerated codegen consumer with redeclared and qualified fields |
| Workspace fmt, Clippy (`-D warnings`, all targets) | passed |
| Python verification tests; EXPRESS, schema, product, geometry, architecture checks | passed |
| `scripts/check_coverage.py` | 501 instantiable AP242 geometry/topology/shape entities classified: 52 adapted, 13 supertype, 2 product, 123 unsupported, 311 out of scope |

## Corpus schema stage

`scripts/corpus.py --schema-validator … --schema-map corpus/ap-schema-map.json`
selects each input's schema from its declared `FILE_SCHEMA` name and validates
structurally under the retaining policy. Of 2,859 physically accepted inputs, 2,652
declare one of the four schemas: `AUTOMOTIVE_DESIGN` 243 accepted / 2,157 rejected /
1 unsupported, AP242 28 / 146, `CONFIG_CONTROL_DESIGN` 22 / 41, AP203e2 5 / 9; 575
inputs declare other names (AP238, `AUTOMOTIVE_DESIGN_CC*`, AP209, IFC, test
schemas) and stay `not_configured`. The whole stage adds about 40 s to the run.

The rejections were reviewed by entity. The dominant one is synthetic: 1,393
`dodgy-step-files` inputs write `PRODUCT_DEFINITION` with five parameters where every
AP declares four, and 130 encode `CONVERSION_BASED_UNIT` units against AP203 layouts
under an AP214 header. On real exporter files (OpenCASCADE, NIST, CAx-IF rounds,
stepcode) the rejections are genuine deviations from the declared schema: untagged
reals in `trimming_select`/`size_select` slots, `STYLED_ITEM.item` pointing at a
representation (valid in AP242, not AP214), `$` in required `name`/`description`
attributes, empty `SET [1:?]` aggregates, AP203e1 selects used under an AP203e2
header, presentation entities under `CONFIG_CONTROL_DESIGN`, and unordered or
incomplete complex instances. Two decoder decisions came out of this review (the
SELECT tag rule and the counted tolerances); no rejection was silenced. Accepted
inputs carried 2.59 million unevaluated rule applications and 204 tolerated `$`
derived slots, which is why acceptance is reported as structural validity, not AP
conformance.

Refreshing the baseline changed 2,859 inputs (the new schema outcome) and moved three
geometry outcomes from `resource_limit` to `rejected`/`unsupported` because the
binary-search lookup leaves more decoder budget for the closure; solid roots meshed
stay at 557 of 1,173.

# Curved import: swept surfaces and cavity shells — 2026-09-26

`SURFACE_OF_LINEAR_EXTRUSION` and `SURFACE_OF_REVOLUTION` import as exact elementary
charts or exact NURBS with the STEP normal orientation preserved; `TRIMMED_CURVE`s
contribute their basis curve and sense; `BREP_WITH_VOIDS` cavity shells (with
`ORIENTED_CLOSED_SHELL` orientation) mesh as inward components. Kernel additions:
`NurbsSurface::isocurve`, `Solid::voids` with disjoint-shell validation, per-shell
volume checks and `TessellationErrorKind::CavityOrientation`. See
[CURVED_IMPORT.md](CURVED_IMPORT.md#swept-surfaces), [TOPOLOGY.md](TOPOLOGY.md) and
[TESSELLATION.md](TESSELLATION.md).

| Check | Result |
| --- | --- |
| `cargo test --workspace --release` | 240 tests/doctests passed, including `brep_swept_surfaces_are_exact_elementary_or_nurbs_charts`, `brep_cavity_shells_mesh_as_inward_components`, `topology_cavity_shells_are_closed_and_uniquely_owned`, `nurbs_surface_isocurves_agree_with_the_surface` |
| Workspace fmt, Clippy (`-D warnings`, all targets/features) and rustdoc | passed |
| Python verification tests; kernel, CLI, EXPRESS, schema, product, metamorphic, architecture checks | passed (41 tests) |
| Fixture provenance and generated conformance | passed; 94 fixtures |
| `check_geometry.py --require-external` | 59 reviewed outcomes passed (new: extruded circle and reversed trimmed ellipse, revolved line, B-spline semicircle and circle, cube and cylinder cavities, rejected outward cavity) |
| `check_capi.py --library-dir target/release` (no sanitizers; local ASan hang) | installed C/C++ package checks passed |

`python3 scripts/corpus.py --check` passed with 52 changed inputs and no regressions:
roots producing solid meshes rose from 521 to 557 of 1,173. Reviewed movements: 36
roots now mesh (25 swept, 11 OpenCASCADE cavity solids); 49 formerly unsupported roots
reach later typed failures (trim and polygon failures on newly planned faces, off-curve
edge endpoints, and 4 OpenCASCADE cavity solids whose voids face out of their cavity,
diagnosed as `CavityOrientation`). The baseline was saved after this review.

# Curved import: pole and apex charts — 2026-09-26

Faces that reach a sphere pole, a cone apex or a collapsed B-spline side now close their
chart with collapsed edges instead of failing: loop joins at singular vertices,
enclosing loops with a pole seam, `VERTEX_LOOP` bounds and full spheres; see
[CURVED_IMPORT.md](CURVED_IMPORT.md#face-charts), [TOPOLOGY.md](TOPOLOGY.md#collapsed-edges)
and [TESSELLATION.md](TESSELLATION.md#poles-and-apices). Supporting changes: exact
meridian pcurves, one-sided pcurve limits at pole ends, explicit detection of curves
through a pole (which the weak Jacobian test missed), distinct-point NURBS inversion
seeds with a step off singular seeds, and 4-ulp end snapping in closed surface and
knot domains (convex combinations on a B-spline boundary previously fell outside it and
failed every pcurve probe).

| Check | Result |
| --- | --- |
| `cargo test --workspace --release` | 236 tests/doctests passed, including `pole_and_apex_charts_close_with_collapsed_edges`, `topology_collapsed_edges_close_pole_and_apex_charts`, `brep_poles_and_apices_close_with_collapsed_edges` |
| Workspace fmt, Clippy (`-D warnings`, all targets/features) and rustdoc | passed |
| Python verification tests; kernel, CLI, EXPRESS, schema, product, metamorphic, architecture checks | passed (41 tests) |
| Fixture provenance and generated conformance | passed; 86 fixtures |
| `check_geometry.py --require-external` | 51 reviewed outcomes passed (new: hemisphere, vertex-loop and seamed cone tips, sphere bounded by a vertex loop, pole-to-pole seamed sphere, collapsed B-spline tetrahedron base, rejected regular vertex loop) |
| `check_capi.py --library-dir target/release` (no sanitizers; local ASan hang) | installed C/C++ package checks passed |

`python3 scripts/corpus.py --check` passed with 89 changed inputs and no regressions:
roots producing solid meshes rose from 419 to 521 of 1,173 (geometry `unsupported`
185 → 91 roots). Reviewed movements: 101 roots now mesh; 34 formerly unsupported roots
reach specific later failures (open single-face test shells, planar loop orientation,
trim self-intersections and off-surface edges now reported after planning succeeds);
24 fail on tessellation budgets; 4 dodgy test loops that double back are now reported
as such. The baseline was saved after this review. Remaining blockers by frequency:
open or inconsistently oriented shells (193), edges off their surface (69), broken wires
(37), swept surfaces (53), tessellation budgets (41) and cavity shells (22).

# Curved import: edge splits, sphere re-charting and survey tolerance — 2026-09-26

Annular faces whose loop vertices are not aligned now split the crossed loop edge
(every face using it is updated) instead of failing, and spherical faces touching
their STEP pole are planned in rotated sphere frames. Both are counted in
`Adaptations`. The survey's tolerance floor rose from 1e-8 m to 1e-7 m after
OCC-written files showed curves 12–25 nm off surfaces that declare 5–10 nm
uncertainties.

| Check | Result |
| --- | --- |
| `cargo test -p tessstep-import` | 31 tests passed, including `brep_edge_splits_and_sphere_recharting_are_reported` |
| Workspace fmt and Clippy (`-D warnings`, all targets/features) | passed |
| Fixture provenance and generated conformance | passed; 81 fixtures |
| `check_geometry.py --require-external` | 46 reviewed outcomes passed (new: split cylinder, re-charted sphere cap, unsupported hemisphere) |

`python3 scripts/corpus.py --check` passed with 57 changed inputs and no regressions:
accepted solid roots rose from 348 to 419 of 1,173 (11 files `unsupported` →
`accepted`, 11 `partial` → `accepted`, 8 geometry-accepted files now failing only at
tessellation). The baseline was saved after this review. Remaining pole and apex
failures are hemispheres, whose great-circle boundary puts a pole on or inside every
sphere frame, and cone apices.

# Curved B-rep import, unit discovery and tessellation hardening — 2026-09-26

Added `import_brep_solid` for `MANIFOLD_SOLID_BREP`s on elementary and (rational,
quasi-uniform) B-spline surfaces with line, conic and B-spline edges, computed pcurves,
inferred outer bounds and inserted annulus seams, plus `discover_solids` for root,
unit and uncertainty discovery; see [CURVED_IMPORT.md](CURVED_IMPORT.md). Kernel
changes: validated periodic NURBS axes, whole-period hole placement in trimming,
aggregate decoder link slots, closure-sized selected-root decoding, and metric-aware
best-first ear clipping, queue-based Lawson flips and facet-plane chord error in the
tessellator. Rust only; no C/C++ entry point is claimed.

Observed locally on macOS arm64, rustc 1.98.1:

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | 232 tests/doctests passed |
| Workspace fmt, Clippy (`-D warnings`, all targets/features) and rustdoc (`-D warnings`) | passed |
| Architecture, authored fixture provenance and generated conformance | passed; 19 packages, 79 fixtures |
| Python verification tests | 41 passed |
| Kernel, CLI, EXPRESS, schema, product and metamorphic checks | passed |
| `check_capi.py --library-dir target/release` (no sanitizers; see local note) | installed C/C++ package checks passed |
| `check_geometry.py --require-external` | 44 reviewed outcomes passed, including 11 generated curved fixtures, the planar fixtures through the curved profile and 3 hash-pinned curved exporter files |
| `cargo check --manifest-path fuzz/Cargo.toml --locked` (unnested copy) | passed with the new `brep_import` target; no coverage-guided run |
| `cargo bench -p tessstep-import --bench brep` | washer import about 103 µs, tessellation 7.6 ms (1,024 triangles); per-root import flat from 82 to 10,496 entities |

The exporter files are an ST-DEVELOPER block with a seamless hole and no
`FACE_OUTER_BOUND` (2 inferred bounds, 1 seam), a CATIA V5 rod with degree-5 B-spline
geometry, and an I-DEAS part whose closed rational B-spline cylinders use periodic
NURBS charts. Each volume agrees with its closed form within the chord-dependent bound.

`python3 scripts/corpus.py --check` first reported 4 regressions: OCC files whose
contexts declare a 1e-16 m uncertainty that their coordinates cannot meet. The
survey now floors the declared uncertainty at 1e-8 m and reports both values. The
rerun passed (3,230 paths / 3,227 unique inputs, 115 s, all compatible) with 234
changed inputs and no regressions. Solid roots accepted through tessellation rose
from 22 to 348 of 1,173 discovered (82 files `unsupported` → `accepted`, 42 →
`partial`, 85 → `rejected` past the profile stage). Two reviewed reclassifications:
`bug31711.stp` no longer exhausts the decode budget and now names
`SURFACE_OF_LINEAR_EXTRUSION`, and `bug33665.step` names `OPEN_SHELL` as outside the
profile. The baseline was saved after this review.

Among unmodified exporter files (excluding the synthetic defect catalog), 312 roots
pass and the remaining failures, by frequency, are: pole or apex charts (about 110:
vertex loops, singular crossings and enclosing loops), edges farther than the
tolerance from their surface (90), annuli without aligned seam vertices (85),
unresolved pcurve fits (31), swept surfaces (52), cavity shells (16) and degenerate
UV loops (16). Every `OpenShell` topology failure is in the synthetic catalog.

# Milestone 20 completion — 2026-09-26

Completed the existing-tessellation milestone: tessellated edges, vertices and
connecting-edge identity; presentation tessellations as placed graphics;
tessellated-root and annotation discovery with representation selection; C/C++ entry
points; and the corpus survey of both. See
[EXISTING_TESSELLATIONS.md](EXISTING_TESSELLATIONS.md). The decoder gained the opt-in
`OmittedSlot::allow_value` mode. Benchmarks were not run locally: they run
automatically in the nightly CI workflow, which now also covers the presentation
measurement.

Observed locally on macOS arm64, rustc 1.98.1:

| Check | Result |
| --- | --- |
| `python3 scripts/check_kernel.py` (`cargo test --workspace`) | 249 tests passed |
| Workspace fmt, Clippy (all targets and features) and rustdoc with warnings denied | passed |
| Architecture, authored fixture provenance and generated conformance | passed; 19 packages, 98 fixtures |
| Python verification tests | 43 passed |
| CLI, EXPRESS, schema, product and metamorphic checks | passed |
| `check_geometry.py --require-external` | 66 reviewed outcomes passed, including 7 authored tessellated fixtures, 2 authored annotation roots, the selection survey and 5 hash-pinned exporter files |
| `check_capi.py` (installed package, without sanitizers) | passed; exactly 50 C exports; relocation, Debug/Release, C-only and C++ consumers |
| `cargo check --manifest-path fuzz/Cargo.toml --locked` (unnested copy) | passed with the new `presentation_import` target; no coverage-guided run |

`python3 scripts/corpus.py --check` first failed on 16 synthetic `dodgy-step-files`
inputs. Their tessellated roots failed at the profile stage, and the survey reported
the tessellation stage as `not_run` where the new protocol requires `not_applicable`.
The survey was corrected. The rerun then showed 52 baseline changes against unchanged
physical outcomes (2,821 clean, 38 reference-error and 368 rejected unique inputs).
Each change was reviewed; none is a lost outcome for a previously surveyed root:

| Change | Inputs |
| --- | --- |
| Annotation occurrences newly measured | 32 |
| Tessellated-only files from `no_shape_roots` to `accepted` (NIST FTC-08 copies, CATIA cuboid, HOOPS shell, one synthetic) | 5 |
| Synthetic files from `no_shape_roots` to `rejected`: malformed tessellated roots rejected at profile | 14 |
| `M025.stp` from `accepted` to `partial`: its B-rep root is unchanged; its newly surveyed tessellated alternative is malformed | 1 (flagged as a regression) |

The baseline was then saved and `--check` passes against it: 3,227 compatible unique
inputs in 224 s. Across unique inputs, 1,197 shape roots in 584 files; 718 pass
geometry/topology and 564 produce meshes (557 before). 7 of 24 tessellated roots
import (every real exported one; the rest are synthetic), and 2 roots are selected
alternatives. 1,359 of 1,366 annotation occurrences in 35 files import (1,360 with
context units): 29,724 polylines and 533,644 fill triangles, including 830 zero-area
triangles. The 7 failures are synthetic files (5) and two over-riding-style complex
instances outside the profile.

# Existing tessellation import (Milestone 20) — 2026-09-26

Added selected-root import of `TESSELLATED_SOLID`, `TESSELLATED_SHELL` and
triangulated surface sets built from `TRIANGULATED_FACE` / `COMPLEX_TRIANGULATED_FACE`
over shared `COORDINATES_LIST`s, directly into owned meshes without retessellation.
Opt-in decoder link slots retain B-rep provenance links without decoding them. See
[EXISTING_TESSELLATIONS.md](EXISTING_TESSELLATIONS.md). Rust only; no C/C++ entry point
is claimed yet. The checks below ran after merging the strict import policy and corpus
solid-import stage from `main`.

Observed locally on macOS arm64, rustc 1.98.1:

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | 217 tests/doctests passed |
| Workspace fmt, Clippy and rustdoc with warnings denied | passed |
| Architecture, authored fixture provenance and generated conformance | passed; 19 packages, 66 fixtures |
| Python verification tests | 41 passed |
| CLI, EXPRESS, schema, product and metamorphic checks | passed |
| `check_geometry.py --require-external` | 28 reviewed outcomes passed, including 4 authored tessellated fixtures and 3 hash-pinned exporter files |
| `cargo check --manifest-path fuzz/Cargo.toml --locked` (unnested copy) | passed with the new `tessellated_import` target; no coverage-guided run |
| `cargo bench -p tessstep-import --bench tessellated` | 120,000 triangles, 28 imports in 2.03 s (about 72 ms each) |

`python3 scripts/corpus.py --check` passed: 3,230 paths / 3,227 unique inputs in
97.2 s, with 2,821 clean, 38 reference-error and 368 rejected physical outcomes, all
compatible with the reviewed baseline and zero baseline changes. The solid-import
stage is unchanged (33 accepted, 3 partial, 382 unsupported, 146 rejected, 1 resource
limit, 2,294 without solid roots), so link slots do not alter planar decoding. That
stage does not yet survey tessellated roots; the baseline was not refreshed.

A separate sweep ran the `tessellated` example over every tessellated root in the
external corpus, at an assumed 0.001 metres per unit, with surface sets capped at
three per file:

| Root type | Accepted | Rejected / unsupported |
| --- | --- | --- |
| `TESSELLATED_SOLID` | 4 (three NIST FTC-08 copies, CATIA cuboid) | 10 synthetic adversarial files: 9 at profile (arity, empty sets), 1 open solid |
| `TESSELLATED_SHELL` | 5 (three NIST copies, HOOPS shell, one synthetic shell whose link disagrees with its tessellation) | 7 synthetic adversarial files, all at profile |
| `COMPLEX_TRIANGULATED_SURFACE_SET` | 19 | 31 supplied normal opposes winding, 2 degenerate triangles, 2 `pnmax`/`npoints` mismatches, 3 arity; 6 more fail physical parsing |
| `TRIANGULATED_SURFACE_SET` | 0 | 1 references an entity outside the profile |

Every real exported shape tessellation imports. The rejected surface sets are
mostly PMI glyph fills, whose winding disagrees with their single annotation-plane
normal. Presentation tessellations are outside this milestone.

# Strict/tolerant planar import policy and baseline refresh — 2026-09-26

The planar importer now accepts `$` in the derived `ORIENTED_EDGE` endpoint slots
by default. The slots are derived, so their values are never read. An explicit
strict switch rejects `$` there: `ImportOptions::strict` in Rust, a new additive
16-byte `ts_import_policy` record with `TS_IMPORT_STRICT` in C, and `ImportPolicy`
in C++. Explicit vertex references in those slots stay rejected in both modes.
The frozen 80-byte options layout is unchanged. `ImportLimits` was renamed
`ImportOptions`.

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | 210 tests/doctests passed |
| Workspace fmt, Clippy and rustdoc with warnings denied | passed |
| Planar/faceted file-stage report with `--require-external` | 21 reviewed outcomes passed, including strict and tolerant runs of the `$` fixture |
| Installed C11/C++17 consumers (without sanitizers; see below) | passed; relocated Debug/Release and C-only packages |
| ABI symbol check | exactly 40 C symbols; policy record 16 bytes; previous layouts unchanged |

Local ASan runs hang inside the sanitizer runtime's own initialization on this
macOS 27 host, before consumer code runs. Sanitized consumers remain covered by CI.

Before the refresh, `python3 scripts/corpus.py --check --repeat 2` against the
previous baseline passed with unchanged physical outcomes (2,821 clean, 38
reference-error, 368 rejected) and no regressions. All 3,227 entries changed only
because the geometry stages were newly measured. The tolerance moved 177 roots
past the profile stage, but all of them then fail topology checks: 80 open shells,
31 inconsistent orientations, 31 broken wires among the reported roots, all in the
deliberately defective dodgy-step-files set. Accepted roots stay at 30. After this
review, `--save-baseline` replaced `corpus/baseline.json`; a following `--check` ran
with zero baseline changes, and `fetch_corpus.py` verification matches the inventory.

# Corpus solid import stage and named profile diagnostics — 2026-09-26

The external corpus runner now imports every `MANIFOLD_SOLID_BREP`,
`BREP_WITH_VOIDS` and `FACETED_BREP` root with the existing selected-root
profiles, through the `tessstep-import` `survey` example. Per-root profile,
geometry/topology and tessellation outcomes replace `not_integrated`; see
[corpus testing](corpus-testing.md#solid-import-stage). Profile rejection of an
entity type now names it (every component of a complex instance, marking those
outside the profile) as outside the import profile, not as unknown to STEP.
No new geometry support is claimed.

Observed locally on macOS arm64, rustc 1.98.1:

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | 208 tests/doctests passed |
| Workspace fmt, Clippy and rustdoc with warnings denied | passed |
| Architecture, authored fixture provenance and generated conformance | passed; 19 packages, 61 fixtures |
| Python verification tests | 41 passed |
| Planar/faceted file-stage report with `--require-external` | 19 reviewed outcomes passed |

`python3 scripts/corpus.py --check --repeat 2` passed: 3,230 paths / 3,227 unique
inputs in 251.8 s, with 2,821 clean, 38 reference-error and 368 rejected physical
outcomes, unchanged from the reviewed baseline. Every input reports a `changed`
baseline entry because the geometry stages are newly measured; there are no
regressions, crashes or timeouts. The baseline was not refreshed.

At the assumed 0.001 metres per source unit, the survey examined 2,859 physically
accepted inputs. 565 contain 1,173 solid roots; 38 roots pass profile, geometry and
topology and 30 produce solid meshes (file stages: 33 accepted, 3 partial, 382
unsupported, 146 rejected, 1 resource limit, 2,294 without solid roots). Root
failures by first stage: 922 profile-unsupported, 194 profile-invalid, 6 missing
entities, 12 geometry/topology, 8 tessellation, 1 resource limit.

The dominant categories are curved geometry outside the planar profile
(`CYLINDRICAL_SURFACE` 190, `CIRCLE` 149, `SURFACE_CURVE` 117,
`B_SPLINE_CURVE_WITH_KNOTS` 92, complex rational B-spline curves 73, `CONICAL_SURFACE`
56, complex rational B-spline surfaces 53, `SPHERICAL_SURFACE` 52) and 185 roots
whose `ORIENTED_EDGE` endpoint slots hold `$` instead of `*`. The slowest input took
1.5 s for the survey. Surface models (`SHELL_BASED_SURFACE_MODEL`) are not surveyed.

The records below describe earlier validation snapshots.

# Edge-based planar STEP import — 2026-09-26

Added selected-root `MANIFOLD_SOLID_BREP` import with `ADVANCED_FACE`, `PLANE`,
`EDGE_LOOP`, `ORIENTED_EDGE`, straight `EDGE_CURVE` and `VERTEX_POINT` topology.
The adapter preserves shared entity identity, trims the declared LINE geometry,
composes edge/bound/face orientation and rejects contradictions without welding
or healing. Explicit physical-profile metadata accepts only `*` in the two derived
ORIENTED_EDGE endpoint slots; ordinary schema decoding remains strict.
See [STEP_IMPORT.md](STEP_IMPORT.md) for scope and usage.

Observed locally on macOS arm64:

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | 207 tests/doctests passed |
| Import/C ABI release tests | 21 passed |
| Workspace fmt, Clippy and rustdoc with warnings denied | passed |
| Architecture, authored fixture provenance and generated conformance | passed; 19 packages, 61 fixtures |
| Python verification tests | 37 passed |
| Planar/faceted file-stage report | 19 reviewed outcomes passed, including the required external exporter case |
| Installed C11/C++17 consumers with ASan/UBSan | passed; relocated Debug/Release and C-only packages |
| ABI symbol/layout checks | 38 C symbols; planar options reuse the 80-byte options layout; existing layouts unchanged |

The unmodified ST-DEVELOPER-exported `foxtrot/examples/cuboid.step`, selected at
root #121 with metre units, produces 8 vertices and 12 outward-facing triangles.
Its extents are 0.0508 × 0.0254 × 0.0762 m and its volume is 0.000098322384 m³.
Rust and the installed C++ package both verify these values and source face IDs;
retained mesh views survive document and mesh-owner destruction. The pinned
external input hash is recorded in `corpus/geometry/external-planar.json`.
The STEP file remains outside the repository and distributable package.

Authored tests cover holes, nontrivial LINE parameter ranges, reversed edge/bound/
face orientations, record ordering, finite budgets and 300 bounded byte mutations.
Negative cases cover off-line endpoints, contradictory sense, invalid vector
magnitudes, broken/shared topology, invalid derived-slot markers and unsupported
curved geometry. Both reduced profiles reproduce their generated metadata.
The CI corpus job now requires the exporter case and runs the installed C++ test.
Hosted cross-platform/MSRV CI has not been observed here. Native sanitizers
instrument the consumers, not Rust internals; mutation smoke tests are not an
instrumented fuzz campaign.

`python3 scripts/corpus.py --check` passed for run
`2026-09-26T08-33-33-474917Z`: 6,438 paths / 3,227 unique inputs in 33.145 s,
with 2,821 clean, 38 reference-error and 368 rejected outcomes. The per-file report
and representative clean/rejected/reference-error entries were inspected: zero
changes from the prior run or reviewed baseline. The baseline was not refreshed.
The full corpus runner still reports schema/product `not_configured` and geometry/
tessellation `not_integrated`; selected-root conversion evidence is in the separate
[geometry report](../reports/geometry/index.html). Curved import, automatic root/
unit/assembly interpretation and general AP validation remain outside this slice.

The records below describe earlier validation snapshots.

# First planar faceted STEP import — 2026-09-26

Implemented selected-root `FACETED_BREP` import through a generated reduced
structural profile, metre-normalized planar geometry, canonical shared topology,
linear pcurves and the existing solid mesh pipeline. Rust, C and C++ expose the
same conversion. See [STEP_IMPORT.md](STEP_IMPORT.md) for the exact support contract.
This is not general curved STEP import, automatic assembly/unit interpretation or
an AP conformance claim.

Observed locally on macOS arm64:

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | 200 tests/doctests passed, zero failures in the shared workspace snapshot |
| Import/C ABI release tests | 15 passed |
| Workspace fmt, Clippy and rustdoc with warnings denied | passed |
| Architecture, authored fixture provenance and generated conformance | passed; 19 packages, 53 fixtures |
| Python verification tests | 37 passed |
| Faceted file-stage report | 11 reviewed outcomes passed |
| Installed C11/C++17 consumers with ASan/UBSan | passed; relocated Debug/Release and C-only packages |
| ABI symbol/layout checks | 36 C symbols; two additive functions, 80-byte options and 32-byte error record; previous layouts unchanged |

The authored box produces 8 vertices / 12 triangles, volume 0.000006 m³; the
through-hole fixture produces 16 vertices / 32 triangles, volume 0.00000384 m³.
Independent checks cover bounds, outward normals, hole exclusion, original STEP
face IDs, shared vertex/edge identity, reversed bounds/faces, placement defaults,
record-order invariance, strict invalid-input rejection and finite budgets.
A 500-case deterministic byte-mutation smoke run completed without panics.
The C/C++ consumers verify meshes and retained views outlive their source documents.
Native ASan/UBSan instruments consumers, not Rust library internals. No new hosted
cross-platform/MSRV run or instrumented import fuzz campaign is claimed.

`python3 scripts/check_geometry.py` verifies that the checked-in profile exactly
matches generated EXPRESS metadata. [Per-file results](../reports/geometry/index.html)
and [JSON](../reports/geometry/latest.json) distinguish profile, geometry/topology
and tessellation. Two authored solids are accepted; five authored cases are rejected
or unsupported as expected. Four external adversarial faceted inputs also produce
no mesh. These checks do not establish successful general CAD-exporter import.

`python3 scripts/corpus.py --check` passed. Run
`2026-09-25T21-33-35-171537Z` examined 6,438 paths / 3,227 unique contents in 35.725 s:
2,821 clean, 38 reference-error, 368 structured rejections; no crashes, timeouts or
runner errors. The per-file report and both comparison sets were inspected: zero
changes from the previous run or reviewed baseline. Representative observations
remain clean `Ctl019.stp`, reference-error `Pf037.stp`, and rejected `Ctl001.stp`.
The baseline was not refreshed. The full external runner still reports schema and
product as `not_configured`, geometry and tessellation as `not_integrated`; the
separate selected-root geometry report must not be conflated with those stages.

The records below describe earlier validation snapshots.

# Milestone 5 product placement completion — 2026-09-25

Completed the bounded local 3D product slice: evaluated item-defined and Cartesian
placements, mapped reuse, indirect item/context membership, shape associations,
SI-normalized uncertainties and iterative nested assembly expansion. See
[PRODUCT_MODEL.md](PRODUCT_MODEL.md) for transform direction, selection rules and
unsupported forms. No public C/C++ product interface or automatic STEP-to-mesh
binding is claimed.

Observed locally on macOS arm64:

| Check | Result |
| --- | --- |
| Locked workspace tests and doctests, isolated build directory | passed |
| Focused product/adapter debug and release tests | 17 passed |
| Workspace formatting, Clippy and rustdoc with warnings denied | passed |
| Authored product corpus and numerical assertions | 11 fixtures passed |
| Deterministic mutation and depth checks | 1,500 adapter/expansion cases; 2,000-level assembly |
| Product fuzz target | build and warnings-denied checks passed; 1,000 seeded driver runs without crashes |
| Python tests, provenance, architecture and generated conformance | 21 tests, 45 fixtures, 18 packages; passed |

The final workspace run used `cargo test --workspace --locked --target-dir
/tmp/tessstep-m5-verification`; the same isolated directory was used sequentially for
Clippy and rustdoc. The stable fuzz driver run had no coverage or sanitizer
instrumentation and is not an instrumented fuzz campaign. No new hosted cross-platform
CI or MSRV run was observed here.

Authored corpus run `2026-09-25T20-52-32-308996Z` accepted physical syntax and schema
decoding for all 11 fixtures. Product adaptation accepted six, correctly rejected four
(degenerate axis, cycle, missing units and negative uncertainty), and reported the 2D
context fixture as unsupported. Numerical assertions cover mixed-unit translations,
reflections/nonuniform scaling and uncertainty normalization. The per-file report's
17 changed entries reflect replacing six earlier fixture contents with actual
placements and adding five fixtures; their manifest hashes and expected outcomes were
reviewed. Geometry and tessellation remain `not_implemented` for these product inputs.

External corpus run `2026-09-25T20-57-11-044411Z` inspected **6,438 paths / 3,227 unique
contents** in 35.91 seconds: **2,821 clean, 38 reference-error, 368 structured rejections**;
no crashes, timeouts or runner errors. `python3 scripts/corpus.py --check` passed.
The per-file progress report and both comparison sets were inspected: **zero changes**
from the prior run or baseline. Representative outcomes remain clean `Ctl019.stp`,
reference-error `Pf037.stp` (TS1103), and rejected `Ctl001.stp` (TS1010).
`corpus/baseline.json` was not refreshed. External schema, product, geometry and
tessellation stages remain `not_implemented` without configured AP validators;
physical parsing success does not establish semantic or tessellation success.

The optimized product benchmark adapted 30,037 entities (1,520,210 bytes) in
16.230 ms/input and expanded 10,003 instances in 3.494 ms/input. These are local
observations, not statistical comparisons; the expanded fixture differs from the
earlier benchmark. Full AP adaptation, external assembly resolution, non-immediate
usage forms, 2D contexts and automatic geometry binding remain outside this slice.

The records below describe earlier verification snapshots.

# Milestone 19 appearance — 2026-09-25

Implemented an independent retained appearance layer with validated linear RGBA
materials, exact asset/face bindings, inherited occurrence overrides, winning-source
provenance and allocation-free triangle queries. C/C++ interfaces preserve shared
scene/mesh ownership. See [APPEARANCE.md](APPEARANCE.md) for the explicit precedence
policy and limits; STEP presentation/style decoding is not claimed.

Observed locally on macOS arm64:

| Check | Result |
| --- | --- |
| Locked workspace tests, separate build directory | 191 passed, including doctests |
| Mesh/C ABI locked release tests | 23 passed |
| Appearance package fmt; workspace Clippy and rustdoc with warnings denied | passed |
| Architecture and generated conformance | passed; 18 packages |
| Installed C11/C++17 consumers with native ASan/UBSan | passed; relocated Debug/Release and C-only packages |
| ABI exports/layouts | exactly 34 C symbols; previous records and status values unchanged |
| Appearance regressions | six tests: precedence, ancestry, transparency/absence, face identity, shared storage, reflected baking, invalid inputs and budgets |
| Mutation/depth evidence | 2,000 deterministic binding/color cases with independent parent-walking oracle; 10,000-deep inheritance |

The full suite used `cargo test --workspace --locked --target-dir target/m19-verification`
to avoid interference from concurrent edits/builds in the shared default target directory.
Clippy/rustdoc used a separate `target/m19-lint` directory. These counts are the observed
workspace snapshot, including product work present during this run. No new hosted
cross-platform CI or instrumented appearance libFuzzer campaign was run here; native
consumer ASan/UBSan does not instrument Rust library accesses.

`cargo bench -p tessstep-mesh --bench appearance` measured 20 appearance constructions
for a 10,000-occurrence assembly in 29.604 ms total, and 1,000,000 allocation-free
triangle queries in 75.300 ms total. These are local optimized observations, not
statistical comparisons, renderer measurements or large-assembly memory guarantees.

External corpus run `2026-09-25T20-44-07-464938Z` inspected **6,438 paths / 3,227 unique
contents** in 32.210 seconds: **2,821 clean, 38 reference-error, 368 structured rejections**;
no crashes, timeouts or runner errors. `python3 scripts/corpus.py --check` passed. The
per-file report and both baseline/previous-run change sets were inspected: no changes.
Representative observations remain clean `Ctl019.stp`, reference-error `Pf037.stp`
(TS1103) and rejected `Ctl001.stp` (TS1010). The baseline was not refreshed. All external
inputs still reported schema, product, geometry and tessellation as `not_implemented`
without configured validators; these independent appearance APIs enable no STEP style
stage. STEP style adaptation, surface-side styling, textures, lighting models and
rendering remain unsupported by this slice.

The records below describe earlier milestones and their verification snapshots.

# Milestone 18 assembly assets — 2026-09-25

Implemented independent shared mesh assets, explicit nested occurrence placement and
checked world-space baking, with additive C/C++ scene operations. Scope and limits are
in [ASSEMBLY_ASSETS.md](ASSEMBLY_ASSETS.md); STEP placement decoding remains pending.

Observed locally on macOS arm64:

| Check | Result |
| --- | --- |
| Workspace debug tests | 178 passed, including doctests |
| Mesh/C ABI release tests with locked dependencies | 16 passed |
| Workspace fmt, Clippy and rustdoc with warnings denied | passed |
| Architecture and generated conformance | passed; unchanged 18-package graph |
| Installed C11/C++17 consumers with native ASan/UBSan | passed; relocated Debug/Release and C-only packages |
| C symbols and layouts | exactly 25 exports; previous ABI-1 layouts/statuses unchanged |
| New scene tests | buffer sharing, noncommuting nested placement, reflections, nonuniform scale/shear, corner provenance, graph/numeric/budget errors |
| Bounded mutation/depth tests | 2,000 graph/transform cases and a 10,000-deep forest; passed |

`cargo bench -p tessstep-mesh --bench scenes` measured 20 constructions of 10,000
shared occurrences in 85.674 ms total and 10,000 explicit single-triangle bakes in
13.362 ms total. These are local optimized observations, not statistical comparisons,
large-part memory profiles or industrial performance guarantees. No new instrumented
scene libFuzzer campaign or hosted cross-platform CI run was performed here.

External corpus run `2026-09-25T20-28-02-201951Z` inspected **6,438 paths / 3,227 unique
contents** in 33.426 seconds: **2,821 clean, 38 reference-error, 368 structured rejections**;
no crashes, timeouts or runner errors. `python3 scripts/corpus.py --check` passed.
The generated per-file JSON/HTML report, baseline changes and previous-run changes
were inspected: both change sets are empty. Representative inputs retain clean
`Ctl019.stp`, missing-reference `Pf037.stp` (TS1103) and rejected `Ctl001.stp` (TS1010).
The baseline was not refreshed. All 3,227 inputs still report schema, product, geometry
and tessellation as `not_implemented` without configured adapters/validators; independent
scene tests do not establish STEP assembly or STEP-to-mesh acceptance.

The records below describe earlier milestones and their verification snapshots.

# Milestones 15–17 adaptive tessellation and mesh validation — 2026-09-25

This delivery implements bounded regular analytic/NURBS face refinement, synchronized
boundary refinement and owned manifold shell/solid meshes. It also adds C/C++ triangle
import and retained read-only mesh views. Contracts are in [TESSELLATION.md](TESSELLATION.md),
[MESH.md](MESH.md) and [C_API.md](C_API.md). STEP geometry adapters, singular pole/apex
repair, exact predicates, self-intersection/material proofs and cavity shells remain open.

Observed locally on macOS arm64:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | 172 passed, including doctests |
| Tessellation/mesh/topology release tests | 39 passed, including three compile-fail doctests |
| Workspace/fuzz fmt and Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture and generated conformance | passed; 18 packages |
| Python corpus/CI tests | 21 passed |
| Installed C11/C++17 consumers with native ASan/UBSan | passed; relocated Debug/Release and C-only packages |
| ABI exports and layouts | exactly 17 C symbols; existing ABI-1 layouts unchanged; new 40/80-byte mesh records |
| Combined stable mutation harness | 3,000 mutations plus 582 prefixes, now including adaptive owned meshes |
| Updated libFuzzer driver | 1,000 stable-built smoke runs completed without crashes (109 seconds) |

Nine adaptive regression tests use independent dense barycentric grids on output
triangles, exercising cylinder/sphere/cone/torus regular patches, reversed planar holes,
rational NURBS bumps, a narrow knot-span feature, tighter tolerances, shared refinement
between curved/planar faces, per-corner sharp normals, caps and both seams of a closed
torus. Closed-cylinder/torus tests check manifold closure and approximate known volume;
inward orientation is rejected. Limits, duplicate face requests and invalid handles
fail explicitly. Mesh tests independently reject invalid indices, degenerate/duplicate
triangles, winding conflicts, malformed attributes, unused and pinched vertices, and
distinguish open/disconnected meshes from solids.

A topology regression confirms that closed circular edges retain distinct endpoint
incidences at the same vertex. This fixes cap/seam shells while the existing invalid-link
regressions continue to pass. C/C++ tests verify copied input ownership, stable view
pointers, retained views surviving their mesh wrapper, safe moves, error mapping,
concurrent reads and balanced release. Native sanitizers do not instrument the Rust
library. The stable-built libFuzzer driver warned about absent sanitizer/coverage
instrumentation; its 1,000 runs are driver smoke only. Cross-platform/MSRV CI and
extended instrumented hardening were not observed.

## External corpus

`python3 scripts/corpus.py --check` passed in **34.542 seconds**, run
**2026-09-25T19-28-43-044904Z**: **6,438 paths / 3,227 unique inputs**, with
**2,821 clean**, **38 reference-error** and **368 structured-rejection** outcomes.
There were no crashes, timeouts, runner errors or outcome/diagnostic changes against
the previous run or reviewed baseline. Stage totals and representative per-file entries
for all three outcomes were inspected in [the generated report](../reports/corpus/index.html).

All external entries still report schema/product/geometry/tessellation as `not_implemented`:
no AP validators are configured and constructed-geometry tessellation does not supply a
STEP adapter. No external geometry acceptance is inferred. `corpus/baseline.json` is
unchanged. The implemented geometry and mesh stages have independent constructed-input
regressions rather than fabricated STEP-stage success.

## Benchmark and architecture review

`cargo bench -p tessstep-tessellate --bench boundaries --locked` reports one-second
local observations after warmup, with geometry construction/normalization outside timing:

| Operation | Iterations | Microseconds/iteration |
| --- | ---: | ---: |
| Closed-cylinder solid, including shared sampling and cap/trim checks | 23 | 44,499.567 |
| NURBS bump adaptive tessellation | 263 | 3,805.205 |

Both new cases use 0.01 metre chord / 0.25 radian angular tolerance. These are initial
observations, not statistical comparisons or STEP-to-mesh throughput guarantees.

The mesh crate activates the reserved math-only dependency boundary. Tessellation shares
UV triangulation, performs bounded conforming refinement and preserves canonical sample
identity through position assembly. Ordered maps and iterative walks retain determinism.
Mesh and C bridge storage stay distinct; C callers see only specified scalar buffers and
opaque ownership. No third-party production dependency or unsafe kernel code was added.
Public C B-rep/tessellation inputs and STEP adaptation remain separate future work.

---

# Milestone 14 planar tessellation validation — 2026-09-25

This delivery adds constrained triangulation of constructed analytic-plane faces.
Concave outlines, multiple holes, collinear vertices and sampled curved boundaries retain
canonical edge-cache positions. See [TESSELLATION.md](TESSELLATION.md) for API, ownership,
resource and numerical limits. Curved-face refinement, owned mesh assets, C/C++ mesh
views, STEP geometry adaptation and watertight solids remain pending.

Observed locally on macOS arm64:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | 157 passed, including doctests |
| Topology/trim/tessellate release tests | 34 passed, including three compile-fail doctests |
| Workspace and fuzz fmt / Clippy with warnings denied | passed |
| Workspace rustdoc with warnings denied | passed |
| Architecture and generated conformance checks | passed |
| Stable combined pipeline mutations | 3,000 mutations plus 388 prefixes passed; now includes planar triangles |
| Updated libFuzzer driver | built with locked dependencies; 1,000 smoke runs, no crash |

Nine planar regression tests verify known areas, independent interior probes, boundary
segment incidence, deterministic output, concavities, multiple hole orders, circles and
an annulus, tilted planes, reversed orientation, shared interior sample identity and
limits. Forty varied concave polygon/hole fixtures exercise bridge/ear selection. A
coarse circle/hole fixture passes smooth trim reconstruction but fails mesh-resolution
polygon validation, then succeeds after shared-edge refinement. A compile-fail doctest
checks that face meshes cannot outlive the edge cache. The trim polygon API separately
checks invalid coordinates, winding, holes and budgets.

The stable-built libFuzzer run warned about absent sanitizer/coverage instrumentation;
it is driver smoke, not an instrumented hardening campaign. The deterministic mutation
suite remains the full-size bounded-input regression check. No new C/C++ operation was
added; the existing C ABI tests passed within the workspace suite. Installed-package
consumer tests were not rerun for this Rust-only API addition.

## External corpus

Final `python3 scripts/corpus.py --check` run: **2026-09-25T19-04-37-108617Z**,
34.234 seconds, **6,438 paths / 3,227 unique inputs**. Results remain **2,821 clean**,
**38 with reference errors**, and **368 structured rejections**. There were no crashes,
timeouts, runner errors, outcome changes or diagnostic changes against either the prior
run or the reviewed baseline. Stage counts and representative per-file entries for all
three outcomes were inspected in [the report](../reports/corpus/index.html).

All 3,227 inputs still report schema/product/geometry/tessellation as `not_implemented`:
no AP validators are configured and no STEP geometry adapter/checker exists. Constructed
planar triangulation is not presented as external STEP tessellation coverage.
`corpus/baseline.json` was not modified.

## Benchmark and architecture review

The boundary benchmark now includes planar disk triangulation from cached edges,
including trim reconstruction and polygon validation: **21,881.108 microseconds per
iteration**, 46 iterations over one second on this machine. This is an initial local
observation, not a statistical comparison or a STEP-to-mesh throughput claim.

The surfaces dependency moves from development-only to the already permitted production
edge; the fuzz lockfile records that local edge. No third-party production dependency,
unsafe kernel code, C symbol or ABI layout was introduced. Polygon relationship checks
are shared with trimming, and triangle results borrow the cache with immutable access.
Bounded iterative bridges/ears and ordered incidence postchecks preserve deterministic
output. Exact predicates, triangle-quality optimization and worst-case performance
improvements remain future work; difficult cases return typed failures.

---

# Milestones 11–13 boundary pipeline validation — 2026-09-25

The existing Milestones 8–10 work was checkpointed as `52c752c` after the workspace
suite passed. This delivery adds independent structural topology states (11), bounded
supplied-pcurve UV reconstruction (12), and shared-edge boundary sampling (13).
Contracts and limits are in [TOPOLOGY.md](TOPOLOGY.md), [TRIMMING.md](TRIMMING.md),
and [TESSELLATION.md](TESSELLATION.md). Planar face triangulation is next; no STEP
geometry adaptation, triangle mesh or watertight-solid result is claimed.

Observed locally on macOS arm64 with Rust 1.95.0:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | 146 passed, including doctests |
| New topology/trim/tessellate release tests | 23 passed, including two compile-fail doctests |
| Workspace/fuzz formatting and Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture/conformance/authored fixture provenance | passed; 17 packages and 40 fixtures |
| Python corpus/CI tests | 21 passed |
| Relocated C/C++ package consumers | passed; Debug/Release, ABI exports/layout, ownership and concurrency |
| Stable combined boundary mutation harness | 3,000 random mutations plus 388 prefixes passed |
| New libFuzzer driver | built; 1,000 smoke runs completed without crashes |

Topology tests check shared identity without geometry edits, malformed references and
ownership, endpoint mismatch, wire closure, closed/open/disconnected shells, conflicting
orientations, excess edge incidence and pinched vertex links. UV tests exercise outer
and hole classification, analytic and NURBS boundaries, full cylinder seams, integer
chart shifts, reversed parameter intervals, one-sided discontinuities, singular surfaces,
missing/mismatched pcurves, invalid winding and noncontractible loops. Resource limits
are failure paths, not successful partial output.

Sampling tests verify zero-copy reversed/shared/seam views, exact canonical vertex
coordinates, circle sagitta/tangent criteria, tighter-tolerance refinement, rational
arc dense-probe error, deterministic output, NURBS knot corners/discontinuities,
collapsed curves, endpoint tolerance failures and face-specific UV mappings.
These checks establish the documented constructed-input behavior, not global continuous
error bounds, exact intersection predicates or AP conformance.

The libFuzzer binary used stable Rust and four original 96-byte seeds. It warned about
missing sanitizer/coverage instrumentation and no interesting coverage; the 1,000 runs
are driver smoke evidence only. Full-size arbitrary mutations are covered separately by
the stable harness. An instrumented nightly CI target is configured but was not observed
here. Hosted cross-platform/MSRV CI and long numerical-hardening campaigns remain open.

## External corpus

Both `python3 scripts/corpus.py --check` runs passed; the reviewed baseline was unchanged:

| Checkpoint | Run | Seconds |
| --- | --- | ---: |
| Initial integrated boundary pipeline | 2026-09-25T18-36-39-590816Z | 33.857 |
| Final boundary pipeline | 2026-09-25T18-46-06-520711Z | 32.926 |

Each report contains **6,438 paths / 3,227 unique inputs**: **2,821 clean**, **38 with
reference errors**, and **368 structured rejections**. Outcome/diagnostic change lists
against the previous run and reviewed baseline are empty. There were no crashes,
timeouts or runner errors. Per-file stage counts and representative clean,
reference-error and rejected entries were inspected in the final report.

All 3,227 external inputs still report schema/product/geometry/tessellation as
`not_implemented`: no AP validators are configured and no STEP geometry adapter/checker
exists. Independent boundary tests are not relabeled as STEP geometry acceptance.
See the generated [per-file HTML report](../reports/corpus/index.html) and
[JSON results](../reports/corpus/latest.json). `corpus/baseline.json` was not refreshed.

## Benchmark and architecture review

`cargo bench -p tessstep-tessellate --bench boundaries --locked` uses an original
cylinder-strip fixture, one warmup and one-second samples. Geometry construction is
outside timing; raw-model cloning is included in validation timing.

| Operation | Iterations | Microseconds/iteration |
| --- | ---: | ---: |
| Structural validation and normalization | 1,535,261 | 0.651 |
| UV seam reconstruction | 265,717 | 3.763 |
| Shared-edge sampling and face boundary mapping | 4,825 | 207.265 |

These are local observations, not statistical comparisons or whole-model tessellation
measurements. The architecture review confirms the reserved downward crate graph,
immutable private validity states, explicit model/cache borrows, deterministic iterative
walks, logical resource budgets and quadratic bounded polygon comparisons. No unsafe
kernel code, third-party production dependency or C ABI symbol/layout was introduced.
Public C/C++ geometry and mesh APIs remain pending under the existing release contract.

---

# Milestones 8–10 surface/NURBS validation — 2026-09-25

The verified analytic-curve milestone was committed as `7758269`. This work implements
independent analytic surfaces (8), bounded positive-weight NURBS curves (9), and
bounded tensor-product NURBS surfaces (10). See [SURFACES.md](SURFACES.md) and
[NURBS.md](NURBS.md) for parameterizations, published algorithms and supported limits.
STEP geometry adapters, topology, trimming and tessellation remain pending.

Observed locally on macOS arm64 with Rust 1.95.0:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | passed, 123 tests including doctests |
| Curve/surface release tests | passed, 27 tests including doctests |
| Workspace/fuzz formatting and Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture/conformance/authored fixture provenance | passed; 14 packages, 40 fixtures |
| Python corpus/CI tests | 21 passed |
| Relocated C/C++ package consumers | passed; Debug/Release, ABI layout/exports, ownership and concurrency |
| Two new stable bounded geometry/spline smoke harnesses | 5,641 cases each: 5,000 arbitrary-bit, 512 mutations, 129 prefixes |
| Two new libFuzzer driver smoke runs | 1,000 runs each, no crashes |

Analytic tests independently compare all first/second partials with finite differences,
check known points/loci, parameter domains, periodic seams and exact pole/apex singularities.
NURBS tests include a rational quarter circle and cylinder, hand-calculated weighted
bilinear partials, mixed-derivative checks, one-sided repeated knots, nonclamped/periodic
representations, degree 16, affine/weight-scale invariance and refinement preservation.
Invalid shapes, knots, counts, weights, domains and logical resource budgets are tested.
Surface refinement with unequal weights across rows verifies that normalization remains
common to the whole net. A checked rectangular layout prevents dimension-product overflow.

Both libFuzzer drivers were compiled with stable Rust and each loaded one 128-byte
seed. They completed but reported missing coverage/sanitizer instrumentation and no
interesting coverage. These are driver smoke checks only; full-size mutation/random
inputs are exercised independently by the stable harnesses. Nightly CI adds instrumented
targets and increases its budget to 35 minutes for the longer target list. Hosted CI,
Rust 1.85, long instrumented campaigns and cross-platform numerical behavior were not
observed locally. No unsafe code, external production dependency or C ABI layout was added.

## External corpus checks

Ran `python3 scripts/corpus.py --check` after the analytic surface and rational curve
steps and after the combined implementation. Each run examined **6,438 paths / 3,227
unique contents** and passed without refreshing `corpus/baseline.json`:

| Stage checkpoint | Run | Seconds |
| --- | --- | ---: |
| Analytic surfaces | 2026-09-25T18-07-55-526622Z | 32.137 |
| Rational curve implementation | 2026-09-25T18-12-44-870866Z | 33.819 |
| Combined surfaces/NURBS | 2026-09-25T18-21-38-235130Z | 34.324 |

Per-file stage counts and comparison lists were inspected for all three runs; representative
clean/reference-error/rejected entries were also inspected in the final report. All runs
retain **2,821 clean**, **38 reference-error**, **368 rejected**, with **zero outcome or
diagnostic changes** versus the previous run and reviewed baseline. There were no crashes,
timeouts or runner errors. See [the final per-file report](../reports/corpus/index.html).

All external inputs still show schema/product/geometry/tessellation as `not_implemented`.
No AP schema/product checker is configured, and no STEP-to-geometry adapter/checker exists.
Independent NURBS tests are not relabeled as STEP geometry acceptance. Real geometry stage
checks must be added with those adapters. Physical acceptance alone establishes none of
the later stages.

## Benchmarks and architecture

`cargo bench -p tessstep-surfaces --bench evaluators --locked` uses one warmup and
one-second samples of 1,000 checked evaluations per batch. Geometry construction is
outside timing; position and all implemented derivative calculations are inside:

| Evaluator | Batches | Microseconds/batch |
| --- | ---: | ---: |
| Analytic torus, position and five partials | 16,797 | 59.536 |
| Rational curve, position and two derivatives | 9,623 | 103.925 |
| Rational surface, position and five partials | 5,736 | 174.347 |

These are local throughput observations, not statistical comparisons, whole-model
benchmarks or numerical-accuracy certificates. The architecture review records shared
iterative basis evaluation, bounded immutable spline storage/refinement, fixed stack
scratch and no raw STEP dependency. Public C/C++ geometry operations remain pending
under the existing ownership/ABI/release-testing contract. Milestone 11 topology validity
states are next; documented Milestone 5 semantic gaps also remain open.

---

# Milestone 7 analytic curve validation — 2026-09-25

Milestone 6 was committed as `cd28385` before this work. The next milestone adds
`tessstep-curves`, depending only on math and reexported as `tessstep::curves`.
It provides independent 2D/3D lines and conics, first/second derivatives, domains,
explicit plane frames, affine evaluation transformation and oriented parameter spans.
See [CURVES.md](CURVES.md) for parameterizations and numerical limits. STEP geometry
adapters, surfaces, NURBS, topology and tessellation remain pending.

Observed locally on macOS arm64 with Rust 1.95.0:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | passed, 106 tests including doctests |
| Analytic curve debug/release tests | passed, 8 integration tests and 2 doctests |
| Frame-space compile-fail example | passed (one of the doctests) |
| Workspace/fuzz formatting and Clippy with warnings denied | passed |
| Workspace rustdoc with warnings denied | passed after fixing an interval rendered as a link |
| Architecture/conformance/fixture provenance | passed; 13 packages, 40 authored fixtures |
| Python corpus/CI tests | 21 passed |
| Relocated C/C++ installed-package consumers | passed; Debug/Release, ABI layouts/exports, ownership and concurrency |
| Deterministic curve arbitrary-float smoke | 10,000 random inputs plus 774 boundary/prefix inputs |
| Curve libFuzzer driver smoke | 1,000 runs completed without crashes |

Independent evidence includes known coordinates and derivatives, implicit conic loci,
central finite differences of positions and first derivatives, 2D orientation and 3D
plane membership, scale-first frames at subnormal/maximum magnitudes, reversed spans,
seam crossing, multiple revolutions, affine covariance and explicit failure cases.
Parabola/hyperbola parameterizations and ellipse axis order are documented and tested.
These are constructed-geometry tests; they do not establish STEP/AP interpretation.

The local libFuzzer driver used stable Rust, no coverage/sanitizer instrumentation and
an empty initial corpus. Runtime warnings confirm those limitations; the full-size
arbitrary-bit cases come from the separate stable harness. Nightly CI configures an
instrumented curve target. No hosted CI, Rust 1.85, extended instrumented fuzzing or
cross-platform numerical behavior was observed locally. No third-party production
dependency, unsafe code, public C symbol or ABI layout was added.

## External corpus

`python3 scripts/corpus.py --check` passed. Run `2026-09-25T17-53-15-066870Z` examined
**6,438 paths / 3,227 unique contents** in **33.837 seconds**. The generated per-file
report, all stage counts, comparison lists and representative clean/reference-error/
rejected entries were inspected. Outcomes remain **2,821 clean**, **38 reference-error**
and **368 rejected**, with **zero changes** against both the previous run and reviewed
baseline. There were no crashes, timeouts or runner errors. `corpus/baseline.json` is
unchanged. See [the external per-file report](../reports/corpus/index.html).

All 3,227 external inputs still report schema/product/geometry/tessellation as
`not_implemented`: no AP schema/product checker was configured, and independent curve
evaluation provides no schema-to-geometry adapter/checker. No geometry acceptance is
inferred from physical parsing. The geometry stage must receive a real checker when
STEP geometry adaptation becomes available.

## Analytic benchmark

`cargo bench -p tessstep-curves --bench analytic --locked` measured 10,000 ellipse
position/first/second-derivative evaluations per batch, with curve construction outside
timing and checked evaluations inside. One warmup and a two-second sample yielded
**6,596 batches / 2.000 seconds / 303.216 microseconds per batch**. This is a local
throughput observation, not a statistical comparison, whole-model benchmark or accuracy
certificate.

The architecture review in [ARCHITECTURE.md](ARCHITECTURE.md) records the independent
math-only dependency, private construction invariants, fixed stack/work costs and the
unchanged C/C++ boundary. Analytic surfaces are the next independent geometry milestone;
STEP curve/placement adapters and the remaining Milestone 5 semantics are still open.

---

# Milestone 6 math foundation validation — 2026-09-25

The verified Milestone 5 product/assembly slice was committed as `d84e5f3`.
The next milestone adds `tessstep-math`, an independent checked coordinate, unit,
tolerance and affine-transform layer, reexported as `tessstep::math`. Its scope and
numerical limitations are in [GEOMETRY.md](GEOMETRY.md) and
[NUMERICAL_ROBUSTNESS.md](NUMERICAL_ROBUSTNESS.md). STEP placement evaluation,
analytic curves/surfaces, NURBS, topology and tessellation remain future work.

Observed locally on macOS arm64, Rust 1.95.0:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | passed, 96 tests including doctests |
| Math debug and release tests | passed, 8 integration tests and 3 doctests |
| Space separation and transform composition compile-fail examples | passed (2 of the doctests) |
| Workspace/fuzz formatting and Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture, conformance and authored-fixture provenance | passed; 12 packages, 40 fixtures |
| Relocated C/C++ installed-package consumers | passed, Debug/Release, layouts, exports, ownership and concurrency |
| Math seeded affine round trips | 2,000 diagonally dominant matrices, both inverse application orders |
| Deterministic arbitrary-float smoke | 10,000 arbitrary-bit cases plus 870 boundary/prefix cases |
| Math libFuzzer driver smoke | 1,000 runs completed without crashes |

The local libFuzzer driver used stable Rust without coverage/sanitizer instrumentation
and started from an empty corpus. Its warnings explicitly report missing instrumentation;
it is only a driver smoke check. The deterministic harness exercises full-size arbitrary
float inputs independently. Nightly CI configures the new instrumented target; its job
budget was increased to accommodate the added two-minute run. No hosted CI, Rust 1.85,
extended instrumented fuzzing or cross-platform numerical results were observed here.

## External corpus

`python3 scripts/corpus.py --check` passed. Run `2026-09-25T17-37-32-443182Z` checked
**6,438 paths / 3,227 unique contents** in **34.107 seconds**. The generated per-file
JSON report, stage counts and representative accepted/reference-error/rejected entries
were inspected: **2,821 clean**, **38 reference-error**, **368 rejected**, and no crashes,
timeouts or runner errors. Both previous-run and baseline comparisons contain **zero
outcome or diagnostic changes**. `corpus/baseline.json` was not modified.

All external inputs still report schema/product/geometry/tessellation as
`not_implemented`: no AP schema/product checker is configured, and math primitives do
not supply a STEP geometry validator. Existing authored product outcomes were rechecked
before the commit: three accepted, two rejected and one unsupported, all schema-accepted.
See [the external per-file report](../reports/corpus/index.html).

## Math benchmark

`cargo bench -p tessstep-math --bench transforms --locked` measured a known affine
inverse plus 10,000 checked point transformations per batch. Point allocation/construction
is outside timing; inverse calculation, result checks and point transformation are inside.
One warmup and a two-second sample yielded **47,964 batches / 2.000 seconds /
41.698 microseconds per batch**. This is a local observation, not a statistical performance
comparison, whole-model benchmark or certified numerical-accuracy measurement.

## Architecture review

The new crate has no dependencies, unsafe code, heap allocation or global mutable state.
Its types preserve finite storage and explicit frame/tolerance contracts; numerical
inversion failure remains separate from finite transform construction. Existing parser,
model, product and C ABI dependencies/layouts are unchanged. Geometry C/C++ operations
remain pending and must follow the existing public-interface contract.

---

# Milestone 5 initial product graph validation — 2026-09-25

Existing Milestone 4 work was committed as `4c4bdc6` after workspace tests, formatting,
Clippy, schema fixture and relocated C/C++ checks, and an unchanged external corpus run.
The subsequent Milestone 5 slice adds independent product graphs, schema-decoded
adapters, explicit unit scales, assemblies and preserved mapping/placement descriptions.
Milestone 5 remains in progress; the exact supported subset and remaining work are in
[PRODUCT_MODEL.md](PRODUCT_MODEL.md).

Observed locally on macOS arm64 with Rust 1.95.0:

| Check | Result |
| --- | --- |
| Locked workspace debug tests | passed |
| Independent product and adapter debug/release tests | passed, including iterative 10,000-definition chain and numeric limits |
| Workspace and fuzz formatting/Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture, conformance and fixture provenance checks | passed; 11 packages and 40 authored fixtures |
| Python corpus/CI tests | 21 passed |
| Product fixture metadata regeneration and compiled checker consumers | passed |
| Deterministic product mutation smoke | 1,500 mutations; repeat results agree |
| Product libFuzzer driver smoke | 1,000 runs completed without crashes |

The libFuzzer driver was compiled with stable Rust without coverage/sanitizer
instrumentation; the runtime explicitly reported missing instrumentation. Hosted
nightly CI configures the instrumented target, but no hosted, extended fuzz or
cross-platform result was observed locally. The Rust 1.85 MSRV was not run here.
No production dependencies or public C ABI layouts were added.

## Product and external corpus observations

The authored product report, run `2026-09-25T17-18-57-773768Z`, was inspected per file.
All six inputs pass physical parsing and structural schema decoding:

- `assembly.step`, `complex.step`, `mapped.step`: accepted product graphs; repeated
  occurrences reuse definitions and representations, with ordered placement/map links.
- `cycle.step`: rejected with graph cycle error.
- `missing-units.step`: rejected with located missing length unit error at entity 23.
- `unsupported-context.step`: explicitly unsupported 2D context at entity 23.

These outcomes and expected graph counts are checked by `scripts/check_product.py`.
The original reduced test schema is not an ISO/AP schema. Geometry and tessellation
remain `not_implemented` for these fixtures. See [the product report](../reports/product/index.html).

`python3 scripts/corpus.py --check` passed: run `2026-09-25T17-24-51-515512Z` examined
**6,438 paths / 3,227 unique contents** in **32.341 seconds**. The report, all stage
counts and representative accepted, reference-error and rejected inputs were inspected.
There were **2,821 clean**, **38 reference-error** and **368 rejected** inputs, with
**zero outcome/diagnostic changes** versus both the previous run and reviewed baseline.
No crashes, timeouts or runner errors occurred. `corpus/baseline.json` is unchanged.

All external inputs remain `not_implemented` for schema and product stages because no
AP metadata/checker was configured; geometry and tessellation are also unimplemented.
The added product column normalizes absent older fields to `not_implemented` for
comparison, without changing the baseline. See [the external report](../reports/corpus/index.html).

## Product adaptation benchmark

`cargo bench -p tessstep-ap242 --bench product --locked` adapted a 540,877-byte,
10,034-entity document with 10,002 occurrences after physical parsing and schema
decoding. One warmup and a two-second sample produced **373 iterations / 2.003 s /
5.370 ms per adaptation**. Output allocations, graph validation and destruction are
included. This is one local observation, not a statistical comparison or a large
industrial assembly memory measurement.

---

# Milestone 4 structural decoder validation — 2026-09-25

Milestone 4 now delivers the supported structural-decoding scope described in
[DECODING.md](DECODING.md). Full EXPRESS rule/algorithm evaluation and AP conformance
remain unsupported. Milestone 5 product/representation/unit/assembly semantics is next.

Observed on macOS arm64, rustc/cargo 1.95.0:

| Check | Observed result |
|---|---|
| Workspace locked debug and release tests | passed; final model changes also passed focused debug/release tests |
| Decoder regression tests | 13 passed, including complex/diamond membership, SELECT tags, numeric/enum/logical equality, bounds, limits and malformed metadata |
| Generated metadata and validator consumers | compiled and ran; valid/invalid instances, JSON status/exit policy and exact generation output budgets passed |
| Authored schema corpus through generated executable | all six reviewed outcomes matched: 2 accepted, 3 rejected, 1 unsupported |
| Shared decoder mutation smoke | 11,354 bounded cases passed |
| Decoder libFuzzer driver smoke | built offline; 1,000 runs completed without crashes |
| Workspace and fuzz formatting/Clippy, warnings denied | passed |
| Workspace rustdoc, warnings denied | passed |
| Architecture, conformance, fixture provenance and CLI checks | passed; 9 packages, 33 authored fixtures |
| Python corpus/CI tests | 19 passed |
| Relocated release package | passed; generated-validator flag, C/C++ consumers and ABI checks included |

The libFuzzer driver used stable Rust without coverage/sanitizer instrumentation; runtime
warnings confirmed this limitation. Nightly CI now includes the instrumented decoder
target, but its hosted run, extended fuzzing, other operating systems and the MSRV were
not observed locally. These are regression checks, not industrial-hardening claims.

## Corpus stages and outcomes

`python3 scripts/corpus.py --check` passed. Final run
`2026-09-25T16-59-17-593268Z` examined **6,438 paths / 3,227 unique contents** in
32.041 seconds. All per-file stage counts, representative cases and prior/baseline
comparisons were inspected in [the report](../reports/corpus/index.html).

- 2,821 inputs: physical syntax accepted and references resolved.
- 38 inputs: physical syntax accepted with missing references.
- 368 inputs: structured rejection.
- Zero crashes, timeouts or runner errors.

**No outcome or diagnostic changes** from the preceding run or reviewed baseline.
`corpus/baseline.json` remains unchanged. Without supplied AP metadata, all external
inputs still show schema/geometry/tessellation as `not_implemented` in this default run.

The separate authored [schema report](../reports/schema/index.html), run
`2026-09-25T16-44-26-716422Z`, verifies the real configured schema stage: `simple.step`
and `complex.step` are accepted; bad SELECT, duplicate aggregate and missing-reference
fixtures are rejected with the expected typed errors; parameterized DATA is unsupported.
All six physically parse. These schema outcomes are checked against reviewed expectations
in `corpus/manifest.json`, not inferred from physical success. Configured-validator hash,
schema identity, structured errors and per-file stages are retained in the report.

## Decoder benchmark

`cargo bench -p tessstep-model --bench decoder --locked` used the same 486,831-byte /
20,000-entity cyclic-reference fixture after physical parsing, with one warmup and a
two-second sample. Result: **158 iterations / 2.011 s / 12.730 ms per input /
1,571,033 entities per second**. View/index allocation and destruction are included.
The earlier simple-decoder observation was 5.242 ms; the expanded hierarchy/membership
checks and storage add measurable cost. This single local run is not a statistical
performance guarantee or an AP-scale benchmark. Optimization and memory profiling
remain future work; no linear complexity or exact RSS claim is made.

---

# Public C/C++ document interface validation — 2026-09-25

Observed on macOS arm64 with Rust 1.95.0 and Apple Clang 21. ABI 1 exposes physical
buffer parsing, document inspection and independent reference diagnostics only.

| Check | Observed result |
|---|---|
| Workspace locked tests and warnings-denied Clippy | Passed |
| Bridge debug/release tests | 5 passed: ownership/report lifetime, all ten budgets, cleared outputs, frozen layouts, Send/Sync and panic containment |
| Installed C11/C++17 consumers | Passed Debug and Release after relocation; Cargo/rustc/rustup invocations blocked |
| C-only CMake project | Passed without enabling C++ |
| Native consumer ASan/UBSan | Passed; Rust library itself was not sanitizer-instrumented |
| ABI exports and native runtime | Exactly 12 C symbols, no Rust exports; macOS identity uses @rpath and runtime dependency is libSystem |
| Architecture/conformance/Python tests | Passed; 16 Python tests |
| Release packaging | macOS arm64 archive and checksum built with relocated consumer gates |

C consumers exercise null/empty arguments, parse/unsupported/limit/not-found statuses,
independent reports after document release, retained document text pointer stability,
source-order inspection and frozen ABI layouts. C++ consumers exercise moves, destruction,
owned diagnostics, construction failures, typed results and simultaneous immutable reads.
Runtime identity inspection caught and fixed a build-path install name; the check now
requires a relocatable macOS identity and an ELF SONAME on Linux.

`python3 scripts/corpus.py --check` covered 6,438 paths / 3,227 unique inputs:
2,821 clean, 38 reference errors, 368 rejected; **no baseline changes**, crashes or
timeouts. The per-file progress report was inspected, including clean, missing-reference
and rejected examples. Schema/AP validation, geometry and tessellation are not established
by this physical-parser corpus run. This public interface does not expose those stages.
The corpus baseline was not refreshed.

Verification scope: the workspace test/Clippy pass above preceded concurrent changes
in the decoder. The final `cargo fmt --all -- --check` was blocked because that work
references `crates/tessstep-model/src/decode/select.rs`, which was not yet present.
Bridge-only formatting, architecture/conformance checks, and relocated package tests
passed afterward. No concurrent decoder edits were reverted or filled in here.

Linux/Windows and macOS Intel consumers are wired into existing CI/release packaging;
they were not run locally. Static linkage, public schema decoding and zero-copy mesh
views remain unimplemented. No cross-platform or industrial-hardening result is claimed.

---

# Milestone 4 initial decoder validation — 2026-09-25

Milestone 4 is in progress. This slice adds structural decoding against explicitly
supplied reflection metadata, with borrowed views, single inheritance, primitive and
named domains, ARRAY/LIST/BAG bounds and local reference compatibility. It does not
claim complete schema validation. See [DECODING.md](DECODING.md).

Observed on macOS arm64 with rustc/cargo 1.95.0:

- Workspace locked debug and release tests passed; seven decoder tests include
  structural acceptance/rejection, exact source context, malformed metadata and
  deterministic mutation smoke. The two latest decoder tests also passed in debug.
- The generated-metadata consumer compiled and ran: EXPRESS source → generated Rust
  reflection → physical instance decoding, with a negative type case.
- Workspace Clippy (all targets/features, warnings denied), formatting, rustdoc with
  warnings denied, architecture checker, conformance evidence and diff whitespace passed.
- Decoder benchmark: 486,831 physical source bytes / 20,000 pre-parsed entities with
  cyclic references; one warmup and 382 iterations over 2.003 seconds. Observed
  **5.242 ms/input / 3,815,003 entities per second**. View/index allocation and destruction
  are timed; physical parsing is excluded. This is a local observation, not a guarantee.

`python3 scripts/corpus.py --check` passed: run `2026-09-25T12-19-02-233794Z`,
**6,438 paths / 3,227 unique inputs**, 31.593 seconds. All per-file stages and both
previous-run/baseline comparisons were inspected, including representative accepted,
missing-reference and rejected inputs in the [progress report](../reports/corpus/index.html).

| Unique inputs | Outcome |
|---:|---|
| 2,821 | physical syntax accepted, references resolved |
| 38 | physical syntax accepted, missing references |
| 368 | structured rejection |
| 0 | crashes, timeouts or runner errors |

**No outcome or diagnostic changes** versus the preceding run or reviewed baseline.
`corpus/baseline.json` was not changed. All 3,227 external inputs still report
`schema`, `geometry` and `tessellation` as `not_implemented`: the external runner has no
supplied AP metadata or schema-decoding CLI integration. The authored generated consumer
checks the new library stage separately; physical acceptance is not schema success.

Complex/multiple-inheritance mappings, SELECT/typed values, uniqueness, expression
semantics and generated owned-record conversion remain open for Milestone 4. Extended
instrumented decoder fuzzing, hosted platform CI and the MSRV run were not observed.
Concurrent C/C++ interface work in the shared workspace is outside this decoder slice.

---

# Milestone 3 validation — 2026-09-25

Environment: macOS arm64, Homebrew rustc/cargo 1.95.0. Milestone 3 adds deterministic
Rust generation and standalone static schema reflection. Generated records/references
are unchecked owned representations; physical schema decoding remains Milestone 4.

| Check | Observed result |
|---|---|
| Workspace debug and release tests, locked | passed; 50 integration tests and 2 doctests |
| Generated Rust consumer | compiled and ran against only the reflection runtime; unrelated-reference assignment correctly failed compilation |
| Generation determinism and limits | passed; repeat bytes, exact output boundary, invalid compilation and work limit |
| Workspace debug/release builds | passed |
| Workspace/fuzz formatting and Clippy, all targets, `-D warnings` | passed |
| Rustdoc with warnings denied | passed |
| Architecture checker | passed; 8 active packages, no third-party production dependencies |
| Authored corpus provenance | passed; 26 fixtures |
| Independent Part 21 / EXPRESS CLI checks | passed |
| Conformance table and test evidence | passed |
| Python corpus/CI runner tests | 16 passed |
| Codegen stable fuzz smoke | passed; 9,395 bounded prefix/mutation/arbitrary-input cases |
| Codegen libFuzzer driver smoke | built; 1,000 runs completed without crashes |
| Relocated release package smoke | passed, including `expressc --rust` output |

Consumer coverage includes imports/aliases, cross-schema inherited anonymous domains,
diamond deduplication, recursive typed references and upcasts, enum/SELECT construction,
aggregate/optional mappings, metadata identities/spans, abstract/supertype flags,
WHERE/UNIQUE/DERIVE/INVERSE source, constants and opaque algorithms. No generated
expression is executed. Architecture review is recorded in ARCHITECTURE.md.

The local libFuzzer driver used stable Rust without coverage or sanitizer instrumentation;
runtime warnings confirmed that limitation. Nightly CI now includes `schema_codegen`.
Hosted Linux/Windows/macOS CI, Rust 1.85 MSRV, dependency audits and instrumented/extended
fuzzing were not observed here; rustup is unavailable locally. These checks do not establish
industrial hardening, full EXPRESS conformance or AP schema coverage.

## Milestone 3 external corpus

`python3 scripts/corpus.py --check` passed. Run `2026-09-25T12-03-06-207101Z` examined
**6,438 paths / 3,227 unique contents** in 33.556 seconds. The per-input progress data,
diagnostics, all stage counts and both prior-run/baseline comparisons were inspected.

| Unique inputs | Outcome |
|---:|---|
| 2,821 | physical syntax accepted, references resolved |
| 38 | physical syntax accepted, missing references |
| 368 | structured rejection |
| 0 | crashes, timeouts or runner errors |

**No outcome or diagnostic changes** from the prior run or reviewed baseline.
`corpus/baseline.json` was not changed. The local [per-file report](../reports/corpus/index.html)
and [JSON](../reports/corpus/latest.json) retain every input result.

The external `schema`, `geometry` and `tessellation` stages remain `not_implemented` on
all 3,227 inputs. Generating schema-source bindings does not validate physical instances.
EXPRESS expression/algorithm semantics, EXTENSIBLE/BASED_ON types, attribute redeclaration,
AP support and the public C/C++ interfaces also remain unimplemented.

## Milestone 3 generator benchmark

`cargo bench -p tessstep-codegen --bench generator --locked`: optimized build, precompiled
IR from **223,944 source bytes / 5,001 declarations**, one warmup and a two-second sample.
Generation allocates and drops the output within the measurement; input construction and
EXPRESS compilation are excluded. The final implementation produced **9,836,240 bytes**
per iteration: **216 iterations, 9.29 ms/input, 1,009.54 generated MiB/s**. This measures
source emission, not rustc throughput, AP-scale compatibility or memory usage. It is a
local observation, not a statistical performance guarantee.

**Next: Milestone 4 — schema-aware instance decoding and validation.**

---

# Milestone 2 validation — 2026-09-25

Environment: macOS arm64, rustc/cargo 1.95.0. Milestone 2 adds the EXPRESS declaration
frontend and `expressc`; geometry and schema-aware STEP instance validation remain future
work. No ISO/AP schema was bundled or manually transcribed.

| Check | Observed result |
|---|---|
| Workspace build and locked tests | passed; 46 integration tests and 2 doctests |
| Workspace/fuzz formatting | passed |
| Workspace clippy, all targets/features, `-D warnings` | passed |
| Fuzz clippy, all targets, `-D warnings` | passed |
| Workspace rustdoc, `RUSTDOCFLAGS=-D warnings` | passed |
| Architecture / unsafe-boundary checker | passed, 6 active packages |
| Authored corpus provenance | passed, 26 fixtures (20 Part 21, 6 EXPRESS) |
| Part 21 and EXPRESS independent CLI JSON checks | passed, including exit policy, escaping and determinism |
| Generated conformance table / test evidence | passed |
| Python corpus-runner tests | 7 passed |
| EXPRESS stable fuzz smoke | passed, 12,285 prefix/mutation/arbitrary-input exercise calls |
| EXPRESS libFuzzer driver smoke | built and completed 1,000 runs without crashes |

The EXPRESS smoke harness exercises lexing, compilation, determinism and diagnostic
spans under finite budgets; invalid UTF-8 is skipped because the library accepts `&str`.
The local libFuzzer driver was built with stable Rust, without coverage/sanitizer
instrumentation; runtime warnings confirmed that limitation. Nightly instrumented CI
now includes the EXPRESS target but has not been observed running here. Hosted platform
CI, the 1.85 MSRV job, advisory/license audits and extended fuzzing remain unobserved
locally. This is regression evidence, not industrial hardening.

## Milestone 2 external corpus

`python3 scripts/corpus.py --check` passed. Run `2026-09-25T11-40-21-681116Z` examined
**6,438 paths / 3,227 unique contents** in 32.921 seconds. The per-file progress report,
stage counts, representative accepted/rejected/reference-error cases, and both previous
run and reviewed-baseline comparisons were inspected:

| Unique inputs | Outcome |
|---:|---|
| 2,821 | physical syntax accepted, references resolved |
| 38 | physical syntax accepted, missing references |
| 368 | structured rejection |
| 0 | crashes, timeouts or runner errors |

**No outcome or diagnostic changes** from the previous run or reviewed baseline.
`corpus/baseline.json` was not changed. Reports remain local generated artifacts:
[per-file progress](../reports/corpus/index.html) and [JSON](../reports/corpus/latest.json).

The external `schema`, `geometry` and `tessellation` stages remain `not_implemented` for
all inputs. The schema stage refers to validation of physical STEP instances, which is
Milestone 4. `scripts/check_express.py` separately verifies source declaration parsing,
basic schema compilation, import resolution and explicit unsupported semantics over
original `.exp` fixtures. The frontend does not turn STEP physical acceptance into
schema conformance. EXPRESS expression/algorithm semantics, EXTENSIBLE/BASED_ON types,
attribute redeclaration and full schema conformance remain unsupported as documented.

## Milestone 2 compiler benchmark

`cargo bench -p tessstep-express --bench compiler`: optimized build, one warmup and a
two-second sample per phase; **233,946 bytes, 5,000 entities and one type**. Input generation
is outside timing; token/AST/IR allocation and destruction are included.

| Phase | Iterations | Mean ms/input | MiB/s |
|---|---:|---:|---:|
| EXPRESS lexer | 539 | 3.72 | 60.05 |
| Declaration AST | 79 | 25.35 | 8.80 |
| Full basic compilation | 67 | 30.00 | 7.44 |

This is a local throughput baseline, not a statistical comparison, AP-scale benchmark,
or memory profile. The source-order IR and import graph are deterministic for repeated
identical ordered inputs. Architecture review is recorded in ARCHITECTURE.md.

**Next: Milestone 3 — deterministic Rust bindings and reflection.**

---

# Milestone 1 validation — 2026-09-25

Environment: macOS arm64, Homebrew rustc 1.95.0 / cargo 1.95.0, edition 2024.
This records observed results, not a full ISO or industrial-readiness certification.

## Quality checks

| Check | Result |
|---|---|
| `cargo build --workspace --locked` | passed |
| `cargo test --workspace --locked` | 33 integration tests and 1 public API doctest passed |
| workspace and fuzz `cargo fmt -- --check` | passed |
| workspace clippy, all targets/features, `-D warnings` | passed |
| fuzz-package clippy, all targets, `-D warnings` | passed |
| rustdoc, `-D warnings` | passed |
| architecture / unsafe-boundary checker | passed, 4 active packages |
| authored corpus provenance checker | passed, 20 fixtures |
| independent CLI JSON / escaping / duplicate-header / determinism checks | passed |
| generated conformance table and named test evidence | passed |
| Python corpus-runner tests | 7 passed |
| handwritten unsafe / unimplemented macro review | none in production code |

The deterministic fuzz smoke exercised **44,220** seed, prefix, mutation and arbitrary-byte
cases. All three libFuzzer drivers built and completed **1,000 runs each** over the supplied
seed directories without crashes. These local driver runs were built with stable Rust:
they lacked coverage and sanitizer instrumentation and emitted warnings to that effect.
They do not replace the nightly instrumented sessions configured in CI. No long fuzzing
or memory-profiling campaign has been performed.

## External corpus

The repository's optional `~/step-corpus` was available. Ran:

```sh
python3 scripts/corpus.py --check
```

Run `2026-09-25T11-21-26-354686Z` examined **6,438 paths / 3,227 unique input contents**
in 31.632 seconds. The report and baseline comparison were inspected. Observations:

| Unique inputs | Outcome |
|---:|---|
| 2,821 | physical syntax accepted, no missing references |
| 38 | physical syntax accepted, missing references reported |
| 368 | structured rejection |
| 0 | crashes, timeouts, or runner errors |

There were **no changes from the reviewed baseline or previous run**. The baseline was
not refreshed. Representative rejections include invalid enumeration framing, malformed
typed parameters, duplicate IDs, nesting limits, overflow and invalid Unicode. An input
labelled as an encoding control contains an enumeration-wrapped typed value; the label
alone is not evidence that rejection is incorrect. These observations are compatibility
results, not an assertion that every rejected input has been independently adjudicated.

The searchable [per-file report](../reports/corpus/index.html) and
[machine-readable results](../reports/corpus/latest.json) remain local generated artifacts.
See [corpus-testing.md](corpus-testing.md) for baseline/report operation. External source
models were not copied into the repository. Schema, geometry and tessellation stages
remain `not_implemented`; no new semantic stage is claimed from this run.

## Benchmark

Command: `cargo bench -p tessstep-part21 --bench parser`. Optimized build, one warmup,
two-second samples, 957,938 input bytes and 20,000 entities. Fixture construction is
outside timing; token/event allocation, decoding and destruction are inside timing.

| Phase | Iterations | Mean ms / input | MiB/s |
|---|---:|---:|---:|
| Lexer | 154 | 13.07 | 69.89 |
| Parser events | 102 | 19.74 | 46.28 |

This is an initial local throughput baseline. It is not a statistical comparison,
a whole-document database benchmark, or a large-assembly memory measurement.

## Remaining verification and capability limits

Hosted Linux/Windows/macOS CI, the Rust 1.85 MSRV job, cargo-deny, advisory audits and
instrumented nightly fuzzing are configured but have not been observed running here.
Physical parsing does not validate declared implementation levels, schema-population
rules, entity attributes, inheritance, units or CAD semantics. Legacy scopes, archives,
BOM handling and tolerant syntax recovery are unsupported. External URI resolution and
cryptographic signature verification are not performed. The parser uses bounded logical
budgets and finite numeric types, not an exact process memory cap or arbitrary precision.
See CONFORMANCE.md for the feature matrix.

The record above describes the earlier Milestone 1 run. The Milestone 2 results at the
top of this document supersede its frontend status and current workspace test counts.
