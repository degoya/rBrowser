#!/usr/bin/env bash
#
# scripts/sign-windows.sh (RB-090-03) in two halves.
#
# Always, with a fake osslsigncode and a fake signtool that record their arguments: the usage, a
# missing key, password, tool or timestamp server stops the run with a clear message; the password
# reaches osslsigncode only as a file (-readpass), never as an argument; neither the password nor
# the key's path is ever printed - a failing tool's own message included, and under `bash -x`;
# SHA-256 and the RFC 3161 timestamp server (configurable, or none with RB_SIGN_NO_TIMESTAMP=1);
# a failed signature or verification leaves the file as it was; in a package folder or ZIP only
# our files and CEF's unsigned ones are signed, node\ and files already signed keep theirs; the
# signtool, PKCS#11 and Azure backends get their arguments.
#
# Then, when osslsigncode is installed, for real: a throwaway CA, code-signing and timestamping
# certificate made with openssl in a temporary folder, a local RFC 3161 server
# (tests/lib/sign_fixtures.py), and an .exe, an MSI and a package folder signed and verified with
# their timestamp. Without osslsigncode that half is skipped, named. No key is ever committed.
#
#   scripts/tests/sign-windows.sh
# shellcheck disable=SC2016  # the conditions are single-quoted on purpose: expect_true evaluates them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
TSA_PID=""
cleanup() {
    [[ -z "$TSA_PID" ]] || kill "$TSA_PID" 2> /dev/null || true
    rm -rf "$SCRATCH"
}
trap cleanup EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

SIGN="$ROOT/scripts/sign-windows.sh"
FIXTURES="$ROOT/scripts/tests/lib/sign_fixtures.py"
FAKE_BIN="$SCRATCH/bin"
CALLS="$SCRATCH/calls"
SEEN_PASS="$SCRATCH/seen-pass"
KEYS="$SCRATCH/keys"
PFX="$KEYS/release-signing.pfx"
PASSWORD='s3cret pass $word'
PASS_FILE="$KEYS/release-signing.pass"
mkdir -p "$FAKE_BIN" "$KEYS"
printf 'not a real pfx\n' > "$PFX"
printf '%s' "$PASSWORD" > "$PASS_FILE"

# osslsigncode <command> ... -in <file> -out <file>: records the call (one argument per line, a
# blank line after it) and the password file it was given; `sign` copies the input and appends
# SIGNED, or with RB_FAKE_SIGN=fail fails as osslsigncode does on a wrong password - naming the
# key's path and, worse than the real one, the password; `verify` accepts a SIGNED file unless
# RB_FAKE_VERIFY=fail.
cat > "$FAKE_BIN/osslsigncode" << 'SH'
#!/usr/bin/env bash
printf '%s\n' "$@" "" >> "$RB_FAKE_CALLS"
command="$1"
shift
in="" out="" readpass=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        -in) in="$2"; shift 2 ;;
        -out) out="$2"; shift 2 ;;
        -readpass) readpass="$2"; shift 2 ;;
        *) shift ;;
    esac
done
case "$command" in
    sign)
        [[ -z "$readpass" ]] || { cp "$readpass" "$RB_FAKE_SEEN_PASS"; echo "$readpass" >> "$RB_FAKE_SEEN_PASS.path"; }
        if [[ "${RB_FAKE_SIGN:-ok}" == fail ]]; then
            echo "Failed to parse PKCS#12 file: ${RB_SIGN_PFX:-} (Wrong password?)"
            [[ -z "$readpass" ]] || echo "tried: $(cat "$readpass")"
            exit 1
        fi
        { cat "$in"; printf 'SIGNED'; } > "$out"
        ;;
    verify)
        if [[ "${RB_FAKE_VERIFY:-ok}" != ok ]] || ! grep -q SIGNED "$in"; then
            echo "Signature verification: failed"
            exit 1
        fi
        printf '%s\n' "Timestamp Server Signature verification: ok" "Signature verification: ok"
        ;;
