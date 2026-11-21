#!/usr/bin/env python3
"""Compacts a schema.org release into plugins/schema-org/data/vocabulary.json (RB-090-96).

    python3 -I scripts/lib/schema_org_vocabulary.py <release.jsonld> <out.json> \
        --version X --url U --sha256 H [--check]

The release is `schemaorg-current-https.jsonld` of one schema.org version; its SHA-256 must be
the pinned one (exit 2 otherwise, nothing written). Kept: every schema.org class with its
schema.org supertypes (`types`), the classes that are data types (`dataTypes`), the enumeration
members with their enumerations (`members`) and every property with its `domainIncludes` (`d`),
`rangeIncludes` (`r`) and `supersededBy` (`s`). Everything of another vocabulary is left out.
One entry per line, sorted, so a new release reads as a diff.

--check writes nothing and exits 1 when the file is not what a run would write.
scripts/schema-org-vocabulary.sh pins the release and downloads it.
"""

import argparse
import hashlib
import json
import os
import sys

PREFIX = "schema:"
LICENSE = (
    "schema.org vocabulary, Creative Commons Attribution-ShareAlike 3.0 (CC BY-SA 3.0), "
    "https://schema.org/docs/terms.html"
)


def ids(value):
    """The `@id`s of a JSON-LD reference or list of references."""
    if isinstance(value, dict):
        value = [value]
    if not isinstance(value, list):
        return []
    return [item["@id"] for item in value if isinstance(item, dict) and isinstance(item.get("@id"), str)]


def own(names):
    """The schema.org names of `names`, short, sorted, each once."""
    return sorted({name[len(PREFIX):] for name in names if name.startswith(PREFIX)})


def kinds(node):
    value = node.get("@type", [])
    return [value] if isinstance(value, str) else [kind for kind in value if isinstance(kind, str)]


def compact(release, version, url, digest):
    """The compact vocabulary of a release's `@graph`."""
    graph = release.get("@graph", [])
    types, data_types, members, properties = {}, [], {}, {}
    for node in graph:
        name = node.get("@id", "")
        if not isinstance(name, str) or not name.startswith(PREFIX):
            continue
        short = name[len(PREFIX):]
        node_kinds = kinds(node)
        if "rdfs:Class" in node_kinds:
            types[short] = own(ids(node.get("rdfs:subClassOf")))
            if "schema:DataType" in node_kinds:
                data_types.append(short)
        elif "rdf:Property" in node_kinds:
            entry = {"d": own(ids(node.get("schema:domainIncludes"))), "r": own(ids(node.get("schema:rangeIncludes")))}
            successor = own(ids(node.get("schema:supersededBy")))
            if successor:
                entry["s"] = successor[0]
            properties[short] = entry
        else:
            enumerations = own(node_kinds)
            if enumerations:
                members[short] = enumerations
    return {
        "version": version,
        "source": url,
        "sha256": digest,
        "license": LICENSE,
        "types": dict(sorted(types.items())),
        "dataTypes": sorted(data_types),
        "members": dict(sorted(members.items())),
        "properties": dict(sorted(properties.items())),
    }


def dump(doc):
    """The file's text: the scalars first, then each map with one entry per line."""
    line = lambda value: json.dumps(value, ensure_ascii=True, separators=(", ", ": "))
    parts = []
    for key, value in doc.items():
        if isinstance(value, dict):
            entries = [f"    {line(name)}: {line(entry)}" for name, entry in value.items()]
            body = ",\n".join(entries)
            parts.append(f"  {line(key)}: {{\n{body}\n  }}" if entries else f"  {line(key)}: {{}}")
        else:
            parts.append(f"  {line(key)}: {line(value)}")
    return "{\n" + ",\n".join(parts) + "\n}\n"


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("release")
    parser.add_argument("out")
    parser.add_argument("--version", required=True)
    parser.add_argument("--url", required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    with open(args.release, "rb") as handle:
        data = handle.read()
    actual = hashlib.sha256(data).hexdigest()
    if actual != args.sha256.lower():
        print(f"schema-org-vocabulary: the release's SHA-256 is {actual}, the pin {args.sha256}", file=sys.stderr)
        return 2
    try:
        release = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        print(f"schema-org-vocabulary: the release is no JSON: {error}", file=sys.stderr)
        return 2
    text = dump(compact(release, args.version, args.url, actual))
    if args.check:
        current = open(args.out, encoding="utf-8").read() if os.path.exists(args.out) else None
        if current != text:
            print(f"schema-org-vocabulary: {args.out} is stale; run scripts/schema-org-vocabulary.sh", file=sys.stderr)
            return 1
        print(f"schema-org-vocabulary: {args.out} is current ({args.version})")
        return 0
    with open(args.out, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)
    print(f"schema-org-vocabulary: wrote {args.out} ({args.version}, {len(text)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
