# shellcheck shell=bash
#
# The development tools' versions, one source each (RB-000-02): Rust is rust-toolchain.toml's
# `channel`, Node is .nvmrc; the rest are pinned here. All were the newest stable release when
# pinned; scripts/update-deps.sh lists what is newer, scripts/tool-versions.sh --check holds every
# copy (AGENTS.md, workflows) to these sources.
#
#   source "$ROOT/scripts/lib/tool-versions.sh"
#   echo "$RB_NEXTEST_VERSION"
#   rb_rust_version "$ROOT"          # 1.99.1
#   rb_node_version "$ROOT"          # 26.13.0
#   rb_use_pinned_node "$ROOT"       # node on PATH = .nvmrc (fnm or nvm), else fails
#
# Pinned 2026-11-10, each the newest stable release then. Sources: Rust `rustup check`
# (static.rust-lang.org), Node https://nodejs.org/dist/index.json, crates from crates.io
# (`cargo info`), shellcheck, gitleaks and actionlint from their GitHub releases. The CEF/Chromium pin is the
# `cef` crate version and our own SHA-256 of its archive (RB_CEF_*, below), the WebKit build our own
# SHA-256 of Playwright's archive (RB_WEBKIT_*); npm packages are pinned by each package-lock.json
# once ui/, agent/ and sidecar/ exist.

# shellcheck disable=SC2034  # read by the scripts that source this file
RB_NEXTEST_VERSION="0.9.151"
# shellcheck disable=SC2034
RB_CARGO_XWIN_VERSION="0.23.1"
# shellcheck disable=SC2034
RB_CARGO_DENY_VERSION="0.20.2"
# cargo-edit: `cargo upgrade --incompatible` in scripts/update-deps.sh; optional.
# shellcheck disable=SC2034
RB_CARGO_EDIT_VERSION="0.13.14"
# shellcheck disable=SC2034
RB_SHELLCHECK_VERSION="0.11.0"
# shellcheck disable=SC2034
RB_SHELLCHECK_SHA256="8c3be12b05d5c177a04c29e3c78ce89ac86f1595681cab149b65b97c4e227198"
# gitleaks: the secret scan of `check.sh --preflight`; pinned for a CI download. A local install
# may be any recent one.
# shellcheck disable=SC2034
RB_GITLEAKS_VERSION="8.30.1"
# shellcheck disable=SC2034
RB_GITLEAKS_SHA256="551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb"
# actionlint: the workflow lint of `check.sh --preflight` (RB-090-04); the SHA-256 is that of the
# linux_amd64 archive, from the release's checksums file, for scripts/ci-tools.sh.
# shellcheck disable=SC2034
RB_ACTIONLINT_VERSION="1.7.12"
# shellcheck disable=SC2034
RB_ACTIONLINT_SHA256="8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"
# komac: opens the winget-pkgs pull request of a release (RB-090-13, scripts/release-channels.sh
# winget); the SHA-256 is that of the x86_64-unknown-linux-gnu archive. rCommander's pin.
# shellcheck disable=SC2034
RB_KOMAC_VERSION="2.17.2"
# shellcheck disable=SC2034
RB_KOMAC_SHA256="b03b020eec98acfb0b53e284a6e21815b884be33de6785b97e7a120f0e748966"
# nfpm: writes the deb and rpm packages (RB-110-01, scripts/release-channels.sh install-nfpm); the
# SHA-256 is that of the Linux_x86_64 archive. rCommander's pin.
# shellcheck disable=SC2034
RB_NFPM_VERSION="2.47.0"
# shellcheck disable=SC2034
RB_NFPM_SHA256="0660ca602b2d2d2ae4781a06c692b3eeb9d437ffea05b831d76e41f4a3188783"

# CEF (RB-010-04, ADR 0001 Decision 1): the distribution the `cef` crate in Cargo.lock downloads
# into CEF_PATH, and our own SHA-256 of its archive - the crate checks only a SHA-1 from the CDN's
# own index. A new `cef` crate means a new archive: download it with a build, check it against
# https://cef-builds.spotifycdn.com/index.json and move all four values here.
# shellcheck disable=SC2034
RB_CEF_VERSION="156.0.26"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE="cef_binary_156.0.26+g8ea0c73+chromium-156.0.8078.88_windows64_minimal.tar.bz2"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_SHA256="2b0aa070793a6bb858b737f5edf9cb01a3ef7b9adcad5efe1e08affaf250f809"
# The macOS distributions of the same build (RB-090-15): the archives the `cef` crate downloads
# for aarch64-apple-darwin and x86_64-apple-darwin, held to our own SHA-256 the same way
# (scripts/lib/cef.sh, rb_cef_verify_macos). Pinned 2026-11-15, each checked against the SHA-1
# of its entry under `macosarm64` and `macosx64` in the index.
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_MACOS_ARM64="cef_binary_156.0.26+g8ea0c73+chromium-156.0.8078.88_macosarm64_minimal.tar.bz2"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_MACOS_ARM64_SHA256="6d7936cdd55ef30cf351a6358b2d922becefe87b25a8a8870a2b9ff5af2d39f6"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_MACOS_X64="cef_binary_156.0.26+g8ea0c73+chromium-156.0.8078.88_macosx64_minimal.tar.bz2"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_MACOS_X64_SHA256="28b343526664cacb8d855891ce112c62437684b4d9cbdc55fb5a6d7e5eac18fb"
# The release date of that build (RB-010-10): the `last_modified` day of the archive's entry under
# `windows64` in https://cef-builds.spotifycdn.com/index.json (the index has no other date).
# `crates/rbrowser/build.rs` embeds it; the shell warns once it is more than six weeks old.
# shellcheck disable=SC2034
RB_CEF_RELEASE_DATE="2026-11-07"

