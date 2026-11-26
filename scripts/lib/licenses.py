"""The component licences of the About page, ui/src/about/licenses/third-party.json (RB-010-18), and
their full texts, packaging/THIRD-PARTY-LICENSES.txt (RB-099-12, scripts/lib/licenses_texts.py).

Adapted from rCommander's scripts/lib/licenses.py (RC-010-19), extended by the npm packages of
the shell and by the plugins' own lock files.

Inputs:

* a scratch directory with, per Rust root (the app for each target, the WSL helper rbrowser-wsl,
  every bundled plugin), the output of `cargo metadata --format-version 1` as
  `<n>.metadata.json` and of `cargo tree --prefix none --format '{p}'` as `<n>.tree.txt`
  (`name vX.Y.Z` per line). A locked crate a tree names ships and is listed with its licence
  under `rust`; every other crate of the lock files is named under `rustNotShipped`.
* `ui/package-lock.json`, `agent/` and `sidecar/`: every package that is neither `dev` nor
  `optional` (the optional ones are the platform binaries of the build tools) is listed under
  `npm`, every other one under `npmNotShipped`. Of the sidecar's only what the package stages
  ships: its production tree without RB_SIDECAR_EXCLUDED (scripts/lib/package-stage.sh) and what
  only those need (scripts/lib/sidecar_packages.py).
* the licence files of what ships: a crate's in its source folder (the manifest_path of cargo
  metadata), an npm package's in its folder of `<front end>/node_modules` (`npm ci` there first).

A package that declares no licence takes its entry from ui/src/about/licenses/overrides.json,
`{"rust": {"name@version": {...}}, "npm": {"name@version": {"license": "<SPDX>", "source": "<where
it was read>"}}}`. The bundled binary components (CEF, Node, ...) are a checked file of their own,
ui/src/about/licenses/bundled.json, and packaging/NOTICE.in.

    python3 scripts/lib/licenses.py <repo root> <scratch dir> [--check]

Exit 0 when written (or, with --check, current); 1 when --check finds a file stale; 2 when a
package has no licence and no override, a shipped package has no licence text, an override is
for no locked package, or an input does not read.
"""

import json
import re
import sys
from pathlib import Path

import licenses_texts
from sidecar_packages import staged

TARGET = "ui/src/about/licenses/third-party.json"
OVERRIDES = "ui/src/about/licenses/overrides.json"
# The front ends whose lock files are read: the shell, the sync agent, the sidecar.
NPM_DIRS = ("ui", "agent", "sidecar")
SIDECAR = "sidecar"
STAGE = "scripts/lib/package-stage.sh"


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


def sidecar_excluded(root: Path) -> list[str]:
    """RB_SIDECAR_EXCLUDED of scripts/lib/package-stage.sh, the one list of what the package
    leaves out of the sidecar."""
    found = re.search(r"^RB_SIDECAR_EXCLUDED=\(([^)]*)\)", (root / STAGE).read_text(encoding="utf-8"), re.M)
    if found is None:
        raise ValueError(f"no RB_SIDECAR_EXCLUDED=(...) in {STAGE}")
    return found.group(1).split()


def npm_packages(lock: dict, kept: set[str] | None = None) -> dict[str, tuple[dict, bool, str]]:
    """Every package of one lock file by `name@version`, with whether it ships and its path; with
    `kept`, only the paths in it ship."""
    packages: dict[str, tuple[dict, bool, str]] = {}
    for path, package in lock.get("packages", {}).items():
        if not path or "version" not in package:
            continue
        key = f"{npm_name(path)}@{package['version']}"
        ships = not package.get("dev") and not package.get("optional") and (kept is None or path in kept)
        previous = packages.get(key)
        if previous is None or (ships and not previous[1]):
            packages[key] = (package, ships, path)
    return packages


def entry(key: str, declared: str, overrides: dict, missing: list[str]) -> dict:
    """The row of `key`; an override wins over what the package declares."""
    name, version = key.rsplit("@", 1)
    license = (overrides.get(key) or {}).get("license") or (declared or "").strip()
    if not license:
        missing.append(f"{key}: declares no licence")
    return {"name": name, "version": version, "license": license}


