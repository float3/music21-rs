"""music21's own tests, run where music21 is not installed and
`music21_rs.provide_music21()` answers `import music21` instead.

music21's tests live inside music21's package, so they cannot be imported
without importing music21. Each module's source is read by path and run
under its own dotted name instead, so its `from music21 import ...` lines are
answered by the provider. A module whose imports fail is blocked whole, and
what it could not import is the blocker. A module that imports has the names
the provider gives its module put over its own, as `install_into_music21`
does to an installed music21, and its `Test` class and docstrings are run.
A module the provider has nothing for runs music21's own code, which is not
music21-rs: its passes are kept apart.
"""

import doctest
import importlib
import os
import re
import signal
import sys
import traceback
import types
import unittest

import music21_rs

# Long enough for any test music21 runs; a test that hangs on a half-provided
# object is cut off rather than holding up the run.
TEST_SECONDS = 60

_NOT_PROVIDED = re.compile(r"music21-rs does not provide (\S+?);")
_NO_MODULE = re.compile(r"No module named '([^']+)'")
_CANNOT_IMPORT = re.compile(r"cannot import name '([^']+)' from '([^']+)'")
_MODULE_ATTRIBUTE = re.compile(r"module '([^']+)' has no attribute '([^']+)'")
_TYPE_ATTRIBUTE = re.compile(r"type object '([^']+)' has no attribute '([^']+)'")
_OBJECT_ATTRIBUTE = re.compile(r"'([^']+)' object has no attribute '([^']+)'")
_UNDEFINED = re.compile(r"name '([^']+)' is not defined")
_RAISED = re.compile(
    r"^\s*(ModuleNotFoundError|ImportError|AttributeError|NameError): (.*)$", re.M
)

# music21's own modules, by the name `from music21 import *` gives each. A
# docstring that uses one music21-rs does not provide stops on a NameError.
MODULES = set()


def blocker_of(kind, message):
    """The missing module or name an import or attribute error is about,
    as a dotted name, or None for one that names nothing."""
    if kind in ("ModuleNotFoundError", "ImportError"):
        for pattern in (_NOT_PROVIDED, _NO_MODULE):
            found = pattern.search(message)
            if found:
                return found.group(1)
        found = _CANNOT_IMPORT.search(message)
        if found:
            return f"{found.group(2)}.{found.group(1)}"
        return None
    if kind == "NameError":
        found = _UNDEFINED.search(message)
        if found and found.group(1) in MODULES:
            return f"music21.{found.group(1)}"
        return None
    for pattern in (_MODULE_ATTRIBUTE, _TYPE_ATTRIBUTE, _OBJECT_ATTRIBUTE):
        found = pattern.search(message)
        if found:
            return f"{found.group(1)}.{found.group(2)}"
    return None


def blocker_in(text, first):
    """The blocker a failure's text names: the first error a doctest raised,
    which stops it there, or the last line of a traceback."""
    raised = _RAISED.findall(text)
    if not raised:
        return None
    kind, message = raised[0] if first else raised[-1]
    return blocker_of(kind, message)


def dotted_name(root, path):
    """`music21.stream.base` for `<root>/music21/stream/base.py`."""
    relative = os.path.relpath(path, root)[: -len(".py")]
    parts = relative.split(os.sep)
    if parts[-1] == "__init__":
        parts.pop()
    return ".".join(parts)


def module_files(root, only):
    """Every module of music21's with a test or a docstring example in it."""
    found = []
    for directory, subdirectories, files in os.walk(os.path.join(root, "music21")):
        subdirectories[:] = sorted(
            name for name in subdirectories if not name.startswith((".", "_"))
        )
        for file in sorted(files):
            if not file.endswith(".py"):
                continue
            path = os.path.join(directory, file)
            name = dotted_name(root, path)
            if only and not any(part in name for part in only):
                continue
            with open(path, encoding="utf-8") as handle:
                source = handle.read()
            if "unittest" in source or ">>>" in source:
                found.append((name, path, source))
    return found


class _TimedOut(Exception):
    pass


def _alarm(signum, frame):
    raise _TimedOut(f"took longer than {TEST_SECONDS} seconds")


class Result(unittest.TestResult):
    """Each test cut off after `TEST_SECONDS`, and each that passed named."""

    def __init__(self, *arguments, **keywords):
        super().__init__(*arguments, **keywords)
        self.passed = []

    def startTest(self, test):
        signal.alarm(TEST_SECONDS)
        super().startTest(test)

    def stopTest(self, test):
        signal.alarm(0)
        super().stopTest(test)

    def addSuccess(self, test):
        super().addSuccess(test)
        self.passed.append(str(test))