# WebKit (RB-040-04, threat model T11): the Windows build of the `webkit` revision that the pinned
# playwright-core (sidecar/package.json) names in its browsers.json, and our own SHA-256 of its
# archive - Playwright's installer checks no digest, so `engines.update` downloads, verifies and
# unpacks it itself (sidecar/src/webkit/install.ts). The archive is
# https://cdn.playwright.dev/dbazure/download/playwright/builds/webkit/<revision>/webkit-win64.zip
# (and the same path under https://playwright.download.prss.microsoft.com/dbazure/download/playwright).
# A new playwright-core means a new revision: download the archive from both mirrors, compare the
# SHA-256s and move all four values here and into sidecar/src/webkit/pin.ts;
# scripts/tool-versions.sh --check holds that copy to these.
# shellcheck disable=SC2034
RB_WEBKIT_PLAYWRIGHT_VERSION="1.65.0"
# shellcheck disable=SC2034
RB_WEBKIT_REVISION="2388"
# shellcheck disable=SC2034
RB_WEBKIT_ARCHIVE_SHA256="0b390661995316d8f90fe0f7a0498f3a88c451c097fc8e4047342e1fbd36548b"
# shellcheck disable=SC2034
RB_WEBKIT_ARCHIVE_BYTES="65443702"

# The Node runtime of the Windows package (RB-010-15): the official win-x64 ZIP of the .nvmrc
# version and its SHA-256 from https://nodejs.org/dist/v<version>/SHASUMS256.txt. A new .nvmrc
# needs both values here; scripts/package-windows.sh and scripts/tool-versions.sh --check refuse a
# pin for another version.
# shellcheck disable=SC2034
RB_NODE_WIN_X64_VERSION="26.13.1"
# shellcheck disable=SC2034
RB_NODE_WIN_X64_SHA256="e2edda75bfb089654705cf65a13b58933bfd5f57148e8ebda916f58a2474f1f7"
# rcedit (electron/rcedit, MIT): writes the icon and the version resource into CEF's
# bootstrap.exe, which the package ships as rbrowser.exe (ADR 0001). A Windows program: the package
# runs it natively (Git Bash) or through WSL interop. The SHA-256 of the release asset
# rcedit-x64.exe, taken 2026-11-10 (the release names no digest of its own).
# shellcheck disable=SC2034
RB_RCEDIT_VERSION="2.0.0"
# shellcheck disable=SC2034
RB_RCEDIT_SHA256="3e7801db1a5edbec91b49a24a094aad776cb4515488ea5a4ca2289c400eade2a"

# The Rust toolchain checkout $1 pins: rust-toolchain.toml's `channel`.
rb_rust_version() {
    sed -n 's/^channel = "\(.*\)"$/\1/p' "$1/rust-toolchain.toml" | head -n 1
}

# The Node version checkout $1 pins: .nvmrc, without a leading `v`. The packaged runtime and every
# build of ui/, agent/ and sidecar/ use it.
rb_node_version() {
    sed -n '1{s/^v//;s/[[:space:]]*$//;p;}' "$1/.nvmrc"
}

# The version of the `node` first on PATH, without the leading `v`; empty when there is none.
rb_node_found() {
    command -v node > /dev/null 2>&1 || return 0
    node --version 2> /dev/null | sed -n '1{s/^v//;p;}'
}

# Makes the `node` (and its npm) first on PATH the version checkout $1 pins in .nvmrc, for the
# calling shell and what it starts: kept when it already is; otherwise activated with fnm (`fnm
# env`, `fnm use`) or taken from nvm's install folder ($NVM_DIR/versions/node) when either holds
# it. Anything else fails with a message naming .nvmrc and the install command, so no front end is
# ever built or tested with another Node. Messages go to stderr.
rb_use_pinned_node() {
    local root="$1" want have env
    want="$(rb_node_version "$root")"
    if [[ -z "$want" ]]; then
        echo "!! $root/.nvmrc names no Node version" >&2
        return 1
    fi
    have="$(rb_node_found)"
    [[ "$have" != "$want" ]] || return 0
    if command -v fnm > /dev/null 2>&1 && env="$(fnm env --shell bash 2> /dev/null)"; then
        eval "$env"
        fnm use "$want" >&2 || true
    fi
    have="$(rb_node_found)"
    local nvm_bin="${NVM_DIR:-$HOME/.nvm}/versions/node/v$want/bin"
    if [[ "$have" != "$want" && -x "$nvm_bin/node" ]]; then
        PATH="$nvm_bin:$PATH"
        have="$(rb_node_found)"
    fi
    if [[ "$have" == "$want" ]]; then
        echo "==> node $want ($(command -v node)), as .nvmrc pins" >&2
        return 0
    fi
    local found="no node"
    [[ -z "$have" ]] || found="node $have"
    echo "!! $found is first on PATH, but $root/.nvmrc pins node $want." >&2
    echo "   install it in the checkout: \`fnm install $want\` (or \`nvm install $want\`), or put node $want first on PATH" >&2
    return 1
}
