#!/usr/bin/env python3
"""Per-target libFuzzer configuration and deterministic seed corpora from committed files.

Every `[[bin]]` in fuzz/Cargo.toml needs an entry in TARGETS: the committed inputs that
fit what its exercise function decodes, generated seeds for the fixed-layout binary
harnesses (the values the stable smoke tests start from), its libFuzzer limits, and the
stable driver smoke tests in SMOKE that exercise it. Committed minimized findings in
fuzz/regressions/<target>/ are always seeds of their target.
"""
import argparse
import glob
import hashlib
import json
import math
from pathlib import Path
import shutil
import struct
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BUDGET = 240
MAX_BUDGET = 900

PART21 = ("corpus/part21/valid/*.step", "corpus/part21/invalid/*.step", "corpus/schema/*.step",
          "corpus/product/*.step", "corpus/geometry/*.step")
EXPRESS = ("corpus/express/valid/*.exp", "corpus/express/invalid/*.exp", "corpus/schema/*.exp",
           "corpus/product/*.exp", "corpus/geometry/*.exp")


def f64s(*values):
    return struct.pack(f"<{len(values)}d", *values)


# The stable smoke tests' boundary values, each repeated across the whole input.
BOUNDARY = {"zero": f64s(0.0), "negative-zero": f64s(-0.0), "subnormal": f64s(5e-324), "one": f64s(1.0),
            "max": f64s(sys.float_info.max), "infinity": f64s(math.inf), "nan": f64s(math.nan),
            "all-ones": b"\xff" * 8}


