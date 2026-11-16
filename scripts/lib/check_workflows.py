"""The rules of the GitHub workflows and of dependabot.yml (RB-090-04).

Every workflow in .github/workflows/ has `permissions: {}` and a `concurrency` group at its top
level; every job names its own permissions, and asks for `write` only where WRITE_ALLOWED says
(never `write-all`); runner images are named (`ubuntu-24.04`), never `-latest`, which moves
under a release without a commit. No step that reads a secret comes before a build step of its
job (BUILD): a build runs the build scripts of every dependency, which must not find a key or a
certificate on the runner - the secret's step may itself finish the build (sign what is built). .github/dependabot.yml has a `github-actions`, a `cargo` and
an `npm` entry, and the npm entry names every directory of the tree that holds a package.json
(an entry may be a glob such as `/plugins/*`).

The files are read line by line, not as YAML: the workflows are this repository's own and keep
the block style the rules below expect. No PyYAML, so it runs on any runner and in WSL alike.

    python3 -I scripts/lib/check_workflows.py [--root DIR]
"""

from __future__ import annotations

import argparse
import fnmatch
import os
import re
import sys
from pathlib import Path

# (workflow file, job, permission) that may be `write`: the release's publish job writes the
# GitHub release and signs SHA256SUMS keylessly with the job's OIDC token.
WRITE_ALLOWED = {
    ("release.yml", "publish", "contents"),
    ("release.yml", "publish", "id-token"),
}

LATEST = re.compile(r"\b((?:ubuntu|windows|macos)-latest)\b")
# A step that compiles or installs packages: cargo, npm ci/install/run build, or a script that does.
BUILD = re.compile(
    r"(^|[\s;&|(])(cargo\s|npm\s+(ci|install|run\s+build)\b"
    r"|scripts/(package-windows|build-plugins|check)\.sh\b)"
)
# Build output, dependencies and the throw-away spikes (outside the workspace) are not read.
SKIPPED_DIRS = {".git", "node_modules", "target", "dist", "artifacts", "spikes"}


def content_lines(path: Path) -> list[str]:
    """The file's lines without comment-only lines and without trailing comments."""
    lines = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.lstrip().startswith("#"):
            continue
        lines.append(re.sub(r"\s+#\s.*$", "", line).rstrip())
    return lines


def split_jobs(lines: list[str]) -> dict[str, list[str]]:
    """The body lines of each job under the top-level `jobs:` key."""
    jobs: dict[str, list[str]] = {}
    current = None
    in_jobs = False
    for line in lines:
        if line and not line.startswith(" "):
            in_jobs = line == "jobs:"
            current = None
            continue
        if not in_jobs:
            continue
        match = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
        if match:
            current = match.group(1)
            jobs[current] = []
        elif current is not None:
            jobs[current].append(line)
    return jobs


def job_permissions(body: list[str]) -> dict[str, str] | None:
    """The job's permissions as a map, `{"*": value}` for a scalar, None when it names none."""
    for index, line in enumerate(body):
        match = re.match(r"^    permissions:\s*(.*)$", line)
        if not match:
            continue
        value = match.group(1).strip()
        if value:
            return {} if value == "{}" else {"*": value}
        perms = {}
        for follow in body[index + 1 :]:
            entry = re.match(r"^      ([a-z-]+):\s*(\S+)$", follow)
            if not entry:
                break
            perms[entry.group(1)] = entry.group(2)
        return perms
    return None


def split_steps(body: list[str]) -> list[list[str]]:
    """The lines of each step under the job's `steps:`, a step starting at `      - `."""
    steps: list[list[str]] = []
    in_steps = False
    for line in body:
        if re.match(r"^    steps:\s*$", line):
            in_steps = True
            continue
        if in_steps and re.match(r"^    \S", line):
            in_steps = False
        if not in_steps:
            continue
        if line.startswith("      - "):
            steps.append([line])
        elif steps:
            steps[-1].append(line)
    return steps


def step_name(step: list[str]) -> str:
    for line in step:
        match = re.match(r"^      [- ] name:\s*(.+)$", line)
        if match:
            return match.group(1).strip().strip('"')
    return step[0].strip()[2:]


