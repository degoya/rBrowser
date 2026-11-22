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
#   rb_sha256 <file>                 # sha256sum, or shasum -a 256 on macOS
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
# nfpm: writes the deb and rpm packages (RB-097-05, scripts/release-channels.sh install-nfpm); the
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
RB_CEF_VERSION="156.0.31"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE="cef_binary_156.0.31+ge603d97+chromium-156.0.8078.100_windows64_minimal.tar.bz2"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_SHA256="c691636862b5f6bab05e76d77bd1b57799659552a25a5bc8b7eaa365de6e63fd"
# The macOS distributions of the same build (RB-090-15): the archives the `cef` crate downloads
# for aarch64-apple-darwin and x86_64-apple-darwin, held to our own SHA-256 the same way
# (scripts/lib/cef.sh, rb_cef_verify_macos). Pinned 2026-11-22, each checked against the SHA-1
# of its entry under `macosarm64` and `macosx64` in the index.
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_MACOS_ARM64="cef_binary_156.0.31+ge603d97+chromium-156.0.8078.100_macosarm64_minimal.tar.bz2"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_MACOS_ARM64_SHA256="1bd39ff38db20baa5b52768919d6599aafb4aacbc7fd192458496749a0630ee8"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_MACOS_X64="cef_binary_156.0.31+ge603d97+chromium-156.0.8078.100_macosx64_minimal.tar.bz2"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_MACOS_X64_SHA256="58ffeec9beff9f0f57cca19fa950674f9756c1b28ba595129ba613806633b6fa"
# The release date of that build (RB-010-10): the `last_modified` day of the archive's entry under
# `windows64` in https://cef-builds.spotifycdn.com/index.json (the index has no other date).
# `crates/rbrowser/build.rs` embeds it; the shell warns once it is more than six weeks old.
# shellcheck disable=SC2034
RB_CEF_RELEASE_DATE="2026-11-18"
# Linux x64 (RB-097-05): the distribution the `cef` crate downloads for x86_64-unknown-linux-gnu
# (entry `linux64` of the index, its SHA-1 0adb813face4ca098f17210fe471b172c2bc1950 checked) and
# our own SHA-256 of it, taken 2026-11-22 (scripts/lib/cef.sh, rb_cef_verify_linux); and the
# Node runtime of the Linux package, the official linux-x64 tarball of the .nvmrc version with
# its SHA-256 from https://nodejs.org/dist/v<version>/SHASUMS256.txt. A new .nvmrc needs both
# Node values; scripts/package-linux.sh refuses a pin for another version.
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_LINUX_X64="cef_binary_156.0.31+ge603d97+chromium-156.0.8078.100_linux64_minimal.tar.bz2"
# shellcheck disable=SC2034
RB_CEF_ARCHIVE_LINUX_X64_SHA256="a6c3452c47f344f9f125c71dfc9d8965149bac7c8df1e23ffd80442a28ce1b62"
# shellcheck disable=SC2034
RB_NODE_LINUX_X64_VERSION="26.13.1"
# shellcheck disable=SC2034
RB_NODE_LINUX_X64_SHA256="5c063e46bab0fbc1c17cc445d7ff129cbdcd0471be6ed83b6ab864f8143bc864"

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
RB_WEBKIT_PLAYWRIGHT_VERSION="1.65.1"
# shellcheck disable=SC2034
RB_WEBKIT_REVISION="2388"
# shellcheck disable=SC2034
RB_WEBKIT_ARCHIVE_SHA256="0b390661995316d8f90fe0f7a0498f3a88c451c097fc8e4047342e1fbd36548b"
# shellcheck disable=SC2034
RB_WEBKIT_ARCHIVE_BYTES="65443702"

