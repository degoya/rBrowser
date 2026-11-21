#!/usr/bin/env bash
#
# scripts/schema-org-vocabulary.sh (scripts/lib/schema_org_vocabulary.py) against a small
# schema.org release fixture: the compact vocabulary keeps the schema.org classes with their
# supertypes, the data types, the enumeration members and the properties with their domains,
# ranges and successor, and nothing of another vocabulary; a file whose SHA-256 is not the pinned
# one is refused; --check passes on a fresh file and finds a stale one. The checked-in
# vocabulary names the pinned release.
#
# Python only, compiles nothing, no network: check.sh --preflight runs it.
#
#   scripts/tests/schema-org-vocabulary.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
COMPACT="$ROOT/scripts/lib/schema_org_vocabulary.py"
WRAPPER="$ROOT/scripts/schema-org-vocabulary.sh"

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

release="$SCRATCH/release.jsonld"
cat > "$release" << 'EOF'
{
  "@context": {"schema": "https://schema.org/", "rdfs": "http://www.w3.org/2000/01/rdf-schema#"},
  "@graph": [
    {"@id": "schema:Thing", "@type": "rdfs:Class", "rdfs:label": "Thing"},
    {"@id": "schema:CreativeWork", "@type": "rdfs:Class", "rdfs:subClassOf": {"@id": "schema:Thing"}},
    {"@id": "schema:Article", "@type": "rdfs:Class", "rdfs:subClassOf": [{"@id": "schema:CreativeWork"}, {"@id": "other:Doc"}]},
    {"@id": "schema:Text", "@type": ["rdfs:Class", "schema:DataType"]},
    {"@id": "schema:URL", "@type": "rdfs:Class", "rdfs:subClassOf": {"@id": "schema:Text"}},
    {"@id": "schema:Enumeration", "@type": "rdfs:Class", "rdfs:subClassOf": {"@id": "schema:Thing"}},
    {"@id": "schema:ItemAvailability", "@type": "rdfs:Class", "rdfs:subClassOf": {"@id": "schema:Enumeration"}},
    {"@id": "schema:InStock", "@type": "schema:ItemAvailability", "rdfs:label": "InStock"},
    {"@id": "schema:headline", "@type": "rdf:Property", "schema:domainIncludes": {"@id": "schema:CreativeWork"}, "schema:rangeIncludes": {"@id": "schema:Text"}},
    {"@id": "schema:url", "@type": "rdf:Property", "schema:domainIncludes": {"@id": "schema:Thing"}, "schema:rangeIncludes": [{"@id": "schema:URL"}]},
    {"@id": "schema:oldName", "@type": "rdf:Property", "schema:domainIncludes": {"@id": "schema:Thing"}, "schema:rangeIncludes": {"@id": "schema:Text"}, "schema:supersededBy": {"@id": "schema:headline"}},
    {"@id": "other:Doc", "@type": "rdfs:Class"},
    {"@id": "other:prop", "@type": "rdf:Property"}
  ]
}
EOF
sum="$(sha256sum "$release" | cut -d ' ' -f 1)"
out="$SCRATCH/vocabulary.json"
pins=(--version 1.0 --url https://example.test/release.jsonld --sha256 "$sum")

run_status python3 -I "$COMPACT" "$release" "$out" "${pins[@]}"
expect_status "the fixture is compacted" 0
expect_true "the file exists" '[[ -f "$out" ]]'

# The condition is one of this test's own literals below, never read from a file.
check_json() {
    python3 -I - "$out" "$1" << 'EOF'
import json, sys
doc = json.load(open(sys.argv[1], encoding="utf-8"))
sys.exit(0 if eval(sys.argv[2], {"doc": doc}) else 1)
EOF
}
run_status check_json 'doc["version"] == "1.0" and doc["sha256"] and doc["source"].endswith("release.jsonld")'
expect_status "the release, its source and hash are named" 0
run_status check_json 'doc["types"]["Article"] == ["CreativeWork"] and doc["types"]["Thing"] == []'
expect_status "a class keeps its schema.org supertypes only" 0
run_status check_json '"Doc" not in doc["types"] and "prop" not in doc["properties"]'
expect_status "another vocabulary is left out" 0
run_status check_json 'doc["dataTypes"] == ["Text"] and doc["types"]["URL"] == ["Text"]'
expect_status "the data types are named" 0
run_status check_json 'doc["members"] == {"InStock": ["ItemAvailability"]}'
expect_status "an enumeration member names its enumeration" 0
run_status check_json 'doc["properties"]["headline"] == {"d": ["CreativeWork"], "r": ["Text"]}'
expect_status "a property keeps its domains and ranges" 0
run_status check_json 'doc["properties"]["oldName"]["s"] == "headline"'
expect_status "a superseded property names its successor" 0
run_status check_json '"CC BY-SA 3.0" in doc["license"]'
expect_status "the licence is named" 0

run_status python3 -I "$COMPACT" "$release" "$out" "${pins[@]}" --check
expect_status "--check passes on a fresh file" 0

printf '\n' >> "$out"
run_status python3 -I "$COMPACT" "$release" "$out" "${pins[@]}" --check
expect_status "--check finds a stale file" 1
expect_output "the stale file is named" "stale"

run_status python3 -I "$COMPACT" "$release" "$SCRATCH/other.json" --version 1.0 \
    --url https://example.test/release.jsonld --sha256 "$(printf '0%.0s' {1..64})"
expect_status "a release with another hash is refused" 2
expect_output "the hash is named" "SHA-256"
expect_true "nothing is written for it" '[[ ! -e "$SCRATCH/other.json" ]]'

run_status "$WRAPPER" --check --from "$release"
expect_status "the wrapper refuses a release that is not the pinned one" 2

run_status python3 -I - "$ROOT/plugins/schema-org/data/vocabulary.json" "$WRAPPER" << 'EOF'
import json, re, sys
doc = json.load(open(sys.argv[1], encoding="utf-8"))
script = open(sys.argv[2], encoding="utf-8").read()
version = re.search(r'^VERSION="([^"]+)"', script, re.M).group(1)
digest = re.search(r'^SHA256="([0-9a-f]{64})"', script, re.M).group(1)
assert doc["version"] == version and doc["sha256"] == digest, (doc["version"], version)
assert len(doc["types"]) > 500 and len(doc["properties"]) > 1000
EOF
expect_status "the checked-in vocabulary is the pinned release's" 0

finish_tests schema-org-vocabulary
