"""The sidecar's production tree as the packages stage it (RB-080-25, RB-099-12).

Everything the root's dependencies reach in a package-lock.json, by Node's lookup, but the
excluded packages (RB_SIDECAR_EXCLUDED in scripts/lib/package-stage.sh; a name ending in `/` is a
scope) and what only they reach. scripts/lib/package-stage.sh stages exactly these packages
(rb_sidecar_packages), scripts/lib/licenses.py lists exactly these as shipped.

    python3 -I scripts/lib/sidecar_packages.py <package-lock.json> [<excluded>...]

prints the package paths (`node_modules/...`), sorted, one per line.
"""

import json
import sys


def name_of(path: str) -> str:
    """The package name of a `packages` key: the part after the last `node_modules/`."""
    return path.rsplit("node_modules/", 1)[-1]


def left_out(name: str, excluded: list[str]) -> bool:
    return any(name.startswith(x) if x.endswith("/") else name == x for x in excluded)


def staged(packages: dict, excluded: list[str]) -> set[str]:
    """The package paths of lock file `packages` the sidecar ships."""

    def resolve(start: str, name: str) -> str | None:
        base = start
        while True:
            candidate = f"{base}/node_modules/{name}" if base else f"node_modules/{name}"
            if candidate in packages:
                return candidate
            if not base:
                return None
            cut = base.rfind("/node_modules/")
            base = base[:cut] if cut >= 0 else ""

    def needs(path: str) -> list[str]:
        entry = packages[path]
        return [*entry.get("dependencies", {}), *entry.get("optionalDependencies", {})]

    found: set[str] = set()
    todo = [resolve("", name) for name in needs("")]
    while todo:
        path = todo.pop()
        if path is None or path in found or left_out(name_of(path), excluded):
            continue
        found.add(path)
        todo.extend(resolve(path, name) for name in needs(path))
    return found


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: sidecar_packages.py <package-lock.json> [<excluded>...]", file=sys.stderr)
        return 2
    with open(argv[1], encoding="utf-8") as handle:
        packages = json.load(handle)["packages"]
    print("\n".join(sorted(staged(packages, argv[2:]))))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
