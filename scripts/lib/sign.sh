# shellcheck shell=bash
#
# Authenticode signing for scripts/sign-windows.sh (RB-090-03): the configuration and one file.
#
#   source "$ROOT/scripts/lib/sign.sh"
#   rb_sign_configure "$backend" "$tmp"   # resolves tool, key and timestamp; fails with the reason
#   rb_sign_summary                       # one line: backend, key kind, timestamp server
#   rb_sign_file "$path" "$label"         # signs, verifies, then replaces the file
#
# The secret never leaves its file: osslsigncode reads the PFX password with -readpass, a password
# given in RB_SIGN_PFX_PASSWORD is written to a file of mode 600 in $tmp and dropped from the
# environment; no message names the key's or the password file's path, and a failing tool's own
# output is shown with both, and the password, masked. Only signtool takes the password as an
# argument (it has no other way): its command line is never printed.
# Tests: scripts/tests/sign-windows.sh.

RB_SIGN_DEFAULT_TIMESTAMP_URL="http://timestamp.digicert.com"

_rb_sign_backend=""   # osslsigncode | signtool | azure
_rb_sign_tool=""      # the program
_rb_sign_key=""       # pfx | pkcs11 | azure
_rb_sign_pass=""      # the password file, or empty
_rb_sign_timestamp="" # the RFC 3161 server, or empty with RB_SIGN_NO_TIMESTAMP=1
_rb_sign_tmp=""

_rb_sign_unconfigured() {
    echo "!! signing not configured: $*" >&2
    echo "   docs/security/code-signing.md names the variables; nothing was signed." >&2
    return 1
}

_rb_sign_windows() {
    case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) return 0 ;; *) return 1 ;; esac
}

# The signtool to use, or a failure: RB_SIGNTOOL, then PATH, then the newest Windows SDK's.
_rb_sign_find_signtool() {
    local found
    if [[ -n "${RB_SIGNTOOL:-}" ]]; then
        [[ -x "$RB_SIGNTOOL" ]] && printf '%s\n' "$RB_SIGNTOOL"
        return
    fi
    found="$(command -v signtool.exe || command -v signtool)" && { printf '%s\n' "$found"; return 0; }
    found="$(compgen -G '/c/Program Files (x86)/Windows Kits/10/bin/*/x64/signtool.exe' | sort -V | tail -n 1)"
    [[ -n "$found" ]] && printf '%s\n' "$found"
}

_rb_sign_find_osslsigncode() {
    if [[ -n "${RB_OSSLSIGNCODE:-}" ]]; then
        [[ -x "$RB_OSSLSIGNCODE" ]] && printf '%s\n' "$RB_OSSLSIGNCODE"
        return
    fi
    command -v osslsigncode
}

# The password file: RB_SIGN_PFX_PASSWORD_FILE, or RB_SIGN_PFX_PASSWORD written to one in the
# scratch folder and dropped from the environment, so no child process inherits it.
_rb_sign_password() {
    local required="$1"
    if [[ -n "${RB_SIGN_PFX_PASSWORD_FILE:-}" ]]; then
        [[ -f "$RB_SIGN_PFX_PASSWORD_FILE" && -r "$RB_SIGN_PFX_PASSWORD_FILE" ]] \
            || { _rb_sign_unconfigured "RB_SIGN_PFX_PASSWORD_FILE names no readable file"; return 1; }
        _rb_sign_pass="$RB_SIGN_PFX_PASSWORD_FILE"
    elif [[ -n "${RB_SIGN_PFX_PASSWORD:-}" ]]; then
        _rb_sign_pass="$_rb_sign_tmp/password"
        (umask 077 && printf '%s' "$RB_SIGN_PFX_PASSWORD" > "$_rb_sign_pass") || return 1
    elif [[ "$required" == 1 ]]; then
        _rb_sign_unconfigured "the PFX needs its password: RB_SIGN_PFX_PASSWORD_FILE or RB_SIGN_PFX_PASSWORD"
        return 1
    fi
    unset RB_SIGN_PFX_PASSWORD
}

