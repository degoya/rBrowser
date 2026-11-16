"""Whether a binary carries rb-update, the self-update crate (RB-090-02).

Without the Cargo feature `self-update` the app does not link rb-update (crates/rbrowser/
Cargo.toml) and does not compile `updates/active.rs`, which holds the release address. What
either would leave in a binary, read as raw bytes - the string list of the binary and the
symbol list of its PDB:

- `rb_update`: the crate's name, in every mangled symbol of it (`9rb_update`, v0 mangling) and
  in its module paths (tracing targets, type names);
- `crates/rb-update/` and `crates\\rb-update\\`: its sources, as every panic location in it
  names them (built in WSL or on Windows);
- `releases/latest/download/`: the release address `updates/active.rs` fetches from.

Each file is read with the PDB beside it (`rbrowser.dll` with `rbrowser.pdb`,
`rbrowser-console.exe` with `rbrowser_console.pdb`), when there is one: a test package ships its
symbols there (Cargo.toml, [profile.release-test]).

    python3 scripts/lib/update_code_scan.py [--expect-present] <binary>...

Exit 0 when no file carries a marker (with --expect-present: when one does, the positive control
of a build with the feature), 1 otherwise, 2 on a usage error.
"""

from __future__ import annotations

import mmap
import sys
from pathlib import Path

USAGE = "usage: scripts/check-update-code.sh [--expect-present] <binary>..."

MARKERS: tuple[bytes, ...] = (
    b"rb_update",
    b"crates/rb-update/",
    b"crates\\rb-update\\",
    b"releases/latest/download/",
)


def count(path: Path, marker: bytes) -> int:
    """How often `marker` occurs in the file at `path`."""
    if path.stat().st_size == 0:
        return 0
    with path.open("rb") as handle, mmap.mmap(handle.fileno(), 0, access=mmap.ACCESS_READ) as data:
        found = 0
        at = data.find(marker)
        while at != -1:
            found += 1
            at = data.find(marker, at + len(marker))
        return found


def files_of(binaries: list[Path]) -> list[Path]:
    """The binaries and the PDB beside each, once each, in order. The linker names a PDB after
    the crate's target, hyphens as underscores (`rbrowser-console.exe`, `rbrowser_console.pdb`)."""
    seen: list[Path] = []
    for binary in binaries:
        pdb = binary.with_suffix(".pdb")
        for path in (binary, pdb, pdb.with_name(pdb.name.replace("-", "_"))):
            if path.is_file() and path not in seen:
                seen.append(path)
    return seen


def main(argv: list[str]) -> int:
    expect_present = False
    binaries: list[Path] = []
    for arg in argv:
        if arg == "--expect-present":
            expect_present = True
        elif arg.startswith("-"):
            print(USAGE, file=sys.stderr)
            return 2
        else:
            binaries.append(Path(arg))
    if not binaries:
        print(USAGE, file=sys.stderr)
        return 2
    missing = [str(binary) for binary in binaries if not binary.is_file()]
    if missing:
        print(f"!! no such file: {', '.join(missing)}", file=sys.stderr)
        return 2

    files = files_of(binaries)
    hits = 0
    for path in files:
        for marker in MARKERS:
            found = count(path, marker)
            if found:
                hits += found
                print(f"{path}: '{marker.decode()}' {found}x")
    if expect_present:
        if hits:
            print(f"==> rb-update found in {len(files)} files ({hits} occurrences)")
            return 0
        print(f"!! no update code in {len(files)} files, expected some", file=sys.stderr)
        return 1
    if hits:
        print(f"!! update code in a build without self-update ({hits} occurrences)", file=sys.stderr)
        return 1
    print(f"==> no update code in {len(files)} files ({len(MARKERS)} markers)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