def load(name, path, source):
    """The module run from its source under its own name."""
    module = types.ModuleType(name)
    module.__file__ = path
    is_package = path.endswith("__init__.py")
    module.__package__ = name if is_package else name.rpartition(".")[0]
    if is_package:
        module.__path__ = [os.path.dirname(path)]
    # Registered while it runs, as an import would: a dataclass looks its
    # module up by name to read its annotations. Not left there, so that no
    # other module imports music21's own code in place of the provider's.
    previous = sys.modules.get(name)
    sys.modules[name] = module
    try:
        exec(compile(source, path, "exec"), module.__dict__)
    finally:
        if previous is None:
            sys.modules.pop(name, None)
        else:
            sys.modules[name] = previous
    return module


def provided_names(name):
    """What the provider answers for the module, or nothing."""
    try:
        provided = importlib.import_module(name)
    except ImportError:
        return {}
    if getattr(getattr(provided, "__spec__", None), "origin", None) != "music21-rs":
        return {}
    return {
        key: value for key, value in vars(provided).items() if not key.startswith("_")
    }


def is_test_module(name):
    """Whether a module only holds tests of others', as `test_base` does."""
    last = name.rpartition(".")[2]
    return last.startswith("test") or last == "tests"


def gather(module):
    """The module's `Test` class and its docstrings, the docstrings found
    before anything is put over the classes that carry them."""
    suite = unittest.TestSuite()
    case = getattr(module, "Test", None)
    if isinstance(case, type) and issubclass(case, unittest.TestCase):
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(case))
    finder = doctest.DocTestFinder(exclude_empty=True)
    try:
        examples = finder.find(module, module.__name__)
    except Exception:
        examples = []
    flags = doctest.ELLIPSIS | doctest.NORMALIZE_WHITESPACE
    for example in sorted(examples, key=lambda found: found.name):
        if example.examples:
            suite.addTest(doctest.DocTestCase(example, optionflags=flags))
    return suite


def leaves(suite):
    for child in suite:
        if isinstance(child, unittest.TestSuite):
            yield from leaves(child)
        else:
            yield child


def run_module(name, path, source, package_names):
    """How one module fared: blocked at its import, or its tests' results."""
    record = {"module": name, "imported": False, "provided": False, "blocker": None, "error": None,
              "run": 0, "passed": [], "failures": [], "errors": [], "blockers": {},
              "detail": {}}
    try:
        module = load(name, path, source)
    except BaseException as error:
        record["error"] = "".join(traceback.format_exception_only(error)).strip()
        if isinstance(error, (ImportError, AttributeError)):
            record["blocker"] = blocker_of(type(error).__name__, str(error))
        if record["blocker"] is None:
            record["blocker"] = f"<{type(error).__name__} importing {name}>"
        return record
    record["imported"] = True

    suite = gather(module)
    overlay = provided_names(name)
    module.__dict__.update(overlay)
    record["provided"] = bool(overlay) or is_test_module(name)
    for test in leaves(suite):
        if isinstance(test, doctest.DocTestCase):
            globs = dict(package_names)
            globs.update(module.__dict__)
            test._dt_test.globs = globs

    result = Result()
    try:
        suite.run(result)
    except BaseException as error:
        record["error"] = f"the run stopped: {error!r}"
    signal.alarm(0)
    record["run"] = result.testsRun
    record["passed"] = result.passed
    for kind, pairs in (("failures", result.failures), ("errors", result.errors)):
        for test, text in pairs:
            test_name = str(test)
            record[kind].append(test_name)
            record["detail"][test_name] = text[-2000:]
            blocker = blocker_in(text, isinstance(test, doctest.DocTestCase))
            if blocker is not None:
                record["blockers"][test_name] = blocker
    return record


def package_globals(music21):
    """What a docstring of music21's sees: the package's names and its
    modules, as music21's own runner gives it a copy of the package, whose
    `__init__` has imported every module. The provided package imports its
    modules on first asking, so each is asked for here."""
    names = {}
    for key in dir(music21):
        if key.startswith("_"):
            continue
        try:
            names[key] = getattr(music21, key)
        except (ImportError, AttributeError):
            continue
    return names


def run(root, only):
    """Every module's record, music21 answered by the provider throughout."""
    music21_rs.provide_music21()
    import music21

    package_names = package_globals(music21)
    previous = signal.signal(signal.SIGALRM, _alarm)
    package = os.path.join(root, "music21")
    for entry in os.listdir(package):
        name = entry[: -len(".py")] if entry.endswith(".py") else entry
        if name.isidentifier() and not name.startswith("_"):
            MODULES.add(name)
    only = [part for part in (only or "").split(",") if part]
    records = []
    try:
        for name, path, source in module_files(root, only):
            records.append(run_module(name, path, source, package_names))
    finally:
        signal.signal(signal.SIGALRM, previous)
    return records