def boundary(width):
    return {f"repeat-{name}": value * (width // 8) for name, value in BOUNDARY.items()}


def math_seeds():
    # 3x3 matrix, translation, point p, vector/direction q (math tests/support/exercise.rs).
    return {"identity": f64s(1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 2, 3, 1, 0, 0, 0, 0, 1),
            "reflection-shear": f64s(1, 0.5, 0, 0, -1, 0, 0, 0, 2, 0, 0, 0, 3, 4, 5, 0, 1, 0),
            "near-singular": f64s(1, 0, 0, 0, 1, 0, 0, 0, 1e-300, 0, 0, 0, 1, 1, 1, 1, 1, 1),
            "singular": f64s(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 0, 0, 1),
            **boundary(144)}


def curve_seeds():
    # Origin, x and y axes, two lengths, a parameter, span ends and two span parameters
    # (curves tests/support/exercise.rs).
    frame = (0, 0, 0, 1, 0, 0, 0, 1, 0)
    return {"conics": f64s(*frame, 2, 1, 0.5, 0, math.pi, 0.25, 0.75),
            "reversed-span": f64s(*frame, 2, 1, 7, 2 * math.pi, 0, 0.5, 0.5),
            "parallel-axes": f64s(1, 2, 3, 1, 1, 0, 2, 2, 0, 0, 0, -1, -1e300, 1e300, math.inf, math.nan),
            **boundary(128)}


def nurbs_seeds():
    # Three 2D control points, six knots, three weights and a parameter; an odd byte 0
    # selects degree byte0 % 20 and an odd byte 1 the supplied knots
    # (curves tests/support/spline_exercise.rs). The first is spline_fuzz.rs's seed.
    quadratic = f64s(0, 0, 1, 1, 2, 0, 0, 0, 0, 1, 1, 1, 1, 0.7, 1, 0.3)
    knots = bytearray(f64s(0, 0, 1, 1, 2, 0, -1, -1, -1, 3, 3, 3, 1, 0.7, 1, 0.3))
    knots[1] |= 1
    degree = bytearray(quadratic)
    degree[0] = 3
    return {"quadratic": quadratic, "supplied-knots": bytes(knots), "odd-degree": bytes(degree), **boundary(128)}


def surface_seeds():
    # Radii/angle, an evaluation (u, v), four bilinear control points and weights
    # (surfaces tests/support/exercise.rs). The first is the surface smoke test's seed.
    return {"bilinear": f64s(0, 0, 0, 0, 1, 0, 1, 0, 0, 1, 1, 1, 1, 2, 3, 4),
            "analytic": f64s(1, 0.5, 0.3, 0.7, 0, 1, 1, 1, 0, 0, 1, 1, 1, 2, 1, 0.4),
            **boundary(128)}


def pipeline_seeds():
    # Byte 0 selects one of six constructed fixtures (square, adjacent faces, disk, seam
    # cylinder, closed cylinder, NURBS bump); each whole 12-byte chunk applies one
    # mutation of kind byte 2 % 8 with an index, an id and an f64
    # (tessellate tests/support/pipeline_exercise.rs).
    seeds = {f"fixture-{kind}": bytes([kind]) for kind in range(6)}
    seeds.update({f"fixture-{kind}-zeros": bytes([kind]) + bytes(95) for kind in range(6)})
    seeds.update({f"closed-cylinder-mutation-{kind}": bytes([4, 1, kind, 1]) + f64s(0.5) for kind in range(8)})
    return seeds


def numeric(generated, max_len, timeout=10):
    return {"sources": (), "generated": generated, "max_len": max_len, "timeout": timeout,
            "rss_limit_mb": 1024, "malloc_limit_mb": 256}


def text(sources, max_len, timeout=10):
    return {"sources": sources, "generated": None, "max_len": max_len, "timeout": timeout,
            "rss_limit_mb": 2048, "malloc_limit_mb": 512}


# max_len is each harness's input budget (Part 21 limits.rs: 64 KiB; EXPRESS: 8 KiB) or
# the bytes it consumes. Timeouts are per input; the input budgets bound work far below
# them, so a timeout is a budget that does not bound work. The import targets select
# root #1000 (#300 for presentations); every authored geometry fixture is a seed so
# that roots of other kinds still contribute their entities to mutation.
GEOMETRY = ("corpus/geometry/*.step",)
TARGETS = {
    "analytic_curves": numeric(curve_seeds, 128),
    "brep_import": text(GEOMETRY, 65536, timeout=30),
    "brep_pipeline": numeric(pipeline_seeds, 96, timeout=30),
    "entity_database": text(PART21, 65536),
    "express_frontend": text(EXPRESS, 8192),
    "math_primitives": numeric(math_seeds, 144),
    "nurbs_curves": numeric(nurbs_seeds, 128),
    "part21_lexer": text(PART21, 65536),
    "part21_parser": text(PART21, 65536),
    "presentation_import": text(GEOMETRY, 65536, timeout=30),
    "product_adapter": text(("corpus/product/*.step",), 65536),
    "schema_codegen": text(EXPRESS, 8192),
    "schema_decoder": text(("corpus/schema/*.step",), 65536),
    "surface_evaluators": numeric(surface_seeds, 128),
    "tessellated_import": text(GEOMETRY, 65536, timeout=30),
}

# Stable, uninstrumented driver smoke: deterministic tests that run a target's shared
# exercise function (or a copy of its calls) over seeds, prefixes and mutations.
SMOKE = [
    {"name": "fuzz_corpus_smoke", "package": "tessstep-model", "test": "fuzz_smoke", "filter": None,
     "targets": ["entity_database", "part21_lexer", "part21_parser"],
     "exercise": "fuzz/harness.rs (lexer, parser and document loops)"},
    {"name": "schema_decoder_fuzz_smoke", "package": "tessstep-model", "test": "decoder_fuzz", "filter": None,
     "targets": ["schema_decoder"], "exercise": "shared"},
    {"name": "express_fuzz_smoke", "package": "tessstep-express", "test": "fuzz_smoke", "filter": None,
     "targets": ["express_frontend"], "exercise": "shared"},
    {"name": "codegen_fuzz_smoke", "package": "tessstep-codegen", "test": "generated", "filter": "codegen_fuzz_smoke",
     "targets": ["schema_codegen"], "exercise": "shared"},
    {"name": "product_adapter_fuzz_smoke", "package": "tessstep-ap242", "test": "product",
     "filter": "product_adapter_fuzz_smoke", "targets": ["product_adapter"],
     "exercise": "copy of the calls, 16 KiB parse budget"},
    {"name": "math_arbitrary_float_fuzz_smoke", "package": "tessstep-math", "test": "fuzz_smoke", "filter": None,
     "targets": ["math_primitives"], "exercise": "shared"},
    {"name": "analytic_curve_arbitrary_float_smoke", "package": "tessstep-curves", "test": "fuzz_smoke",
     "filter": None, "targets": ["analytic_curves"], "exercise": "shared"},
    {"name": "nurbs_curve_bounded_mutation_smoke", "package": "tessstep-curves", "test": "spline_fuzz",
     "filter": None, "targets": ["nurbs_curves"], "exercise": "shared"},
    {"name": "surface_bounded_arbitrary_float_smoke", "package": "tessstep-surfaces", "test": "fuzz_smoke",
     "filter": None, "targets": ["surface_evaluators"], "exercise": "shared"},
    {"name": "topology_trim_sampling_bounded_mutation_smoke", "package": "tessstep-tessellate",
     "test": "pipeline_fuzz", "filter": None, "targets": ["brep_pipeline"], "exercise": "shared"},
    {"name": "tessellated_budgets_order_and_mutation_smoke", "package": "tessstep-import", "test": "tessellated",
     "filter": "tessellated_budgets_order_and_mutation_smoke", "targets": ["tessellated_import"],
     "exercise": "copy of the calls, larger budgets"},
    {"name": "presentation_rejections_are_typed_and_located", "package": "tessstep-import", "test": "presentation",
     "filter": "presentation_rejections_are_typed_and_located", "targets": ["presentation_import"],
     "exercise": "copy of the calls, larger budgets"},
    {"name": "brep_budgets_record_order_and_mutation_smoke", "package": "tessstep-import", "test": "brep",
     "filter": "brep_budgets_record_order_and_mutation_smoke", "targets": ["brep_import"],
     "exercise": "copy of the calls, larger budgets"},
    {"name": "fuzz_regressions", "package": "tessstep", "test": "fuzz_regressions", "filter": None,
     "targets": sorted(TARGETS), "exercise": "shared; replays fuzz/regressions"},
]


def cargo_targets(root=ROOT):
    manifest = tomllib.loads((root / "fuzz/Cargo.toml").read_text(encoding="utf-8"))
    return sorted(binary["name"] for binary in manifest.get("bin", []))


def check(root=ROOT, targets=None, smoke=None):
    """Raise ValueError unless configuration, fuzz/Cargo.toml and the corpora agree."""
    targets = TARGETS if targets is None else targets
    smoke = SMOKE if smoke is None else smoke
    declared = cargo_targets(root)
    if declared != sorted(targets):
        missing = sorted(set(declared) - set(targets))
        stale = sorted(set(targets) - set(declared))
        raise ValueError(f"fuzz/Cargo.toml and scripts/fuzz_seeds.py TARGETS differ: unconfigured {missing}, "
                         f"undeclared {stale}; add seeds, limits and a SMOKE entry for each new target")
    for name, config in sorted(targets.items()):
        for pattern in config["sources"]:
            if not glob.glob(pattern, root_dir=root):
                raise ValueError(f"{name}: seed pattern {pattern} matches no committed file")
        if not config["sources"] and not config["generated"]:
            raise ValueError(f"{name}: no seed source")
    # The committed-finding replay covers every target; each also needs its own smoke.
    covered = {target for row in smoke if row["name"] != "fuzz_regressions" for target in row["targets"]}
    uncovered, unknown = sorted(set(targets) - covered), sorted(covered - set(targets))
    if uncovered or unknown:
        raise ValueError(f"no stable smoke test in SMOKE for {uncovered}"
                         + (f"; SMOKE names unknown targets {unknown}" if unknown else ""))
    return declared


def candidates(name, config, root=ROOT):
    """(seed name, source label, bytes) in a deterministic order."""
    for pattern in config["sources"]:
        for path in sorted(Path(p).as_posix() for p in glob.glob(pattern, root_dir=root)):
            yield path.replace("/", "__"), path, (root / path).read_bytes()
    if config["generated"]:
        for label, data in config["generated"]().items():
            yield f"generated__{label}.bin", f"generated:{label}", data
    regressions = root / "fuzz/regressions" / name
    if regressions.is_dir():
        for path in sorted(regressions.iterdir()):
            if path.is_file() and not path.name.startswith("."):
                yield f"regression__{path.name}", path.relative_to(root).as_posix(), path.read_bytes()


def assemble(name, output, root=ROOT, config=None):
    """Write the seeds of one target into output/<name>/ and its manifest to output/<name>.json."""
    config = TARGETS[name] if config is None else config
    directory = Path(output) / name
    if directory.exists():
        shutil.rmtree(directory)
    directory.mkdir(parents=True)
    seeds, skipped, hashes = [], [], {}
    for seed, source, data in candidates(name, config, root):
        digest = hashlib.sha256(data).hexdigest()
        if len(data) > config["max_len"]:
            skipped.append({"source": source, "reason": f"{len(data)} bytes exceeds -max_len={config['max_len']}"})
        elif digest in hashes:
            skipped.append({"source": source, "reason": f"duplicate of {hashes[digest]}"})
        else:
            if (directory / seed).exists():
                raise ValueError(f"{name}: seed name collision {seed}")
            hashes[digest] = source
            (directory / seed).write_bytes(data)
            seeds.append({"name": seed, "source": source, "bytes": len(data), "sha256": digest})
    manifest = {"format_version": 1, "target": name, "max_len": config["max_len"], "seeds": seeds, "skipped": skipped}
    (Path(output) / f"{name}.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def plan(only="", budget="", root=ROOT):
    """The GitHub Actions outputs: the target matrix and the per-target seconds."""
    declared = check(root)
    selected = [name.strip() for name in only.replace(",", " ").split() if name.strip()]
    unknown = sorted(set(selected) - set(declared))
    if unknown:
        raise ValueError(f"unknown fuzz targets {unknown}; choose from {declared}")
    budget = str(budget).strip() or str(DEFAULT_BUDGET)
    if not budget.isdigit() or not 1 <= int(budget) <= MAX_BUDGET:
        raise ValueError(f"budget must be whole seconds between 1 and {MAX_BUDGET}, not {budget!r}")
    return {"targets": json.dumps(sorted(set(selected)) or declared), "budget": str(int(budget))}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--output", type=Path, help="write seeds to OUTPUT/<target>/ and manifests to OUTPUT/<target>.json")
    parser.add_argument("--target", action="append", help="assemble only this target (repeatable)")
    parser.add_argument("--list", action="store_true", help="print the configured targets")
    parser.add_argument("--plan", action="store_true", help="print GitHub Actions outputs: targets matrix and budget")
    parser.add_argument("--only", default="", help="with --plan: comma or space separated subset of targets")
    parser.add_argument("--budget", default="", help=f"with --plan: seconds per target (default {DEFAULT_BUDGET})")
    args = parser.parse_args(argv)
    try:
        if args.plan:
            for key, value in plan(args.only, args.budget).items():
                print(f"{key}={value}")
            return 0
        declared = check()
        if args.list:
            print("\n".join(declared))
            return 0
        if not args.output:
            parser.error("give --output, --list or --plan")
        for name in args.target or declared:
            if name not in TARGETS:
                raise ValueError(f"unknown fuzz target {name}; choose from {declared}")
            manifest = assemble(name, args.output)
            print(f"{name}: {len(manifest['seeds'])} seeds, {len(manifest['skipped'])} skipped")
    except ValueError as error:
        print(f"fuzz_seeds: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
