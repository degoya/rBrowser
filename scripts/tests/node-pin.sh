#!/usr/bin/env bash
#
# rb_use_pinned_node (scripts/lib/tool-versions.sh), which scripts/package-windows.sh and the npm
# stages of scripts/check.sh call before they run npm: a node on PATH at the .nvmrc version is
# kept; another one is replaced by the pinned version from fnm (`fnm env`, `fnm use`) or from
# nvm's install folder; without either the call fails and names .nvmrc and the install command.
# Fakes for node and fnm on a PATH of their own; the real ones are never run.
#
#   scripts/tests/node-pin.sh
# shellcheck disable=SC2016  # single-quoted on purpose: a child bash and expect_true evaluate them
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

# shellcheck source=lib/expect.sh
source "$ROOT/scripts/tests/lib/expect.sh"

PIN="26.13.0"
CHECKOUT="$SCRATCH/checkout"
mkdir -p "$CHECKOUT"
echo "v$PIN" > "$CHECKOUT/.nvmrc"

# A fake node of version $2 in folder $1.
fake_node() {
    mkdir -p "$1"
    printf '#!/usr/bin/env bash\necho v%s\n' "$2" > "$1/node"
    chmod +x "$1/node"
}

# A fake fnm in folder $1 that knows the versions under $SCRATCH/fnm/versions: `fnm env` puts its
# multishell folder first on PATH, `fnm use <v>` links that version's node there. Each call is
# logged.
fake_fnm() {
    mkdir -p "$1" "$SCRATCH/fnm/versions"
    cat > "$1/fnm" << SH
#!/usr/bin/env bash
echo "\$*" >> "$SCRATCH/fnm/calls"
case "\$1" in
    env) echo 'export PATH="$SCRATCH/fnm/shell/bin":"\$PATH"' ;;
    use)
        version="\${2#v}"
        if [[ ! -x "$SCRATCH/fnm/versions/\$version/node" ]]; then
            echo "error: Requested version v\$version is not currently installed" >&2
            exit 1
        fi
        mkdir -p "$SCRATCH/fnm/shell/bin"
        ln -sf "$SCRATCH/fnm/versions/\$version/node" "$SCRATCH/fnm/shell/bin/node"
        echo "Using Node v\$version"
        ;;
    *) exit 2 ;;
esac
SH
    chmod +x "$1/fnm"
}

# Runs rb_use_pinned_node on the checkout in a clean shell whose PATH is $1 then /usr/bin:/bin,
# and prints the node it leaves first on PATH.
in_pin() {
    run_status env -i HOME="$SCRATCH/home" NVM_DIR="$SCRATCH/nvm" PATH="$1:/usr/bin:/bin" \
        "$BASH" -c 'set -euo pipefail
            source "$1/scripts/lib/tool-versions.sh"
            rb_use_pinned_node "$2"
            echo "node now $(node --version)"' _ "$ROOT" "$CHECKOUT"
}

reset() {
    rm -rf "${SCRATCH:?}/bin" "${SCRATCH:?}/fnm" "${SCRATCH:?}/nvm"
}

# --- the pinned node is already first: kept, fnm never asked.
reset
fake_node "$SCRATCH/bin" "$PIN"
fake_fnm "$SCRATCH/bin"
in_pin "$SCRATCH/bin"
expect_status "the pinned node on PATH is kept" 0
expect_output "and runs" "node now v$PIN"
expect_true "fnm is not asked" '[[ ! -e "$SCRATCH/fnm/calls" ]]'

# --- another node first, fnm has the pin: fnm's is activated.
reset
fake_node "$SCRATCH/bin" 24.0.0
fake_fnm "$SCRATCH/bin"
fake_node "$SCRATCH/fnm/versions/$PIN" "$PIN"
in_pin "$SCRATCH/bin"
expect_status "another node with fnm holding the pin is replaced" 0
expect_output "by the pinned one" "node now v$PIN"
expect_output "and the switch is named" "node $PIN"
expect_true "through fnm use" 'grep -qxs "use $PIN" "$SCRATCH/fnm/calls"'

# --- fnm without the pin: stops with the install command.
reset
fake_node "$SCRATCH/bin" 24.0.0
fake_fnm "$SCRATCH/bin"
in_pin "$SCRATCH/bin"
expect_status "fnm without the pinned version stops" 1
expect_output "naming .nvmrc" ".nvmrc"
expect_output "and the install command" "fnm install $PIN"
expect_output "and the node found" "24.0.0"
expect_true "and runs nothing after it" '! grep -q "node now" <<< "$output"'

# --- no fnm, nvm has the pin: nvm's install folder goes first on PATH.
reset
fake_node "$SCRATCH/bin" 24.0.0
fake_node "$SCRATCH/nvm/versions/node/v$PIN/bin" "$PIN"
in_pin "$SCRATCH/bin"
expect_status "another node with nvm holding the pin is replaced" 0
expect_output "by nvm's" "node now v$PIN"

# --- another node, no version manager: stops.
reset
fake_node "$SCRATCH/bin" 24.0.0
in_pin "$SCRATCH/bin"
expect_status "another node without fnm or nvm stops" 1
expect_output "naming .nvmrc" ".nvmrc"
expect_output "and the version found" "!! node 24.0.0 is first on PATH"

# --- no node at all (only where the system has none either).
if ! PATH=/usr/bin:/bin command -v node > /dev/null 2>&1; then
    reset
    mkdir -p "$SCRATCH/bin"
    in_pin "$SCRATCH/bin"
    expect_status "no node at all stops" 1
    expect_output "and says so" "!! no node is first on PATH"
fi

# --- an empty .nvmrc stops.
reset
fake_node "$SCRATCH/bin" "$PIN"
: > "$CHECKOUT/.nvmrc"
in_pin "$SCRATCH/bin"
expect_status "an empty .nvmrc stops" 1
expect_output "and is named" ".nvmrc names no Node version"
echo "v$PIN" > "$CHECKOUT/.nvmrc"

finish_tests node-pin
