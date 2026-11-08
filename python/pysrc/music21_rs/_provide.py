"""`import music21` answered by this package, where music21 is not installed.

A library written for music21 imports music21 by name. With music21
installed, `install_into_music21()` puts this package's classes inside it.
Without it, `provide_music21()` stands in for it: it adds an import hook that
answers `music21` and each of music21's modules this package has classes or
functions for with a module built from them, under music21's names. Nothing
happens until it is called, or until `music21_rs` is imported with
`MUSIC21_RS_PROVIDE_MUSIC21=1` in the environment; the `.pth` file the wheel
puts in site-packages imports it then.

Only what the package has is answered. A module of music21's it has nothing
for raises `ModuleNotFoundError` saying so, rather than being faked, and a
name a provided module lacks is an `AttributeError` naming what is there.
"""

import importlib
import importlib.abc
import importlib.machinery
import importlib.util
import sys
import warnings

from . import _homes
from . import music21_rs as _extension

ENVIRONMENT_VARIABLE = "MUSIC21_RS_PROVIDE_MUSIC21"

# A package of music21's also carries the names of these submodules of its
# own: `music21.stream.Stream` is `music21.stream.base.Stream`, and
# `music21.meter.MeterSequence` is `music21.meter.core.MeterSequence`.
# `_homes.ALSO` says so name by name now; this is what was done before.
_REEXPORTED_CHILDREN = ("base", "core")


class Music21Exception(Exception):
    """What every exception music21 raises is, as music21 has it."""


Music21Exception.__module__ = "music21.exceptions21"


class Music21DeprecationWarning(UserWarning):
    """music21's warning for a member on its way out."""


Music21DeprecationWarning.__module__ = "music21.exceptions21"


def provide_music21():
    """Answer `import music21` from this package, music21 not being installed.

    Code written for music21 then runs on these classes unchanged, as far as
    the names it imports are ones this package has::

        import music21_rs
        music21_rs.provide_music21()

        from music21 import chord
        chord.Chord('C4 E4 G4').commonName   # 'major triad'

    It refuses, with `ImportError`, where music21 itself can be imported:
    `install_into_music21()` is the way to run music21 on these classes
    then. Calling it again does nothing.

    Call it before anything that imports music21's exceptions, so that the
    package's exceptions are built on the `Music21Exception` it provides.
    """
    if providing():
        return
    _refuse_real_music21()
    # In before the rest is worked out: working it out builds the package's
    # exceptions, and building one imports `music21.exceptions21` for its
    # base, which has to be the one answered here.
    finder = _Provider({"music21": {}, "music21.exceptions21": _EXCEPTIONS21})
    sys.meta_path.insert(0, finder)
    finder.update(_contents())


def providing():
    """Whether `provide_music21()` is answering `import music21`."""
    return any(isinstance(finder, _Provider) for finder in sys.meta_path)


def _refuse_real_music21():
    loaded = sys.modules.get("music21")
    if loaded is not None:
        raise ImportError(
            f"music21 is already imported (from {_where(loaded)}), so music21-rs "
            "will not stand in for it; music21_rs.install_into_music21() runs "
            "music21 on this package's classes instead"
        )
    spec = importlib.util.find_spec("music21")
    # A directory called music21 with no `__init__.py` is a namespace
    # package, which is not music21: a checkout of this project has one.
    if spec is not None and spec.origin is not None and spec.loader is not None:
        raise ImportError(
            f"music21 is installed ({spec.origin}), so music21-rs will not "
            "stand in for it; music21_rs.install_into_music21() runs music21 "
            "on this package's classes instead"
        )


def _where(module):
    return getattr(module, "__file__", None) or repr(module)


_EXCEPTIONS21 = {
    "Music21Exception": Music21Exception,
    "Music21DeprecationWarning": Music21DeprecationWarning,
}

# What music21's `exceptions21` defines, each answered by the package's own
# class of that name.
_EXCEPTIONS21_NAMES = (
    "StreamException",
    "ImmutableStreamException",
    "MetadataException",
    "AnalysisException",
    "TreeException",
    "InstrumentException",
    "SubConverterException",
    "GroupException",
    "MeterException",
    "TimeSignatureException",
)

# What every module object has of its own, which a built module's names
# must not put over the provided module's.
_MODULE_SPECIALS = frozenset(
    ("__name__", "__doc__", "__loader__", "__spec__", "__package__", "__all__")
)

# A module of music21's whose names the package keeps on its package: the
# stream classes music21 defines in `stream.base` are `music21.stream`'s
# here, and are answered under both names.
_CHILDREN_OF_PACKAGE = {"music21.stream.base": "music21.stream"}

# music21's modules the package has in Python beside the extension, each by
# the name of the package's own module.
_PYTHON_MODULES = {
    "music21.converter": "converter",
    "music21.corpus": "corpus",
}