# Resolves backend $1 (auto, osslsigncode, signtool, azure) with scratch folder $2.
rb_sign_configure() {
    local backend="$1"
    _rb_sign_tmp="$2"
    if [[ "${RB_SIGN_NO_TIMESTAMP:-0}" == 1 ]]; then
        echo "note: RB_SIGN_NO_TIMESTAMP=1 signs without a timestamp: the signature dies with the certificate. For a test only." >&2
    else
        _rb_sign_timestamp="${RB_SIGN_TIMESTAMP_URL:-$RB_SIGN_DEFAULT_TIMESTAMP_URL}"
        [[ "$_rb_sign_timestamp" =~ ^https?://[^[:space:]]+$ ]] \
            || { _rb_sign_unconfigured "RB_SIGN_TIMESTAMP_URL is no http(s) URL of an RFC 3161 server"; return 1; }
    fi
    if [[ "$backend" == auto ]]; then
        backend=osslsigncode
        if _rb_sign_windows && [[ -z "${RB_SIGN_PKCS11_MODULE:-}" ]] && _rb_sign_find_signtool > /dev/null; then
            backend=signtool
        fi
    fi
    _rb_sign_backend="$backend"
    case "$backend" in
        osslsigncode)
            _rb_sign_tool="$(_rb_sign_find_osslsigncode)" \
                || { _rb_sign_unconfigured "osslsigncode is not installed (apt install osslsigncode; on Windows signtool, RB_SIGN_BACKEND=signtool)"; return 1; }
            ;;
        signtool | azure)
            _rb_sign_tool="$(_rb_sign_find_signtool)" \
                || { _rb_sign_unconfigured "no signtool.exe (the Windows SDK; RB_SIGNTOOL names one)"; return 1; }
            ;;
        *) echo "!! unknown backend '$backend': auto, osslsigncode, signtool or azure" >&2; return 2 ;;
    esac
    if [[ "$backend" == azure ]]; then
        _rb_sign_key=azure
        [[ -f "${RB_SIGN_AZURE_DLIB:-}" && -f "${RB_SIGN_AZURE_METADATA:-}" ]] || {
            _rb_sign_unconfigured "the azure backend is prepared only: RB_SIGN_AZURE_DLIB (Azure.CodeSigning.Dlib.dll) and RB_SIGN_AZURE_METADATA (its metadata.json) - RB-110-06 chooses the backend"
            return 1
        }
    elif [[ -n "${RB_SIGN_PKCS11_MODULE:-}" ]]; then
        _rb_sign_key=pkcs11
        [[ "$backend" == osslsigncode ]] \
            || { _rb_sign_unconfigured "a PKCS#11 token signs with osslsigncode (RB_SIGN_BACKEND=osslsigncode)"; return 1; }
        [[ -f "$RB_SIGN_PKCS11_MODULE" ]] || { _rb_sign_unconfigured "RB_SIGN_PKCS11_MODULE names no file"; return 1; }
        [[ -n "${RB_SIGN_KEY_ID:-}" && -f "${RB_SIGN_CERT:-}" ]] \
            || { _rb_sign_unconfigured "a PKCS#11 token needs RB_SIGN_KEY_ID and RB_SIGN_CERT (the certificate chain, PEM)"; return 1; }
        _rb_sign_password 0
    elif [[ -n "${RB_SIGN_PFX:-}" ]]; then
        _rb_sign_key=pfx
        [[ -f "$RB_SIGN_PFX" && -r "$RB_SIGN_PFX" ]] || { _rb_sign_unconfigured "RB_SIGN_PFX names no readable file"; return 1; }
        _rb_sign_password 1
    else
        _rb_sign_unconfigured "no key: RB_SIGN_PFX and RB_SIGN_PFX_PASSWORD_FILE (or RB_SIGN_PKCS11_MODULE, RB_SIGN_KEY_ID and RB_SIGN_CERT)"
        return 1
    fi
}

rb_sign_summary() {
    echo "==> signing with $_rb_sign_backend, key: $_rb_sign_key, timestamp: ${_rb_sign_timestamp:-none}"
}

# The log $1 on stderr, indented, with the key's and password file's paths and the password masked.
_rb_sign_show() {
    local text secret
    text="$(cat "$1" 2> /dev/null)" || text=""
    for secret in "${RB_SIGN_PFX:-}" "${RB_SIGN_PFX_PASSWORD_FILE:-}" "$_rb_sign_tmp"; do
        [[ -z "$secret" ]] || text="${text//"$secret"/<masked>}"
    done
    if [[ -n "$_rb_sign_pass" && -r "$_rb_sign_pass" ]]; then
        secret="$(< "$_rb_sign_pass")"
        [[ -z "$secret" ]] || text="${text//"$secret"/<masked>}"
    fi
    [[ -z "$text" ]] || printf '     %s\n' "${text//$'\n'/$'\n'     }" >&2
}

