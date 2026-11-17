"""Which files scripts/sign-windows.sh signs (RB-090-03).

    python3 -I scripts/lib/sign_plan.py <file or package folder>...

Prints one NUL-terminated record per file, `<action>\t<label>\t<path>`: `sign`, or `keep` for a
file that already carries an Authenticode signature of its own. Nothing is printed, and the exit
status is 1, when any argument cannot be signed.

- A file named as an argument is signed as asked: a PE image (.exe, .dll, .com) or an MSI.
- A package folder (rbrowser.exe and rbrowser.dll at its top, as scripts/package-windows.sh
  stages it): every PE image and MSI in it - ours and CEF's, which ships unsigned (libcef.dll,
  chrome_elf.dll, ...) - except the Node runtime under node/, which keeps the OpenJS
  Foundation's signature, and a file that is signed already (Microsoft's DirectX DLLs), which
  keeps its own. Locales, resources, the shell, the sidecar's scripts and the plugins are no
  programs and stay as they are.
"""

import os
import struct
import sys

PROGRAMS = {".exe", ".dll", ".com"}
INSTALLERS = {".msi"}
REQUIRED = ("rbrowser.exe", "rbrowser.dll")
OWN_SIGNATURE = {"node"}
CFB_MAGIC = bytes.fromhex("d0cf11e0a1b11ae1")
SECURITY_DIR = 4


def pe_signed(path: str) -> bool | None:
    """Whether the PE image carries a signature (a non-empty security directory); None if no PE."""
    try:
        with open(path, "rb") as image:
            if image.read(2) != b"MZ":
                return None
            image.seek(0x3C)
            (offset,) = struct.unpack("<I", image.read(4))
            image.seek(offset)
            if image.read(4) != b"PE\0\0":
                return None
            image.seek(offset + 24)
            (magic,) = struct.unpack("<H", image.read(2))
            if magic not in (0x10B, 0x20B):
                return None
            base = offset + 24 + (96 if magic == 0x10B else 112)
            image.seek(base - 4)
            (count,) = struct.unpack("<I", image.read(4))
            if count <= SECURITY_DIR:
                return False
            image.seek(base + 8 * SECURITY_DIR)
            _, size = struct.unpack("<II", image.read(8))
            return size > 0
    except (OSError, struct.error):
        return None


def is_msi(path: str) -> bool:
    try:
        with open(path, "rb") as package:
            return package.read(8) == CFB_MAGIC
    except OSError:
        return False


def kind(path: str) -> str:
    return os.path.splitext(path)[1].lower()


def plan_file(path: str, label: str, explicit: bool, errors: list[str]) -> tuple[str, str, str] | None:
    suffix = kind(path)
    if suffix in PROGRAMS:
        signed = pe_signed(path)
        if signed is None:
            errors.append(f"{path} is no PE image")
            return None
        return ("keep" if signed and not explicit else "sign", label, path)
    if suffix in INSTALLERS:
        if not is_msi(path):
            errors.append(f"{path} is no MSI (no compound file)")
            return None
        return ("sign", label, path)
    if explicit:
        errors.append(f"{path} cannot be signed: a .exe, .dll, .com or .msi, a package folder or its ZIP")
    return None


def plan_folder(folder: str, errors: list[str]) -> list[tuple[str, str, str]]:
    missing = [name for name in REQUIRED if not os.path.isfile(os.path.join(folder, name))]
    if missing:
        errors.append(f"{folder} is no package folder: {', '.join(missing)} missing at its top")
        return []
    plan = []
    for top, dirs, files in os.walk(folder):
        if top == folder:
            dirs[:] = [name for name in dirs if name not in OWN_SIGNATURE]
        dirs.sort()
        for name in sorted(files):
            path = os.path.join(top, name)
            entry = plan_file(path, os.path.relpath(path, folder).replace(os.sep, "/"), False, errors)
            if entry:
                plan.append(entry)
    return plan


def main() -> int:
    errors: list[str] = []
    plan: list[tuple[str, str, str]] = []
    for target in sys.argv[1:]:
        if os.path.isdir(target):
            plan += plan_folder(target, errors)
        elif os.path.isfile(target):
            entry = plan_file(target, os.path.basename(target), True, errors)
            if entry:
                plan.append(entry)
        else:
            errors.append(f"{target} does not exist")
    if errors:
        for error in errors:
            print(f"!! {error}", file=sys.stderr)
        return 1
    for action, label, path in plan:
        sys.stdout.write(f"{action}\t{label}\t{path}\0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