# WebKit on macOS (RB-097-02): Playwright builds that WebKit per macOS version and CPU, and
# `engines.update` takes the archive of the host (sidecar/src/webkit/pin.ts `hostPlatform`): for
# each host its SHA-256 and size, taken 2026-11-22 (cdn.playwright.dev redirects to the Microsoft
# mirror, which served them). The archive is
# <mirror>/builds/webkit/<revision>/webkit-mac-<version>[-arm64].zip; macOS 14 keeps revision 2251
# (browsers.json `revisionOverrides`), the others take RB_WEBKIT_REVISION. A new playwright-core
# moves these with the Windows pin; scripts/tool-versions.sh --check holds `MAC_PINS` in pin.ts to
# them.
# shellcheck disable=SC2034
RB_WEBKIT_MAC14_SHA256="50ce5d7004274b01b106dda5ceee6703013b7bd0d0384457cd93a524ced34202"
# shellcheck disable=SC2034
RB_WEBKIT_MAC14_BYTES="86430592"
# shellcheck disable=SC2034
RB_WEBKIT_MAC14_ARM64_SHA256="2754eca5bab8773a6b1315528e7d8f342977ace799b3a91314e1be0cb324ee49"
# shellcheck disable=SC2034
RB_WEBKIT_MAC14_ARM64_BYTES="82217310"
# shellcheck disable=SC2034
RB_WEBKIT_MAC15_SHA256="c1220c739a2dbb6450059bfd678d5f5057e807d1d004464ef890ee1e6a9db727"
# shellcheck disable=SC2034
RB_WEBKIT_MAC15_BYTES="93121084"
# shellcheck disable=SC2034
RB_WEBKIT_MAC15_ARM64_SHA256="50b4238e145b8018e48aacefc15e9b3a43fafd18eee42ed4cb29a2e5c5a2e385"
# shellcheck disable=SC2034
RB_WEBKIT_MAC15_ARM64_BYTES="85207540"
# shellcheck disable=SC2034
RB_WEBKIT_MAC26_SHA256="43603b6130fc1341e02b87dde0d635f02b6d2defb90bc4f4e4b4a2d101900872"
# shellcheck disable=SC2034
RB_WEBKIT_MAC26_BYTES="93747438"
# shellcheck disable=SC2034
RB_WEBKIT_MAC26_ARM64_SHA256="d3cc730027105a014d0cbf53925bd7eababbdf8dbccbb4a3a1713c5f631dbcaf"
# shellcheck disable=SC2034
RB_WEBKIT_MAC26_ARM64_BYTES="85789503"

# The Node runtime of the Windows package (RB-010-15): the official win-x64 ZIP of the .nvmrc
# version and its SHA-256 from https://nodejs.org/dist/v<version>/SHASUMS256.txt. A new .nvmrc
# needs both values here; scripts/package-windows.sh and scripts/tool-versions.sh --check refuse a
# pin for another version.
# shellcheck disable=SC2034
RB_NODE_WIN_X64_VERSION="26.13.1"
# shellcheck disable=SC2034
RB_NODE_WIN_X64_SHA256="e2edda75bfb089654705cf65a13b58933bfd5f57148e8ebda916f58a2474f1f7"
# The Node runtime of the macOS app bundle (RB-097-01): the official darwin-arm64 and darwin-x64
# tar.gz of the .nvmrc version, each with its SHA-256 from the same SHASUMS256.txt (taken
# 2026-11-22; its win-x64 line is the pin above). scripts/package-macos.sh stages bin/node and
# LICENSE into rBrowser.app/Contents/Resources/node; a new .nvmrc needs all three values here, and
# scripts/tool-versions.sh --check refuses a pin for another version.
# shellcheck disable=SC2034
RB_NODE_DARWIN_VERSION="26.13.1"
# shellcheck disable=SC2034
RB_NODE_DARWIN_ARM64_SHA256="aec2514ea1ceb2ba52455e9ec65986f6749a06dd759e5db2a72e6889668c9735"
# shellcheck disable=SC2034
RB_NODE_DARWIN_X64_SHA256="82bb96078a24ca50b9c9a38f2379e6be6d87171fcd400fc14c8934b3bce96e2f"
# rcedit (electron/rcedit, MIT): writes the icon and the version resource into CEF's
# bootstrap.exe, which the package ships as rbrowser.exe (ADR 0001). A Windows program: the package
# runs it natively (Git Bash) or through WSL interop. The SHA-256 of the release asset
# rcedit-x64.exe, taken 2026-11-10 (the release names no digest of its own).
# shellcheck disable=SC2034
RB_RCEDIT_VERSION="2.0.0"
# shellcheck disable=SC2034
RB_RCEDIT_SHA256="3e7801db1a5edbec91b49a24a094aad776cb4515488ea5a4ca2289c400eade2a"

# The SHA-256 of file $1 with what the system has: sha256sum, or shasum on macOS.
rb_sha256() {
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

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