# A Windows path for signtool under Git Bash (MSYS_NO_PATHCONV keeps /fd from turning into one).
_rb_sign_path() {
    if command -v cygpath > /dev/null 2>&1; then cygpath -w "$1"; else printf '%s\n' "$1"; fi
}

_rb_sign_osslsigncode() {
    local in="$1" out="$2" args=(sign)
    case "$_rb_sign_key" in
        pfx) args+=(-pkcs12 "$RB_SIGN_PFX" -readpass "$_rb_sign_pass") ;;
        pkcs11)
            args+=(-pkcs11module "$RB_SIGN_PKCS11_MODULE" -certs "$RB_SIGN_CERT" -key "$RB_SIGN_KEY_ID")
            [[ -z "$_rb_sign_pass" ]] || args+=(-readpass "$_rb_sign_pass")
            ;;
    esac
    args+=(-h sha256 -n rBrowser)
    [[ -z "$_rb_sign_timestamp" ]] || args+=(-ts "$_rb_sign_timestamp")
    "$_rb_sign_tool" "${args[@]}" -in "$in" -out "$out"
}

_rb_sign_osslsigncode_verify() {
    local args=(verify -in "$1")
    [[ -z "${RB_SIGN_VERIFY_CAFILE:-}" ]] || args+=(-CAfile "$RB_SIGN_VERIFY_CAFILE" -TSA-CAfile "$RB_SIGN_VERIFY_CAFILE")
    "$_rb_sign_tool" "${args[@]}" || return 1
}

_rb_sign_signtool() {
    local out="$2" args=(sign /fd SHA256 /d rBrowser) password=""
    cp -f "$1" "$out"
    if [[ "$_rb_sign_key" == azure ]]; then
        args+=(/dlib "$(_rb_sign_path "$RB_SIGN_AZURE_DLIB")" /dmdf "$(_rb_sign_path "$RB_SIGN_AZURE_METADATA")")
    else
        password="$(< "$_rb_sign_pass")"
        args+=(/f "$(_rb_sign_path "$RB_SIGN_PFX")" /p "$password")
    fi
    [[ -z "$_rb_sign_timestamp" ]] || args+=(/tr "$_rb_sign_timestamp" /td SHA256)
    MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' "$_rb_sign_tool" "${args[@]}" "$(_rb_sign_path "$out")"
}

_rb_sign_signtool_verify() {
    MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' "$_rb_sign_tool" verify /pa /v "$(_rb_sign_path "$1")"
}

# Signs the file $1 (named $2 in messages) into a copy, verifies the copy - with its timestamp
# when one was asked for - and only then puts it in place: a failure leaves the file as it was.
rb_sign_file() {
    local file="$1" label="$2" suffix out log stamp stamped=""
    suffix="${file##*.}"
    out="$_rb_sign_tmp/signed.$suffix"
    log="$_rb_sign_tmp/tool.log"
    rm -f "$out"
    if [[ "$_rb_sign_backend" == osslsigncode ]]; then
        _rb_sign_osslsigncode "$file" "$out" > "$log" 2>&1 || {
            echo "!! signing $label failed ($_rb_sign_backend):" >&2
            _rb_sign_show "$log"
            return 1
        }
        _rb_sign_osslsigncode_verify "$out" > "$log" 2>&1 || {
            echo "!! verifying the signature of $label failed:" >&2
            _rb_sign_show "$log"
            return 1
        }
        stamp="Timestamp Server Signature verification: ok"
    else
        _rb_sign_signtool "$file" "$out" > "$log" 2>&1 || {
            echo "!! signing $label failed ($_rb_sign_backend):" >&2
            _rb_sign_show "$log"
            return 1
        }
        _rb_sign_signtool_verify "$out" > "$log" 2>&1 || {
            echo "!! verifying the signature of $label failed:" >&2
            _rb_sign_show "$log"
            return 1
        }
        stamp="The signature is timestamped"
    fi
    if [[ -n "$_rb_sign_timestamp" ]]; then
        grep -qF "$stamp" "$log" || {
            echo "!! the signature of $label carries no valid timestamp:" >&2
            _rb_sign_show "$log"
            return 1
        }
        stamped=", timestamped"
    fi
    cp -f "$out" "$file"
    rm -f "$out"
    echo "==> signed $label (verified$stamped)"
}