def _contents(homes=True, children=False):
    """Every module this package answers for, mapped to its names.

    A class says which module of music21's it belongs to; a function or a
    constant does not, and `_homes` says it for them, with the names music21
    imports into each module from where they live. `homes` false leaves the
    table out, which is what `_homes` is worked out against, and `children`
    copies each `base` and `core` module's names into its package instead,
    as this did before there was a table.
    """
    contents = {"music21": {}, "music21.exceptions21": dict(_EXCEPTIONS21)}
    for module, names in _extension.__music21_modules__:
        for name in names:
            value = getattr(_extension, name, None)
            if value is not None:
                contents.setdefault(module, {})[name] = value
    _warn_on_unprovided_base()
    for name in [*_extension.__all__, *_extension.__exception_names__]:
        if name.startswith("_"):
            continue
        value = getattr(_extension, name)
        module = getattr(value, "__module__", None)
        if not isinstance(module, str) or not module.startswith("music21."):
            continue
        contents.setdefault(module, {}).setdefault(name, value)
    for module, own in _PYTHON_MODULES.items():
        written = importlib.import_module(f"{__package__}.{own}")
        for name in written.__all__:
            contents.setdefault(module, {})[name] = getattr(written, name)
    # Built whole by the extension, as modules of their own.
    for module in getattr(_extension, "__music21_built_modules__", ()):
        built = _extension._music21_module(module)
        names = contents.setdefault(module, {})
        for name, value in vars(built).items():
            if name not in _MODULE_SPECIALS:
                names[name] = value
    for name in _EXCEPTIONS21_NAMES:
        exception = getattr(_extension, name, None)
        if exception is not None:
            contents["music21.exceptions21"].setdefault(name, exception)
    # music21 defines it there and imports it into `corpus`.
    contents["music21.exceptions21"]["CorpusException"] = contents["music21.corpus"][
        "CorpusException"
    ]
    if homes:
        known = {"Music21Exception": Music21Exception}
        for table in (_homes.HOMES, _homes.ALSO):
            for module, names in table.items():
                for name in names:
                    value = known.get(name) or getattr(_extension, name)
                    contents.setdefault(module, {}).setdefault(name, value)
    for module in list(contents):
        parts = module.split(".")
        for end in range(1, len(parts)):
            contents.setdefault(".".join(parts[:end]), {})
    for module in sorted(contents, key=len, reverse=True):
        if not children:
            break
        parent, _, child = module.rpartition(".")
        if child in _REEXPORTED_CHILDREN and parent in contents:
            for name, value in contents[module].items():
                contents[parent].setdefault(name, value)
    contents["music21"]["Music21Exception"] = Music21Exception
    return contents


def _warn_on_unprovided_base():
    # The package's exceptions are built on whatever `Music21Exception`
    # was there when the first of them was built, and a class cannot be
    # rebuilt on another base afterwards.
    if not issubclass(_extension.ChordException, Music21Exception):
        warnings.warn(
            "music21_rs's exceptions were built before provide_music21() was "
            "called, so catching music21.exceptions21.Music21Exception will not "
            "catch them",
            stacklevel=4,
        )


class _Provider(importlib.abc.MetaPathFinder, importlib.abc.Loader):
    """The import hook: finds music21's modules here, and builds them."""

    def __init__(self, contents):
        self.contents = contents
        self.children = {"music21": {"exceptions21"}}

    def update(self, contents):
        """Answers for these modules from now on, and fills in any of them
        already imported while they were being worked out."""
        self.contents = contents
        self.children = {}
        for module in contents:
            parent, _, child = module.rpartition(".")
            if parent:
                self.children.setdefault(parent, set()).add(child)
        for fullname in contents:
            module = sys.modules.get(fullname)
            if module is not None and module.__spec__.loader is self:
                self.exec_module(module)

    def find_spec(self, fullname, path=None, target=None):
        if fullname != "music21" and not fullname.startswith("music21."):
            return None
        if fullname not in self.contents:
            raise ModuleNotFoundError(self._missing(fullname))
        return importlib.machinery.ModuleSpec(
            fullname,
            self,
            origin="music21-rs",
            is_package=fullname in self.children,
        )

    def _missing(self, fullname):
        parent = fullname.rpartition(".")[0]
        while parent not in self.contents:
            parent = parent.rpartition(".")[0]
        offered = ", ".join(sorted(self.children.get(parent, ()))) or "nothing"
        return (
            f"music21-rs does not provide {fullname}; it answers for music21 "
            f"here, music21 itself not being installed, and its modules under "
            f"{parent} are: {offered}"
        )

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        fullname = module.__name__
        names = self.contents[fullname]
        module.__doc__ = f"music21's {fullname}, as far as music21-rs provides it."
        module.__dict__.update(names)
        module.__all__ = sorted(name for name in names if not name.startswith("_"))
        if fullname == "music21":
            version = _extension.__version__
            module.__doc__ = (
                f"music21-rs {version} standing in for music21, which is not "
                "installed: music21's modules and names, as far as music21-rs "
                "provides them."
            )
            module.__version__ = version
            module.VERSION = tuple(_version_parts(version))
            module.VERSION_STR = version
            module.__music21_rs__ = True
        module.__getattr__ = self._lazy_children(fullname)
        module.__dir__ = lambda: sorted({*names, *self.children.get(fullname, ())})

    def _lazy_children(self, fullname):
        def __getattr__(name):
            if name in self.children.get(fullname, ()):
                return importlib.import_module(f"{fullname}.{name}")
            raise AttributeError(
                f"module {fullname!r} has no attribute {name!r}: music21-rs, "
                "which answers for music21 here, does not provide it"
            )

        return __getattr__


def _version_parts(version):
    for part in version.split("."):
        digits = "".join(character for character in part if character.isdigit())
        if digits:
            yield int(digits)
