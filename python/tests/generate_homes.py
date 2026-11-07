"""Works out `music21_rs/_homes.py` from music21 itself.

`provide_music21()` places a class by its module string. A function or a
constant has none, so this finds, for each name the package exports and
places nowhere, the module of music21's that defines it: a function or class
by its `__module__`, a constant by the module whose source assigns it. A
name music21 defines in two places is an error unless `CHOSEN` settles it,
and one it defines nowhere is listed in `NO_HOME`, the package's own.

It then lists, for each module the package answers for, the names music21
imports into it from where they live -- `music21.stream.StreamException`,
`music21.search.noteNameSearch` -- so that those resolve as in music21.

    python generate_homes.py            # rewrites _homes.py
    python generate_homes.py --check    # fails if _homes.py is stale
    python generate_homes.py --coverage # music21's names against ours

Both need music21 importable as well as music21_rs.
"""

import argparse
import importlib
import pathlib
import pkgutil
import re
import sys
import types
import warnings

# Names music21 defines in more than one module, and the one the package's
# own is: `features.jSymbolic` has an `extractorsById` of its own library.
CHOSEN = {
    "extractorsById": "music21.features.base",
}

HOMES_FILE = (
    pathlib.Path(__file__).resolve().parents[1] / "pysrc" / "music21_rs" / "_homes.py"
)


def music21_modules():
    """Every module of music21's by name, tests left out."""
    import music21

    modules = {"music21": music21}
    for found in pkgutil.walk_packages(music21.__path__, "music21."):
        if found.name.rpartition(".")[2].startswith("test"):
            continue
        modules[found.name] = importlib.import_module(found.name)
    return modules


def defines(module, name):
    """Whether `module` is where music21 defines `name`, not imports it."""
    if name not in vars(module):
        return False
    value = vars(module)[name]
    if isinstance(value, types.ModuleType):
        return False
    home = getattr(value, "__module__", None)
    # A class, a function, or one wrapped as `lru_cache` wraps them.
    if callable(value) and isinstance(home, str):
        return home == module.__name__
    source = getattr(module, "__file__", None)
    if source is None:
        return False
    text = pathlib.Path(source).read_text(encoding="utf-8")
    return re.search(rf"^{re.escape(name)}\s*[:=]", text, re.MULTILINE) is not None


def placed_names(contents):
    return {name for names in contents.values() for name in names}


def work_out(modules):
    """The three tables `_homes.py` holds."""
    from music21_rs import _provide
    from music21_rs import music21_rs as extension

    own = _provide._contents(homes=False)
    placed = placed_names(own)
    homes, no_home = {}, []
    exported = [*extension.__all__, *extension.__exception_names__]
    for name in sorted(set(exported) - placed):
        found = [
            module_name
            for module_name, module in modules.items()
            if defines(module, name)
        ]
        if name in CHOSEN:
            if CHOSEN[name] not in found:
                raise SystemExit(f"music21 does not define {name} in {CHOSEN[name]}")
            found = [CHOSEN[name]]
        if len(found) > 1:
            raise SystemExit(f"music21 defines {name} in {found}: add it to CHOSEN")
        if found:
            homes.setdefault(found[0], []).append(name)
        else:
            no_home.append(name)

    provided = {module: dict(names) for module, names in own.items()}
    for module, names in homes.items():
        provided.setdefault(module, {}).update(dict.fromkeys(names))
    theirs = {}
    for module_name, names in provided.items():
        module = modules.get(module_name)
        for name in names:
            if module is not None and name in vars(module):
                theirs.setdefault(name, set()).add(id(vars(module)[name]))
    also = {}
    for module_name in sorted(provided):
        module = modules.get(module_name)
        if module is None:
            continue
        for name, value in vars(module).items():
            if name in provided[module_name] or name not in theirs:
                continue
            if isinstance(value, types.ModuleType) or name.startswith("__"):
                continue
            if id(value) in theirs[name]:
                also.setdefault(module_name, []).append(name)
    return homes, also, no_home


def render(homes, also, no_home):
    def table(entries):
        lines = ["{"]
        for module in sorted(entries):
            lines.append(f"    {module!r}: (")
            lines.extend(f"        {name!r}," for name in sorted(entries[module]))
            lines.append("    ),")
        lines.append("}")
        return "\n".join(lines)

    names = "\n".join(f"    {name!r}," for name in sorted(no_home))
    return f'''"""Where music21 keeps each function and constant this package exports.

Generated from music21 by `python/tests/generate_homes.py`; run it again
rather than editing this by hand.

`HOMES` is the module of music21's that defines each name `provide_music21`
would otherwise place nowhere. `ALSO` is the names music21 imports into a
module the package answers for from where they live. `NO_HOME` is the
package's own names, which music21 has nowhere.
"""

HOMES = {table(homes)}

ALSO = {table(also)}

NO_HOME = (
{names}
)
'''


def coverage(modules):
    """For each module provided: music21's public names in it, and how
    many of them are provided without `_homes` and with it."""
    from music21_rs import _provide

    before = with_children(_provide._contents(homes=False, children=True))
    after = with_children(_provide._contents())
    rows = []
    for module_name in sorted(after):
        module = modules.get(module_name)
        if module is None:
            continue
        public = set(getattr(module, "__all__", None) or public_names(module))
        rows.append(
            (
                module_name,
                len(public),
                len(public & before.get(module_name, set())),
                len(public & after[module_name]),
            )
        )
    return rows


def public_names(module):
    """A module's names that are not private, not modules, and not
    imported from outside music21."""
    return [
        name
        for name, value in vars(module).items()
        if not name.startswith("_")
        and not isinstance(value, types.ModuleType)
        and str(getattr(value, "__module__", "music21")).startswith("music21")
    ]


def with_children(contents):
    """Each module's names with its submodules', as `music21.stream`
    answers for `iterator`."""
    names = {module: set(found) for module, found in contents.items()}
    for module in contents:
        parent, _, child = module.rpartition(".")
        if parent in names:
            names[parent].add(child)
    return names


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--coverage", action="store_true")
    arguments = parser.parse_args()
    warnings.simplefilter("ignore")
    modules = music21_modules()
    if arguments.coverage:
        rows = coverage(modules)
        print(f"{'module':45} {'music21':>7} {'before':>6} {'after':>6}")
        for module_name, public, before, after in rows:
            print(f"{module_name:45} {public:7} {before:6} {after:6}")
        totals = [sum(row[column] for row in rows) for column in (1, 2, 3)]
        print(f"{'total':45} {totals[0]:7} {totals[1]:6} {totals[2]:6}")
        return 0
    written = render(*work_out(modules))
    if arguments.check:
        if HOMES_FILE.read_text(encoding="utf-8") != written:
            print(f"{HOMES_FILE} is stale: run {__file__}", file=sys.stderr)
            return 1
        return 0
    HOMES_FILE.write_text(written, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
