"""The component licences of the About page, ui/src/about/licenses/third-party.json (RB-010-18).

Adapted from rCommander's scripts/lib/licenses.py (RC-010-19), extended by the npm packages of
the shell and by the plugins' own lock files.

Inputs:

* a scratch directory with, per Rust root (the app for each target, the WSL helper rbrowser-wsl,
  every bundled plugin), the output of `cargo metadata --format-version 1` as
  `<n>.metadata.json` and of `cargo tree --prefix none --format '{p}'` as `<n>.tree.txt`
  (`name vX.Y.Z` per line). A locked crate a tree names ships and is listed with its licence
  under `rust`; every other crate of the lock files is named under `rustNotShipped`.
* `ui/package-lock.json` (and `agent/`, `sidecar/` once they exist): every package that is
  neither `dev` nor `optional` (the optional ones are the platform binaries of the build tools)
  is listed under `npm`, every other one under `npmNotShipped`.

A package that declares no licence takes its entry from ui/src/about/licenses/overrides.json,
`{"rust": {"name@version": {...}}, "npm": {"name@version": {"license": "<SPDX>", "source": "<where
it was read>"}}}`. The bundled binary components (CEF, Node, ...) are a checked file of their own,
ui/src/about/licenses/bundled.json.

    python3 scripts/lib/licenses.py <repo root> <scratch dir> [--check]

Exit 0 when written (or, with --check, current); 1 when --check finds the list stale; 2 when a
package has no licence and no override, an override is for no locked package, or an input does
not read.
"""

import json
import sys
from pathlib import Path

TARGET = "ui/src/about/licenses/third-party.json"
OVERRIDES = "ui/src/about/licenses/overrides.json"
NPM_LOCKS = ("ui/package-lock.json", "agent/package-lock.json", "sidecar/package-lock.json")


def linked(text: str) -> set[str]:
    """`name@version` of every crate a `cargo tree` output names."""
    crates = set()
    for line in text.splitlines():
        fields = line.split()
        if len(fields) >= 2 and fields[1].startswith("v"):
            crates.add(f"{fields[0]}@{fields[1][1:]}")
    return crates


def locked_crates(metadata: dict) -> dict[str, dict]:
    """The registry and git crates of one `cargo metadata`, by `name@version`; a package without
    a source is a workspace member or a path dependency, rBrowser itself."""
    return {
        f"{package['name']}@{package['version']}": package
        for package in metadata["packages"]
        if package.get("source") is not None
    }


def npm_name(path: str) -> str:
    """The package name of a `packages` key: the part after the last `node_modules/`."""
    return path.rsplit("node_modules/", 1)[-1]


def npm_packages(lock: dict) -> dict[str, tuple[dict, bool]]:
    """Every package of one lock file by `name@version`, with whether it ships."""
    packages = {}
    for path, package in lock.get("packages", {}).items():
        if not path or "version" not in package:
            continue
        key = f"{npm_name(path)}@{package['version']}"
        ships = not package.get("dev") and not package.get("optional")
        previous = packages.get(key)
        packages[key] = (package, ships or (previous is not None and previous[1]))
    return packages


def entry(key: str, declared: str, overrides: dict, missing: list[str]) -> dict:
    """The row of `key`; an override wins over what the package declares."""
    name, version = key.rsplit("@", 1)
    license = (overrides.get(key) or {}).get("license") or (declared or "").strip()
    if not license:
        missing.append(f"{key}: declares no licence")
    return {"name": name, "version": version, "license": license}


def render(rust_roots: list[tuple[dict, str]], npm_locks: list[dict], overrides: dict) -> tuple[str, list[str]]:
    """The file's text and what has no licence."""
    rust_overrides = overrides.get("rust", {})
    npm_overrides = overrides.get("npm", {})
    locked: dict[str, dict] = {}
    shipped: set[str] = set()
    for metadata, tree in rust_roots:
        locked.update(locked_crates(metadata))
        shipped |= linked(tree)
    npm: dict[str, tuple[dict, bool]] = {}
    for lock in npm_locks:
        for key, (package, ships) in npm_packages(lock).items():
            previous = npm.get(key)
            npm[key] = (package, ships or (previous is not None and previous[1]))

    missing = [f"rust {key}: an override for no locked crate" for key in sorted(rust_overrides) if key not in locked]
    missing += [f"npm {key}: an override for no locked package" for key in sorted(npm_overrides) if key not in npm]
    order = lambda key: (key.rsplit("@", 1)[0], key.rsplit("@", 1)[1])  # noqa: E731
    rust, rust_not_shipped = [], []
    for key in sorted(locked, key=order):
        if key in shipped:
            rust.append(entry(key, locked[key].get("license"), rust_overrides, missing))
        else:
            rust_not_shipped.append(key)
    node, node_not_shipped = [], []
    for key in sorted(npm, key=order):
        package, ships = npm[key]
        if ships:
            node.append(entry(key, package.get("license"), npm_overrides, missing))
        else:
            node_not_shipped.append(key)

    def rows(entries: list) -> str:
        return ",\n".join("    " + json.dumps(row, ensure_ascii=False) for row in entries)

    text = (
        "{\n"
        f'  "rust": [\n{rows(rust)}\n  ],\n'
        f'  "rustNotShipped": [\n{rows(rust_not_shipped)}\n  ],\n'
        f'  "npm": [\n{rows(node)}\n  ],\n'
        f'  "npmNotShipped": [\n{rows(node_not_shipped)}\n  ]\n'
        "}\n"
    )
    return text, missing


def read_inputs(root: Path, scratch: Path) -> tuple[list[tuple[dict, str]], list[dict], dict]:
    """The Rust roots of the scratch directory, the npm lock files and the overrides."""
    rust_roots = []
    for metadata_path in sorted(scratch.glob("*.metadata.json")):
        tree_path = metadata_path.with_name(metadata_path.name.replace(".metadata.json", ".tree.txt"))
        rust_roots.append(
            (json.loads(metadata_path.read_text(encoding="utf-8")), tree_path.read_text(encoding="utf-8"))
        )
    if not rust_roots:
        raise ValueError(f"no *.metadata.json in {scratch}")
    npm_locks = [
        json.loads((root / lock).read_text(encoding="utf-8")) for lock in NPM_LOCKS if (root / lock).exists()
    ]
    overrides_path = root / OVERRIDES
    overrides = json.loads(overrides_path.read_text(encoding="utf-8")) if overrides_path.exists() else {}
    return rust_roots, npm_locks, overrides


def main(argv: list[str]) -> int:
    if len(argv) < 3 or any(arg != "--check" for arg in argv[3:]):
        print("usage: licenses.py <repo root> <scratch dir> [--check]", file=sys.stderr)
        return 2
    root = Path(argv[1])
    try:
        text, missing = render(*read_inputs(root, Path(argv[2])))
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    if missing:
        print(f"error: record each in {OVERRIDES} with the place its licence was read:", file=sys.stderr)
        for line in missing:
            print(f"  {line}", file=sys.stderr)
        return 2
    target = root / TARGET
    current = target.read_text(encoding="utf-8") if target.exists() else None
    if "--check" in argv[3:]:
        if current != text:
            print(f"{TARGET} is stale: run scripts/licenses.sh", file=sys.stderr)
            return 1
        print(f"==> {TARGET} is current")
        return 0
    if current == text:
        print(f"==> {TARGET} was already current")
    else:
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
        print(f"==> wrote {TARGET}: {text.count(chr(34) + 'license' + chr(34))} packages")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
