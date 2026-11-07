"""music21-shaped Python classes over the music21-rs crate.

The classes come from the extension module beside this file and are imported
with the package. The exceptions are not: each is built on music21's own
`Music21Exception`, so building one imports music21, and a wheel that needs
no music21 to run should not import it for a name nobody has asked for. They
are handed out on first asking instead, which is what `__getattr__` is for.
"""

from .music21_rs import *
from .music21_rs import __all__ as _extension_all
from .music21_rs import __doc__ as _extension_doc
from .music21_rs import __exception_names__ as _exception_names

__doc__ = _extension_doc
# The extension lists its private helpers too (`_thawed`, which pickles call,
# and music21's `_sharpsToPitchCache`), so that the star import above brings
# them in. They stay importable but are not part of the public API.
__all__ = sorted(
    name for name in [*_extension_all, *_exception_names] if not name.startswith("_")
)


# music21's modules the package has in Python, beside the extension: each is
# imported on first asking, as `import music21` makes `music21.converter`
# reachable, without the cost when nobody asks.
_SUBMODULES = ("converter",)


def __getattr__(name):
    if name in _SUBMODULES:
        import importlib

        return importlib.import_module(f"{__name__}.{name}")
    if name in _exception_names:
        from . import music21_rs as _extension

        exception = getattr(_extension, name)
        globals()[name] = exception
        return exception
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