def secret_before_build(name: str, job: str, body: list[str]) -> list[str]:
    """A finding for the first step that reads a secret and a later build step."""
    steps = split_steps(body)
    for index, step in enumerate(steps):
        if not any("secrets." in line for line in step):
            continue
        for later in range(index + 1, len(steps)):
            if any(BUILD.search(line) for line in steps[later]):
                return [
                    f"{name}: job '{job}' reads a secret in step {index + 1} "
                    f"({step_name(step)}) before the build in step {later + 1} "
                    f"({step_name(steps[later])})"
                ]
    return []


def check_workflow(path: Path) -> tuple[list[str], int]:
    name = path.name
    lines = content_lines(path)
    top = [line for line in lines if line and not line.startswith(" ")]
    findings = []
    if "permissions: {}" not in top or sum(l.startswith("permissions:") for l in top) != 1:
        findings.append(f"{name}: no top-level 'permissions: {{}}'")
    if not any(line.startswith("concurrency:") for line in top):
        findings.append(f"{name}: no top-level 'concurrency'")
    jobs = split_jobs(lines)
    if not jobs:
        findings.append(f"{name}: no jobs")
    for job, body in jobs.items():
        perms = job_permissions(body)
        if perms is None:
            findings.append(f"{name}: job '{job}' names no permissions")
            perms = {}
        for perm, value in perms.items():
            if perm == "*" and value != "read-all":
                findings.append(f"{name}: job '{job}' asks for {value}")
            elif value == "write" and (name, job, perm) not in WRITE_ALLOWED:
                findings.append(f"{name}: job '{job}' asks for {perm}: write")
        for line in body:
            latest = LATEST.search(line)
            if latest:
                findings.append(f"{name}: job '{job}' runs on {latest.group(1)}")
        findings += secret_before_build(name, job, body)
    return findings, len(jobs)


def dependabot_entries(path: Path) -> dict[str, list[str]]:
    """Each package ecosystem and the directories its entries name."""
    entries: dict[str, list[str]] = {}
    ecosystem = None
    in_list = False
    for line in content_lines(path):
        match = re.match(r"^\s*- package-ecosystem:\s*\"?([a-z-]+)\"?$", line)
        if match:
            ecosystem = match.group(1)
            entries.setdefault(ecosystem, [])
            in_list = False
            continue
        if ecosystem is None:
            continue
        single = re.match(r"^\s+directory:\s*\"?([^\"]+)\"?$", line)
        if single:
            entries[ecosystem].append(single.group(1))
            in_list = False
        elif re.match(r"^\s+directories:\s*$", line):
            in_list = True
        elif in_list and (item := re.match(r"^\s+- \"?([^\"]+)\"?$", line)):
            entries[ecosystem].append(item.group(1))
        else:
            in_list = False
    return entries


def npm_directories(root: Path) -> list[str]:
    """Every directory of the tree that holds a package.json, as `/ui`."""
    found = []
    for current, dirs, files in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in SKIPPED_DIRS)
        if "package.json" in files:
            relative = Path(current).relative_to(root)
            found.append("/" + relative.as_posix() if relative.parts else "/")
    return found


def check_dependabot(root: Path) -> list[str]:
    path = root / ".github" / "dependabot.yml"
    if not path.is_file():
        return ["dependabot.yml: missing"]
    entries = dependabot_entries(path)
    findings = [
        f"dependabot.yml: no '{ecosystem}' entry"
        for ecosystem in ("github-actions", "cargo", "npm")
        if ecosystem not in entries
    ]
    listed = entries.get("npm", [])
    for directory in npm_directories(root):
        if not any(fnmatch.fnmatchcase(directory, pattern) for pattern in listed):
            findings.append(f"dependabot.yml: npm directory {directory} is not listed")
    return findings


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", default=str(Path(__file__).resolve().parents[2]))
    root = Path(parser.parse_args().root)
    workflows = sorted((root / ".github" / "workflows").glob("*.y*ml"))
    if not workflows:
        print(f"no workflows under {root}/.github/workflows", file=sys.stderr)
        return 2
    findings: list[str] = []
    job_count = 0
    for workflow in workflows:
        found, jobs = check_workflow(workflow)
        findings += found
        job_count += jobs
    findings += check_dependabot(root)
    for finding in findings:
        print(finding)
    if findings:
        print(f"!! {len(findings)} workflow rule(s) broken (scripts/lib/check_workflows.py)", file=sys.stderr)
        return 1
    print(f"    {len(workflows)} workflow(s), {job_count} job(s), dependabot.yml: every rule holds")
    return 0


if __name__ == "__main__":
    sys.exit(main())
