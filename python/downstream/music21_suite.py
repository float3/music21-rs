"""Run music21's own test suite against `music21_rs`.

`run.py` measures the crate against somebody else's library. This measures it
against music21 itself: every `Test` class and every docstring in every
music21 module, run twice — once on music21, once with
`music21_rs.install_into_music21()` in front of it — with the two sets of
failures compared.

music21's suite has failures of its own in any given environment (a missing
optional package, a platform difference), so the test is a comparison and not
a pass mark, exactly as `run.py` is: anything that fails only under
`music21_rs` is a gap in the crate.

    python python/downstream/music21_suite.py

The two runs are separate processes, because installing over music21 cannot
be undone inside one. Needs the `music21` submodule checked out, the `music21_rs`
wheel installed, and music21's own test dependencies — `scipy` and
`python-Levenshtein` on top of what music21 itself needs.

music21's own `testSingleCoreAll.main` is not used: it insists on lilypond
being installed before it will run anything. Everything after that check is
what happens here.
"""

from __future__ import annotations

import argparse
import doctest
import json
import subprocess
import sys
import unittest
import warnings
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SUBMODULE = ROOT / "music21"
OPTIONS = doctest.ELLIPSIS | doctest.NORMALIZE_WHITESPACE


def build_suite(only: str | None) -> unittest.TestSuite:
    """Every `Test` class and every docstring music21 has."""
    from music21 import common
    from music21.test import commonTest, testRunner

    suite = unittest.TestSuite()
    for module in common.misc.sortModules(commonTest.ModuleGather().load(False)):
        if only is not None and only not in module.__name__:
            continue
        if hasattr(module, "Test"):
            suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(module.Test))
        try:
            suite.addTests(commonTest.defaultDoctestSuite(module))
        except ValueError:
            continue
        testRunner.addDocAttrTestsToSuite(
            suite,
            [getattr(module, name) for name in dir(module)],
            outerFilename=module.__file__,
            globs=__import__("music21").__dict__.copy(),
            optionflags=OPTIONS,
        )
    testRunner.fixDoctests(suite)
    return suite


def clear_corpus_cache() -> None:
    """Throw away music21's parsed-score cache before a run.

    A cached score is a pickle carrying the classes it was parsed with, so a
    cache written by one of the two runs would be read by the other and the
    comparison would be measuring the wrong thing.
    """
    from music21 import environment

    scratch = Path(str(environment.Environment().getRootTempDir()))
    for cached in scratch.glob("*.p*"):
        cached.unlink(missing_ok=True)


def run_one(out: Path, use_rs: bool, only: str | None) -> int:
    """One run of the suite, writing what failed to `out`."""
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
    sys.path.insert(0, str(SUBMODULE))

    import music21

    print(f"music21 {music21.__version__} from {music21.__file__}", flush=True)
    clear_corpus_cache()
    if use_rs:
        import music21_rs

        print(f"music21_rs over {music21_rs.install_into_music21()} names", flush=True)

    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        result = unittest.TextTestRunner(verbosity=0, stream=sys.stdout).run(
            build_suite(only)
        )

    report = {
        "run": result.testsRun,
        "failures": sorted(str(case) for case, _ in result.failures),
        "errors": sorted(str(case) for case, _ in result.errors),
        "detail": {
            **{str(case): text for case, text in result.failures},
            **{str(case): text for case, text in result.errors},
        },
    }
    out.write_text(json.dumps(report, indent=1), encoding="utf-8")
    print(
        f"ran {report['run']}, {len(report['failures'])} failures, "
        f"{len(report['errors'])} errors",
        flush=True,
    )
    return 0


def compare(plain: Path, ours: Path) -> int:
    """What fails under `music21_rs` and not under music21."""
    left = json.loads(plain.read_text(encoding="utf-8"))
    right = json.loads(ours.read_text(encoding="utf-8"))
    bad = lambda report: set(report["failures"]) | set(report["errors"])  # noqa: E731
    theirs, mine = bad(left), bad(right)
    new = sorted(mine - theirs)
    print()
    print(f"  music21   : {len(theirs)} of its own failures, {left['run']} tests")
    print(f"  music21_rs: {len(mine)} failures, {right['run']} tests")
    if not new:
        print()
        print("music21's own suite behaves identically on music21_rs.")
        return 0
    print()
    print(f"{len(new)} tests fail only under music21_rs:")
    for name in new[:40]:
        print(f"  {name}")
    if len(new) > 40:
        print(f"  ... and {len(new) - 40} more")
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--only", help="run only modules whose name contains this")
    parser.add_argument("--out", type=Path, default=ROOT / "target" / "music21-suite")
    parser.add_argument("--run", choices=("music21", "music21_rs"), help=argparse.SUPPRESS)
    arguments = parser.parse_args()

    arguments.out.mkdir(parents=True, exist_ok=True)
    plain, ours = arguments.out / "music21.json", arguments.out / "music21_rs.json"

    if arguments.run is not None:
        return run_one(
            plain if arguments.run == "music21" else ours,
            arguments.run == "music21_rs",
            arguments.only,
        )

    if not SUBMODULE.exists():
        print("the music21 submodule is not checked out", file=sys.stderr)
        return 1

    for which in ("music21", "music21_rs"):
        print(f"$ running music21's suite on {which}", flush=True)
        command = [sys.executable, __file__, "--run", which, "--out", str(arguments.out)]
        if arguments.only:
            command += ["--only", arguments.only]
        finished = subprocess.run(command, check=False)
        if finished.returncode != 0:
            print(f"the {which} run did not finish", file=sys.stderr)
            return 1

    return compare(plain, ours)


if __name__ == "__main__":
    sys.exit(main())
