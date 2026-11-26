#!/usr/bin/env bash
#
# Signs the Windows binaries with Authenticode (RB-090-03): SHA-256, an RFC 3161 timestamp, and
# each signature verified before the file is replaced. Prepared and switched off: phase 1 ships
# unsigned until a certificate exists (RB-110-06). Called by scripts/package-windows.sh --sign
# (RB_SIGN=1) on the staged folder, and by release.yml on the finished ZIP when the repository
# variable RB_CODESIGN is `true`.
#
# Usage:
#   scripts/sign-windows.sh [--backend auto|osslsigncode|signtool|azure] <target>...
#   scripts/sign-windows.sh [--backend ...] --check     # the configuration only, signs nothing
#
# A target is a package ZIP (artifacts/windows/rbrowser-<version>-windows-x64.zip: unpacked,
# signed, packed again in place), a package folder, or single .exe, .dll, .com and .msi files.
# In a package, rbrowser.exe, rbrowser.dll and every other PE image are signed - CEF's ship
# unsigned - except node\ (the Node runtime keeps the OpenJS Foundation's signature) and a file
# already signed by its vendor (scripts/lib/sign_plan.py).
#
# The backend (--backend or RB_SIGN_BACKEND, default auto): osslsigncode in WSL and on Linux;
# signtool.exe (Windows SDK; RB_SIGNTOOL names one) natively on Windows, where auto prefers it;
# azure, prepared only - signtool with the Azure Trusted Signing client library (RB-110-06).
#
# The key, from the environment only - never an argument, never printed:
#   RB_SIGN_PFX                   the certificate and key as a PFX (PKCS#12) file
#   RB_SIGN_PFX_PASSWORD_FILE     a file holding its password (or RB_SIGN_PFX_PASSWORD, which is
#                                 written to a file of mode 600 and dropped from the environment)
#   RB_SIGN_PKCS11_MODULE, RB_SIGN_KEY_ID, RB_SIGN_CERT
#                                 a hardware token instead (osslsigncode); the PIN, if one is
#                                 asked, in RB_SIGN_PFX_PASSWORD_FILE
#   RB_SIGN_AZURE_DLIB, RB_SIGN_AZURE_METADATA   the azure backend's client library and metadata
# and further:
#   RB_SIGN_TIMESTAMP_URL         the RFC 3161 server (default http://timestamp.digicert.com)
#   RB_SIGN_NO_TIMESTAMP=1        no timestamp - a test only, the signature then ends with the
#                                 certificate
#   RB_SIGN_VERIFY_CAFILE         the CA bundle osslsigncode verifies against (default its own)
#   RB_OSSLSIGNCODE, RB_SIGNTOOL  the tool's path, when it is not on PATH
#
# Exit status: 0 signed and verified; 1 not configured, signing or verification failed (the file
# stays as it was); 2 usage. Tests: scripts/tests/sign-windows.sh. Background, backends and
# secrets: docs/security/code-signing.md.
set -euo pipefail
# A trace would print the password's way; this script is never traced.
{ set +x; } 2> /dev/null

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/package.sh
source "$ROOT/scripts/lib/package.sh"
# shellcheck source=lib/sign.sh
source "$ROOT/scripts/lib/sign.sh"

usage="usage: scripts/sign-windows.sh [--backend auto|osslsigncode|signtool|azure] (--check | <zip|folder|file>...)"
backend="${RB_SIGN_BACKEND:-auto}"
check=0
targets=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --backend)
            [[ $# -ge 2 ]] || { echo "$usage" >&2; exit 2; }
            backend="$2"
            shift 2
            ;;
        --check) check=1; shift ;;
        -h | --help) echo "$usage"; exit 0 ;;
        -*) echo "$usage" >&2; exit 2 ;;
        *) targets+=("$1"); shift ;;
    esac
done
case "$backend" in auto | osslsigncode | signtool | azure) ;; *) echo "!! unknown backend '$backend'" >&2; echo "$usage" >&2; exit 2 ;; esac
[[ "$check" == 1 || ${#targets[@]} -gt 0 ]] || { echo "$usage" >&2; exit 2; }

PYTHON="$(command -v python3 || command -v python)" || { echo "!! neither python3 nor python" >&2; exit 1; }
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
rb_sign_configure "$backend" "$TMP" || exit $?
rb_sign_summary
if [[ "$check" == 1 ]]; then
    echo "==> signing is configured"
    exit 0
fi

# Signs what scripts/lib/sign_plan.py lists for the folders and files $@.
sign_planned() {
    local plan action label path
    plan="$TMP/plan"
    "$PYTHON" -I "$ROOT/scripts/lib/sign_plan.py" "$@" > "$plan" || return 1
    while IFS=$'\t' read -r -d '' action label path; do
        if [[ "$action" == keep ]]; then
            echo "==> $label keeps its signature"
        else
            rb_sign_file "$path" "$label" || return 1
        fi
    done < "$plan"
}

# The ZIP $1: unpacked into the scratch folder, signed, packed again and put in its place only
# when every file is signed. The members keep the ZIP's date (the commit's, RB-099-01), so two
# signed builds of one commit still match, and a SHA256SUMS beside the ZIP is written again.
sign_zip() {
    local zip="$1" work folder name epoch sums
    zip="$(cd "$(dirname "$zip")" && pwd)/$(basename "$zip")"
    epoch="$("$PYTHON" -I -c 'import calendar, sys, zipfile
print(calendar.timegm(max(i.date_time for i in zipfile.ZipFile(sys.argv[1]).infolist()) + (0, 0, 0)))' "$zip")" || return 1
    work="$TMP/zip"
    rm -rf "$work"
    mkdir -p "$work"
    folder="$(rb_package_unzip "$zip" "$work")" || return 1
    name="$(basename "$folder")"
    echo "==> $(basename "$zip"): signing $name"
    sign_planned "$folder" || return 1
    SOURCE_DATE_EPOCH="$epoch" rb_package_zip "$work" "$name" "$work/signed.zip" || return 1
    mv -f "$work/signed.zip" "$zip"
    rm -rf "$work"
    sums="$(dirname "$zip")/SHA256SUMS"
    if [[ -f "$sums" ]]; then
        local names=()
        mapfile -t names < <(sed 's/^[0-9a-f]*  //' "$sums")
        rb_package_sums "$(dirname "$zip")" "${names[@]}" || return 1
        echo "==> $(basename "$sums") written again"
    fi
    echo "==> $(basename "$zip") packed again"
}

files=()
for target in "${targets[@]}"; do
    case "$target" in
        *.zip | *.ZIP)
            [[ -f "$target" ]] || { echo "!! $target does not exist" >&2; exit 1; }
            sign_zip "$target" || exit 1
            ;;
        *) files+=("$target") ;;
    esac
done
if ((${#files[@]})); then
    sign_planned "${files[@]}" || exit 1
fi
echo "==> done"