esac
SH
# signtool sign|verify ... <file>: records the call; `sign` appends SIGNED to the last argument,
# `verify` accepts a SIGNED file and says it is timestamped.
cat > "$FAKE_BIN/signtool" << 'SH'
#!/usr/bin/env bash
printf '%s\n' "$@" "" >> "$RB_FAKE_CALLS"
file="${!#}"
case "$1" in
    sign) printf 'SIGNED' >> "$file" ;;
    verify)
        grep -q SIGNED "$file" || { echo "SignTool Error: No signature found."; exit 1; }
        echo "The signature is timestamped: today"
        ;;
esac
SH
chmod +x "$FAKE_BIN/osslsigncode" "$FAKE_BIN/signtool"

# Runs the signer with the fakes first on PATH and a configured PFX; arguments before `--` are
# extra environment (NAME=value, or -u NAME to drop one), the rest go to the script.
signer() {
    local envs=()
    while [[ $# -gt 0 && "$1" != -- ]]; do
        # An empty value is an unset one to the script; `env -u` cannot follow the defaults.
        if [[ "$1" == -u ]]; then envs+=("$2="); shift 2; else envs+=("$1"); shift; fi
    done
    [[ $# -eq 0 ]] || shift
    rm -f "$CALLS" "$SEEN_PASS" "$SEEN_PASS.path"
    run_status env PATH="$FAKE_BIN:$PATH" RB_FAKE_CALLS="$CALLS" RB_FAKE_SEEN_PASS="$SEEN_PASS" \
        RB_SIGN_BACKEND=osslsigncode RB_SIGN_PFX="$PFX" RB_SIGN_PFX_PASSWORD_FILE="$PASS_FILE" \
        RB_SIGN_TIMESTAMP_URL=http://tsa.example.invalid/rfc3161 "${envs[@]}" "$SIGN" "$@"
}
calls() { cat "$CALLS" 2> /dev/null || true; }
# Whether the last run printed neither the password nor the key's path.
expect_no_secret() {
    expect_true "$1: the password is never printed" '! grep -qF "s3cret" <<< "$output"'
    expect_true "$1: nor the key's path" '! grep -qF "release-signing" <<< "$output"'
}
pe() { python3 -I "$FIXTURES" pe "$@"; }

# A package folder as package-windows.sh stages it: ours, CEF's (one of them already signed, as
# Microsoft's DirectX DLLs are), the Node runtime and files that are no program.
make_package() {
    local dir="$1"
    mkdir -p "$dir/node" "$dir/locales" "$dir/plugins"
    for file in rbrowser.exe rbrowser.dll libcef.dll chrome_elf.dll; do pe "$dir/$file"; done
    pe "$dir/dxcompiler.dll" --signed
    pe "$dir/node/node.exe"
    printf 'pak\n' > "$dir/locales/en-US.pak"
    printf 'readme\n' > "$dir/README.txt"
    printf 'plugin\n' > "$dir/plugins/example.rbplugin"
}

# --- The usage and the configuration.
signer --
expect_status "without a file the usage stops the run" 2
expect_output "and is printed" "usage: scripts/sign-windows.sh"
signer -- --frobnicate app.exe
expect_status "an unknown option is refused" 2

pe "$SCRATCH/app.exe"
signer -u RB_SIGN_PFX -- "$SCRATCH/app.exe"
expect_status "no key: the run stops" 1
expect_output "with a clear message" "signing not configured"
expect_output "naming what to set" "RB_SIGN_PFX"
expect_true "and nothing is signed" '[[ -z "$(calls)" ]]'

signer RB_SIGN_PFX="$KEYS/missing-release-signing.pfx" -- "$SCRATCH/app.exe"
expect_status "a key that is no readable file stops the run" 1
expect_output "naming the variable" "RB_SIGN_PFX names no readable file"
expect_no_secret "a missing key"

signer -u RB_SIGN_PFX_PASSWORD_FILE -- "$SCRATCH/app.exe"
expect_status "no password stops the run" 1
expect_output "naming both ways to give it" "RB_SIGN_PFX_PASSWORD_FILE or RB_SIGN_PFX_PASSWORD"
signer RB_SIGN_PFX_PASSWORD_FILE="$KEYS/none" -- "$SCRATCH/app.exe"
expect_status "a password file that is not there stops the run" 1
expect_no_secret "a missing password file"

signer RB_OSSLSIGNCODE="$SCRATCH/nowhere/osslsigncode" -- "$SCRATCH/app.exe"
expect_status "no osslsigncode stops the run" 1
expect_output "and says what to install" "osslsigncode is not installed"

signer RB_SIGN_TIMESTAMP_URL="ftp://tsa.example.invalid" -- "$SCRATCH/app.exe"
expect_status "a timestamp server that is no http(s) URL is refused" 1
expect_output "naming the variable" "RB_SIGN_TIMESTAMP_URL"

signer -- --check
expect_status "--check with everything configured passes" 0
expect_output "naming the backend" "osslsigncode"
expect_true "and signs nothing" '[[ -z "$(calls)" ]]'
signer -u RB_SIGN_PFX -- --check
expect_status "--check without a key fails" 1
expect_output "as signing would" "signing not configured"

printf 'text\n' > "$SCRATCH/notes.txt"
signer -- "$SCRATCH/notes.txt"
expect_status "a file that is no program, MSI, folder or ZIP is refused" 1
expect_output "and named" "notes.txt"
printf 'not a program\n' > "$SCRATCH/broken.dll"
signer -- "$SCRATCH/broken.dll"
expect_status "a .dll that is no PE image is refused" 1
expect_output "and named" "broken.dll is no PE image"

# --- One file: the arguments, the secret's way and the result.
signer -- "$SCRATCH/app.exe"
expect_status "an .exe is signed" 0
expect_output "verified and timestamped" "signed app.exe (verified, timestamped)"
expect_true "with SHA-256" 'grep -qx -- "-h" "$CALLS" && grep -qx sha256 "$CALLS"'
expect_true "with the PFX" 'grep -qx -- "-pkcs12" "$CALLS"'
expect_true "the password as a file" 'grep -qx -- "-readpass" "$CALLS" && [[ "$(cat "$SEEN_PASS")" == "$PASSWORD" ]]'
expect_true "never as an argument" '! grep -qF "s3cret" "$CALLS" && ! grep -qx -- "-pass" "$CALLS"'
expect_true "the RFC 3161 server" 'grep -qx -- "-ts" "$CALLS" && grep -qx http://tsa.example.invalid/rfc3161 "$CALLS"'
expect_true "then verified" 'grep -qx verify "$CALLS"'
expect_true "the file is signed in place" 'grep -q SIGNED "$SCRATCH/app.exe"'
expect_no_secret "a signed file"

pe "$SCRATCH/app.exe"
signer -u RB_SIGN_TIMESTAMP_URL -- "$SCRATCH/app.exe"
expect_status "without RB_SIGN_TIMESTAMP_URL the default server is taken" 0
expect_true "DigiCert's" 'grep -qx http://timestamp.digicert.com "$CALLS"'

pe "$SCRATCH/app.exe"
signer RB_SIGN_NO_TIMESTAMP=1 -- "$SCRATCH/app.exe"
expect_status "RB_SIGN_NO_TIMESTAMP=1 signs without a timestamp" 0
expect_output "and warns" "without a timestamp"
expect_true "no server is asked" '! grep -qx -- "-ts" "$CALLS"'

pe "$SCRATCH/app.exe"
signer -u RB_SIGN_PFX_PASSWORD_FILE RB_SIGN_PFX_PASSWORD="$PASSWORD" -- "$SCRATCH/app.exe"
expect_status "the password from RB_SIGN_PFX_PASSWORD" 0
expect_true "reaches the tool as a file" '[[ "$(cat "$SEEN_PASS")" == "$PASSWORD" ]]'
expect_true "never as an argument" '! grep -qF "s3cret" "$CALLS"'
expect_true "and the file is gone afterwards" '[[ ! -e "$(cat "$SEEN_PASS.path")" ]]'
expect_no_secret "a password from the environment"

pe "$SCRATCH/app.exe"
rm -f "$CALLS"
run_status env PATH="$FAKE_BIN:$PATH" RB_FAKE_CALLS="$CALLS" RB_FAKE_SEEN_PASS="$SEEN_PASS" \
    RB_SIGN_BACKEND=osslsigncode RB_SIGN_PFX="$PFX" RB_SIGN_PFX_PASSWORD="$PASSWORD" \
    bash -x "$SIGN" "$SCRATCH/app.exe"
expect_status "under bash -x the file is signed" 0
expect_no_secret "under bash -x"

pe "$SCRATCH/app.exe"
before="$(sha256sum < "$SCRATCH/app.exe")"
signer RB_FAKE_SIGN=fail -- "$SCRATCH/app.exe"
expect_status "a wrong password stops the run" 1
expect_output "with a clear message" "signing app.exe failed"
expect_output "and the tool's own words, masked" "Wrong password?"
expect_no_secret "a wrong password"
expect "the file is unchanged" "$before" "$(sha256sum < "$SCRATCH/app.exe")"

signer RB_FAKE_VERIFY=fail -- "$SCRATCH/app.exe"
expect_status "a failed verification stops the run" 1
expect_output "with a clear message" "verifying the signature of app.exe failed"
expect "the file is unchanged" "$before" "$(sha256sum < "$SCRATCH/app.exe")"

# --- A package folder and a package ZIP.
PKG="$SCRATCH/pkg/rbrowser-1.2.3-windows-x64"
make_package "$PKG"
signer -- "$PKG"
expect_status "a package folder is signed" 0
for file in rbrowser.exe rbrowser.dll libcef.dll chrome_elf.dll; do
    expect_true "$file is signed" "grep -q SIGNED '$PKG/$file'"
done
expect_output "a file already signed keeps its signature" "dxcompiler.dll keeps its signature"
expect_true "and is untouched" '! grep -q SIGNED "$PKG/dxcompiler.dll"'
expect_true "the Node runtime keeps its own" '! grep -q SIGNED "$PKG/node/node.exe" && ! grep -qF "node.exe" "$CALLS"'
expect_true "files that are no program are left alone" '! grep -qF -e en-US.pak -e README.txt -e example.rbplugin "$CALLS"'

mkdir -p "$SCRATCH/notapkg"
pe "$SCRATCH/notapkg/other.dll"
signer -- "$SCRATCH/notapkg"
expect_status "a folder without rbrowser.exe and rbrowser.dll is refused" 1
expect_output "and named" "is no package folder"

rm -rf "$SCRATCH/pkg"
make_package "$PKG"
(cd "$SCRATCH/pkg" && zip -qry "$SCRATCH/pkg/rbrowser-1.2.3-windows-x64.zip" rbrowser-1.2.3-windows-x64)
rm -rf "$PKG"
signer -- "$SCRATCH/pkg/rbrowser-1.2.3-windows-x64.zip"
expect_status "a package ZIP is signed and packed again" 0
zip_member() { python3 -I -c 'import sys, zipfile; sys.stdout.buffer.write(zipfile.ZipFile(sys.argv[1]).read(sys.argv[2]))' "$SCRATCH/pkg/rbrowser-1.2.3-windows-x64.zip" "rbrowser-1.2.3-windows-x64/$1"; }
expect_true "rbrowser.exe in it is signed" 'zip_member rbrowser.exe | grep -q SIGNED'
expect_true "libcef.dll too" 'zip_member libcef.dll | grep -q SIGNED'
expect_true "node.exe is not" '! zip_member node/node.exe | grep -q SIGNED'
expect "the other files are still there" "pak" "$(zip_member locales/en-US.pak)"
expect_true "nothing but the ZIP is left beside it" '[[ "$(ls "$SCRATCH/pkg")" == rbrowser-1.2.3-windows-x64.zip ]]'

# --- The other backends' arguments.
pe "$SCRATCH/app.exe"
signer RB_SIGN_BACKEND=signtool RB_SIGNTOOL="$FAKE_BIN/signtool" -- "$SCRATCH/app.exe"
expect_status "signtool signs" 0
expect_true "with SHA-256 for file and timestamp" 'grep -qx /fd "$CALLS" && grep -qx /td "$CALLS" && grep -qx SHA256 "$CALLS"'
expect_true "the RFC 3161 server" 'grep -qx /tr "$CALLS" && grep -qx http://tsa.example.invalid/rfc3161 "$CALLS"'
expect_true "then verifies by the Authenticode policy" 'grep -qx verify "$CALLS" && grep -qx /pa "$CALLS"'
expect_no_secret "signtool"

pe "$SCRATCH/app.exe"
printf 'cert\n' > "$KEYS/token-cert.pem"
signer -u RB_SIGN_PFX RB_SIGN_PKCS11_MODULE="$FAKE_BIN/signtool" RB_SIGN_KEY_ID=01 \
    RB_SIGN_CERT="$KEYS/token-cert.pem" -- "$SCRATCH/app.exe"
expect_status "a PKCS#11 token signs" 0
expect_true "through its module and key id" 'grep -qx -- "-pkcs11module" "$CALLS" && grep -qx -- "-key" "$CALLS" && grep -qx 01 "$CALLS"'
expect_true "with the certificate" 'grep -qx -- "-certs" "$CALLS" && ! grep -qx -- "-pkcs12" "$CALLS"'
signer -u RB_SIGN_PFX RB_SIGN_PKCS11_MODULE="$FAKE_BIN/signtool" -- "$SCRATCH/app.exe"
expect_status "a token without key id and certificate is refused" 1
expect_output "naming both" "RB_SIGN_KEY_ID and RB_SIGN_CERT"

signer RB_SIGN_BACKEND=azure RB_SIGNTOOL="$FAKE_BIN/signtool" -- "$SCRATCH/app.exe"
expect_status "the Azure backend without its client library is refused" 1
expect_output "as prepared only" "RB-110-06"
signer -- --backend nonsense "$SCRATCH/app.exe"
expect_status "an unknown backend is refused" 2

# --- For real, when osslsigncode is installed.
if ! command -v osslsigncode > /dev/null 2>&1 || ! command -v openssl > /dev/null 2>&1; then
    echo "skip the signing with a test certificate: osslsigncode or openssl is not installed"
    finish_tests sign-windows
    exit 0
fi
CERTS="$SCRATCH/certs"
mkdir -p "$CERTS"
(
    cd "$CERTS"
    openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem -days 2 \
        -subj "/CN=rBrowser test CA" -addext basicConstraints=critical,CA:TRUE \
        -addext keyUsage=critical,keyCertSign,cRLSign
    for who in signer tsa; do
        openssl req -newkey rsa:2048 -nodes -keyout "$who.key" -out "$who.csr" -subj "/CN=rBrowser test $who"
    done
    printf '%s\n' basicConstraints=CA:FALSE keyUsage=critical,digitalSignature extendedKeyUsage=codeSigning > signer.ext
    printf '%s\n' basicConstraints=CA:FALSE keyUsage=critical,digitalSignature \
        extendedKeyUsage=critical,timeStamping > tsa.ext
    for who in signer tsa; do
        openssl x509 -req -in "$who.csr" -CA ca.pem -CAkey ca.key -CAcreateserial -out "$who.pem" \
            -days 2 -extfile "$who.ext"
    done
    openssl pkcs12 -export -out signer.pfx -inkey signer.key -in signer.pem -certfile ca.pem \
        -passout "file:$PASS_FILE"
    printf '%s\n' '[ tsa ]' 'default_tsa = tsa_config' '[ tsa_config ]' "serial = $CERTS/tsaserial" \
        'crypto_device = builtin' 'signer_digest = sha256' 'default_policy = 1.2.3.4.1' \
        'digests = sha256, sha384, sha512' 'accuracy = secs:1' 'ess_cert_id_alg = sha256' > ts.cnf
    echo 01 > tsaserial
) > "$SCRATCH/openssl.log" 2>&1 || { cat "$SCRATCH/openssl.log"; fail "the test certificates are made"; finish_tests sign-windows; }
python3 -I "$FIXTURES" tsa "$CERTS" "$CERTS/port" &
TSA_PID=$!
for _ in $(seq 100); do [[ -s "$CERTS/port" ]] && break; sleep 0.1; done
TSA_URL="http://127.0.0.1:$(cat "$CERTS/port")/"

real() {
    run_status env -u RB_SIGN_BACKEND RB_SIGN_PFX="$CERTS/signer.pfx" RB_SIGN_PFX_PASSWORD_FILE="$PASS_FILE" \
        RB_SIGN_TIMESTAMP_URL="$TSA_URL" RB_SIGN_VERIFY_CAFILE="$CERTS/ca.pem" "$@"
}
mkdir -p "$SCRATCH/real"
pe "$SCRATCH/real/app.exe"
python3 -I "$FIXTURES" msi "$SCRATCH/real/app.msi"
real "$SIGN" "$SCRATCH/real/app.exe" "$SCRATCH/real/app.msi"
expect_status "the test certificate signs an .exe and an MSI" 0
expect_output "the .exe verified and timestamped" "signed app.exe (verified, timestamped)"
expect_output "the MSI too" "signed app.msi (verified, timestamped)"
expect_no_secret "a real signature"
for file in app.exe app.msi; do
    run_status osslsigncode verify -in "$SCRATCH/real/$file" -CAfile "$CERTS/ca.pem" -TSA-CAfile "$CERTS/ca.pem"
    expect_status "osslsigncode verify accepts $file" 0
    expect_output "with a SHA-256 digest" "Message digest algorithm: SHA256"
    expect_output "and a valid timestamp" "Timestamp Server Signature verification: ok"
done

pe "$SCRATCH/real/wrong.exe"
before="$(sha256sum < "$SCRATCH/real/wrong.exe")"
printf 'wrong %s' "$PASSWORD" > "$KEYS/wrong.pass"
real env RB_SIGN_PFX_PASSWORD_FILE="$KEYS/wrong.pass" "$SIGN" "$SCRATCH/real/wrong.exe"
expect_status "a wrong password stops the real run" 1
expect_output "with a clear message" "signing wrong.exe failed"
expect_true "the password is never printed" '! grep -qF "s3cret" <<< "$output"'
expect_true "nor the key's path" '! grep -qF "signer.pfx" <<< "$output"'
expect "the file is unchanged" "$before" "$(sha256sum < "$SCRATCH/real/wrong.exe")"

make_package "$SCRATCH/real/pkg"
real "$SIGN" "$SCRATCH/real/pkg"
expect_status "a package folder is signed for real" 0
run_status osslsigncode verify -in "$SCRATCH/real/pkg/libcef.dll" -CAfile "$CERTS/ca.pem" -TSA-CAfile "$CERTS/ca.pem"
expect_status "libcef.dll carries a valid signature" 0
run_status osslsigncode verify -in "$SCRATCH/real/pkg/node/node.exe" -CAfile "$CERTS/ca.pem"
expect_status "node.exe carries none of ours" 1

finish_tests sign-windows
