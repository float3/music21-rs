"""Run a real music21 project's test suite against `music21_rs`.

The doctest harness in `python-parity` measures the crate against music21's
own docstrings. This measures it against somebody else's code: a library
written for music21, by someone who had never heard of this one, with its own
test suite and its own idea of what music21 does.

`harte-library` parses Harte chord notation. It is a good subject because it
does exactly what a music21 library does — it *subclasses* `chord.Chord` and
`interval.Interval` and builds on their behaviour — and because it stays
inside the part of music21 the crate models: pitches, intervals and chords,
with no streams and no notation files.

The test is a comparison, not a pass mark. The suite is run twice, once
against music21 and once with `music21_rs.install_into_music21()` in front of
it, and the two sets of failures have to match exactly. harte-library's own
suite has 212 failures of its own (its chord grammar mis-parses `113` as a
degree), and those must fail identically either way; anything that fails only
under `music21_rs` is a gap in the crate.

    python python/downstream/run.py

Needs `git`, and `music21`, `lark`, `numpy`, `pytest` and the `music21_rs`
wheel installed in the interpreter that runs it.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

PROJECT = "harte-library"
REPOSITORY = "https://github.com/andreamust/harte-library.git"
# Pinned so the comparison is against a fixed suite; bump deliberately.
COMMIT = "dd8cfd728572ed728195e05c8f2062a58b3e9036"

CONFTEST = '''\
"""Point this project at the Rust implementation.

Written by music21-rs's `python/downstream/run.py`. The swap has to happen
before the project does `from music21.chord import Chord`, and a rootdir
conftest is imported before any test module is.
"""

import os

if os.environ.get("MUSIC21_RS") == "1":
    import music21_rs

    music21_rs.install_into_music21()
'''

FAILED = re.compile(r"^FAILED (\S+)", re.MULTILINE)
COUNT = re.compile(r"(\d+) (passed|failed)")


def run(command: list[str], **kwargs) -> subprocess.CompletedProcess[str]:
    print("$", " ".join(command), flush=True)
    return subprocess.run(command, text=True, **kwargs)


def checkout(into: Path) -> Path:
    """Clones the project at its pinned commit, or reuses what is there."""
    worktree = into / PROJECT
    if not (worktree / ".git").is_dir():
        worktree.mkdir(parents=True, exist_ok=True)
        run(["git", "init", "--quiet", str(worktree)], check=True)
        run(["git", "-C", str(worktree), "remote", "add", "origin", REPOSITORY], check=True)
    run(
        ["git", "-C", str(worktree), "fetch", "--quiet", "--depth", "1", "origin", COMMIT],
        check=True,
    )
    run(["git", "-C", str(worktree), "checkout", "--quiet", COMMIT], check=True)
    (worktree / "conftest.py").write_text(CONFTEST, encoding="utf-8")
    return worktree


def failures(worktree: Path, *, swapped: bool) -> set[str]:
    environment = dict(os.environ)
    environment["MUSIC21_RS"] = "1" if swapped else "0"
    result = run(
        [sys.executable, "-m", "pytest", "test", "-q", "--no-header", "-p", "no:cacheprovider"],
        cwd=worktree,
        env=environment,
        capture_output=True,
    )
    tail = result.stdout.strip().splitlines()[-1:] or ["(no output)"]
    print(f"  {'music21_rs' if swapped else 'music21   '}: {tail[0]}", flush=True)
    # pytest answers 0 when everything passed and 1 when tests failed;
    # anything else means the suite never ran.
    if result.returncode not in (0, 1):
        print(result.stdout[-4000:], file=sys.stderr)
        print(result.stderr[-4000:], file=sys.stderr)
        raise SystemExit(f"{PROJECT}'s suite did not run (pytest exit {result.returncode})")
    ran = sum(int(count) for count, _ in COUNT.findall(tail[0]))
    if ran < 1000:
        print(result.stdout[-4000:], file=sys.stderr)
        raise SystemExit(f"{PROJECT} ran only {ran} tests; its suite has thousands")
    return set(FAILED.findall(result.stdout))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--directory",
        type=Path,
        default=Path("target/downstream"),
        help="where to check the project out (default: target/downstream)",
    )
    arguments = parser.parse_args()

    worktree = checkout(arguments.directory.resolve())
    print(f"\nrunning {PROJECT}'s suite twice", flush=True)
    stock = failures(worktree, swapped=False)
    ours = failures(worktree, swapped=True)

    only_ours = sorted(ours - stock)
    only_stock = sorted(stock - ours)
    if not only_ours and not only_stock:
        print(
            f"\n{PROJECT} behaves identically on music21_rs: "
            f"{len(stock)} of its own failures either way"
        )
        return 0
    if only_ours:
        print(f"\n{len(only_ours)} tests fail only against music21_rs:")
        print("    " + "\n    ".join(only_ours))
    if only_stock:
        print(f"\n{len(only_stock)} tests fail only against music21:")
        print("    " + "\n    ".join(only_stock))
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