def collect(
    rust_roots: list[tuple[dict, str]], npm_locks: list[tuple[str, dict, set[str] | None]], overrides: dict
) -> tuple[dict, list[dict], list[str]]:
    """The four lists of third-party.json, the shipped packages with their folders (Rust first,
    then npm, each sorted) and what has no licence."""
    rust_overrides = overrides.get("rust", {})
    npm_overrides = overrides.get("npm", {})
    locked: dict[str, dict] = {}
    shipped: set[str] = set()
    for metadata, tree in rust_roots:
        locked.update(locked_crates(metadata))
        shipped |= linked(tree)
    npm: dict[str, tuple[dict, bool, str]] = {}
    for directory, lock, kept in npm_locks:
        for key, (package, ships, path) in npm_packages(lock, kept).items():
            previous = npm.get(key)
            if previous is None or (ships and not previous[1]):
                npm[key] = (package, ships, f"{directory}/{path}")

    missing = [f"rust {key}: an override for no locked crate" for key in sorted(rust_overrides) if key not in locked]
    missing += [f"npm {key}: an override for no locked package" for key in sorted(npm_overrides) if key not in npm]
    order = lambda key: (key.rsplit("@", 1)[0], key.rsplit("@", 1)[1])  # noqa: E731
    lists: dict[str, list] = {"rust": [], "rustNotShipped": [], "npm": [], "npmNotShipped": []}
    packages: list[dict] = []
    for key in sorted(locked, key=order):
        if key not in shipped:
            lists["rustNotShipped"].append(key)
            continue
        crate = locked[key]
        row = entry(key, crate.get("license"), rust_overrides, missing)
        lists["rust"].append(row)
        packages.append(
            {**row, "kind": "rust", "folder": str(Path(crate.get("manifest_path", "")).parent),
             "license_file": crate.get("license_file"), "authors": crate.get("authors") or []}
        )
    for key in sorted(npm, key=order):
        package, ships, path = npm[key]
        if not ships:
            lists["npmNotShipped"].append(key)
            continue
        row = entry(key, package.get("license"), npm_overrides, missing)
        lists["npm"].append(row)
        directory = path.split("/", 1)[0]
        packages.append(
            {**row, "kind": "npm", "folder": path, "front_end": directory,
             "missing_hint": f"not installed in {directory}/node_modules - run npm ci in {directory}/"}
        )
    return lists, packages, missing


def render(lists: dict) -> str:
    """The text of third-party.json."""

    def rows(entries: list) -> str:
        return ",\n".join("    " + json.dumps(row, ensure_ascii=False) for row in entries)

    return (
        "{\n"
        f'  "rust": [\n{rows(lists["rust"])}\n  ],\n'
        f'  "rustNotShipped": [\n{rows(lists["rustNotShipped"])}\n  ],\n'
        f'  "npm": [\n{rows(lists["npm"])}\n  ],\n'
        f'  "npmNotShipped": [\n{rows(lists["npmNotShipped"])}\n  ]\n'
        "}\n"
    )


def read_inputs(root: Path, scratch: Path) -> tuple[list[tuple[dict, str]], list[tuple[str, dict, set[str] | None]], dict]:
    """The Rust roots of the scratch directory, the npm lock files (with the sidecar's staged
    paths) and the overrides."""
    rust_roots = []
    for metadata_path in sorted(scratch.glob("*.metadata.json")):
        tree_path = metadata_path.with_name(metadata_path.name.replace(".metadata.json", ".tree.txt"))
        rust_roots.append(
            (json.loads(metadata_path.read_text(encoding="utf-8")), tree_path.read_text(encoding="utf-8"))
        )
    if not rust_roots:
        raise ValueError(f"no *.metadata.json in {scratch}")
    npm_locks = []
    for directory in NPM_DIRS:
        path = root / directory / "package-lock.json"
        if path.exists():
            lock = json.loads(path.read_text(encoding="utf-8"))
            kept = staged(lock["packages"], sidecar_excluded(root)) if directory == SIDECAR else None
            npm_locks.append((directory, lock, kept))
    overrides_path = root / OVERRIDES
    overrides = json.loads(overrides_path.read_text(encoding="utf-8")) if overrides_path.exists() else {}
    return rust_roots, npm_locks, overrides


def main(argv: list[str]) -> int:
    if len(argv) < 3 or any(arg != "--check" for arg in argv[3:]):
        print("usage: licenses.py <repo root> <scratch dir> [--check]", file=sys.stderr)
        return 2
    root = Path(argv[1])
    try:
        lists, packages, missing = collect(*read_inputs(root, Path(argv[2])))
        for package in packages:
            if package["kind"] == "npm":
                package["folder"] = str(root / package["folder"])
        texts, without = licenses_texts.render(packages, root / licenses_texts.STANDARD)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    if missing or without:
        if missing:
            print(f"error: record each in {OVERRIDES} with the place its licence was read:", file=sys.stderr)
            for line in missing:
                print(f"  {line}", file=sys.stderr)
        if without:
            print(f"error: no licence text for {licenses_texts.TEXTS}:", file=sys.stderr)
            for line in without:
                print(f"  {line}", file=sys.stderr)
        return 2
    outputs = ((TARGET, render(lists)), (licenses_texts.TEXTS, texts))
    if "--check" in argv[3:]:
        stale = [name for name, text in outputs if read(root / name) != text]
        for name in stale:
            print(f"{name} is stale: run scripts/licenses.sh", file=sys.stderr)
        if stale:
            return 1
        print(f"==> {TARGET} and {licenses_texts.TEXTS} are current")
        return 0
    for name, text in outputs:
        target = root / name
        if read(target) == text:
            print(f"==> {name} was already current")
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
        print(f"==> wrote {name}: {len(lists['rust']) + len(lists['npm'])} packages")
    return 0


def read(path: Path) -> str | None:
    return path.read_text(encoding="utf-8") if path.exists() else None


if __name__ == "__main__":
    sys.exit(main(sys.argv))
