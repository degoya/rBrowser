# shellcheck shell=bash
#
# The macOS app bundle (RB-090-15): rBrowser.app as CEF needs it on macOS - an app bundle with the
# framework and the helper apps, a bare binary does not run. Plain bash 3.2 (the macOS /bin/bash)
# and no macOS tool, so the layout and its check run - and are tested - on Linux too
# (scripts/tests/macos-bundle.sh); scripts/package-macos.sh builds, signs and packs it on a Mac.
#
#   rBrowser.app/Contents/
#     Info.plist                       net.rbrowser.app, the version, LSMinimumSystemVersion
#     MacOS/rbrowser                   the app (the crate's rbrowser-nosandbox binary)
#     Frameworks/Chromium Embedded Framework.framework     as the CEF distribution ships it
#     Frameworks/rbrowser Helper.app, rbrowser Helper (GPU).app, (Renderer), (Plugin), (Alerts)
#                                      each Contents/MacOS/<its name> (the rbrowser-helper binary)
#                                      and its Info.plist (net.rbrowser.app.helper[.gpu|...])
#     Resources/ui/                    the built shell; rbrowser.icns and the rest from the caller
#
# CEF finds the helpers by the app executable's name (`<name> Helper (GPU).app/Contents/MacOS/
# <name> Helper (GPU)` beside its framework), so they are named after `rbrowser`; their display
# name is "rBrowser Helper (...)".
#
#   source "$ROOT/scripts/lib/macos-bundle.sh"
#   rb_macos_stage <cef dir> <build dir> <app> <version> [ui dist]   # assembles the bundle
#   rb_macos_check <app>                                              # the structure test
#   rb_macos_plist <executable> <id> <name> <version> <helper 0|1> [icon]   # an Info.plist

# The bundle id, after rCommander's net.rcommander.app; tied to the website's domain (RB-110-09)
# once the owner decides it.
RB_MACOS_BUNDLE_ID="net.rbrowser.app"
# The oldest macOS: CEF's own deployment target (cmake/cef_variables.cmake, CEF_TARGET_SDK).
RB_MACOS_MINIMUM_SYSTEM="13.0"
# The app's executable; the helpers are named after it.
RB_MACOS_EXECUTABLE="rbrowser"
RB_MACOS_FRAMEWORK="Chromium Embedded Framework.framework"
# The helper apps CEF starts its subprocesses from, by the suffix of their name.
RB_MACOS_HELPERS=("" " (GPU)" " (Renderer)" " (Plugin)" " (Alerts)")

# The bundle id suffix of the helper with name suffix $1: "" -> "", " (GPU)" -> ".gpu".
rb_macos_helper_id() {
    local word="${1//[ ()]/}"
    [[ -z "$word" ]] && return 0
    printf '.%s\n' "$(printf '%s' "$word" | tr '[:upper:]' '[:lower:]')"
}

# Prints the Info.plist of a bundle: executable $1, bundle id $2, name $3, version $4 (at most
# three numbers, as Apple wants), helper $5 (1: no Dock icon, no menu bar), icon file $6 (main app).
rb_macos_plist() {
    local executable="$1" id="$2" name="$3" version="$4" helper="$5" icon="${6:-}" extra=""
    if [[ "$helper" == 1 ]]; then
        extra="    <key>LSUIElement</key>
    <true/>"
    elif [[ -n "$icon" ]]; then
        extra="    <key>CFBundleIconFile</key>
    <string>$icon</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>"
    fi
    cat << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>$name</string>
    <key>CFBundleExecutable</key>
    <string>$executable</string>
    <key>CFBundleIdentifier</key>
    <string>$id</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$name</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$version</string>
    <key>CFBundleVersion</key>
    <string>$version</string>
    <key>LSEnvironment</key>
    <dict>
        <key>MallocNanoZone</key>
        <string>0</string>
    </dict>
    <key>LSFileQuarantineEnabled</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>$RB_MACOS_MINIMUM_SYSTEM</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>NSBluetoothAlwaysUsageDescription</key>
    <string>A page in rBrowser asks for a Bluetooth device.</string>
    <key>NSCameraUsageDescription</key>
    <string>A page in rBrowser asks for the camera.</string>
    <key>NSMicrophoneUsageDescription</key>
    <string>A page in rBrowser asks for the microphone.</string>
    <key>NSWebBrowserPublicKeyCredentialUsageDescription</key>
    <string>A page in rBrowser asks for a passkey.</string>
$extra
</dict>
</plist>
PLIST
}

# Assembles bundle $3 (rBrowser.app, replaced) from the CEF distribution folder $1 (its framework
# at the top), the build output $2 (target/<triple>/<profile> with rbrowser-nosandbox and
# rbrowser-helper), the version $4 and, when it holds a build, the shell $5.
rb_macos_stage() {
    local cef="$1" build="$2" app="$3" version="$4" ui="${5:-}" contents suffix name file
    local short="${version%%[-+]*}"
    for file in rbrowser-nosandbox rbrowser-helper; do
        if [[ ! -f "$build/$file" ]]; then
            echo "!! $build has no $file: build -p rbrowser for macOS first" >&2
            return 1
        fi
    done
    if [[ ! -d "$cef/$RB_MACOS_FRAMEWORK" ]]; then
        echo "!! no $RB_MACOS_FRAMEWORK in $cef" >&2
        return 1
    fi
    rm -rf "$app"
    contents="$app/Contents"
    mkdir -p "$contents/MacOS" "$contents/Frameworks" "$contents/Resources"
    cp "$build/rbrowser-nosandbox" "$contents/MacOS/$RB_MACOS_EXECUTABLE"
    chmod 755 "$contents/MacOS/$RB_MACOS_EXECUTABLE"
    # As the distribution ships it, links and modes kept.
    cp -R "$cef/$RB_MACOS_FRAMEWORK" "$contents/Frameworks/"
    for suffix in "${RB_MACOS_HELPERS[@]}"; do
        name="$RB_MACOS_EXECUTABLE Helper$suffix"
        mkdir -p "$contents/Frameworks/$name.app/Contents/MacOS"
        cp "$build/rbrowser-helper" "$contents/Frameworks/$name.app/Contents/MacOS/$name"
        chmod 755 "$contents/Frameworks/$name.app/Contents/MacOS/$name"
        rb_macos_plist "$name" "$RB_MACOS_BUNDLE_ID.helper$(rb_macos_helper_id "$suffix")" \
            "rBrowser Helper$suffix" "$short" 1 > "$contents/Frameworks/$name.app/Contents/Info.plist"
    done
    rb_macos_plist "$RB_MACOS_EXECUTABLE" "$RB_MACOS_BUNDLE_ID" rBrowser "$short" 0 rbrowser \
        > "$contents/Info.plist"
    if [[ -n "$ui" && -f "$ui/index.html" ]]; then
        cp -R "$ui" "$contents/Resources/ui"
    fi
}

# The structure test of bundle $1: the app, the framework, the five helpers with their
# executables and Info.plists naming them, the shell. Every problem is named; 1 when any.
rb_macos_check() {
    local app="$1" contents="$1/Contents" suffix name problems=0 path
    for path in "$contents/Info.plist" "$contents/Resources/ui/index.html" \
        "$contents/Frameworks/$RB_MACOS_FRAMEWORK/Chromium Embedded Framework"; do
        [[ -f "$path" ]] || { echo "!! $app: no ${path#"$app/"}" >&2; problems=1; }
    done
    [[ -x "$contents/MacOS/$RB_MACOS_EXECUTABLE" ]] \
        || { echo "!! $app: no executable Contents/MacOS/$RB_MACOS_EXECUTABLE" >&2; problems=1; }
    grep -q "<string>$RB_MACOS_EXECUTABLE</string>" "$contents/Info.plist" 2> /dev/null \
        || { echo "!! $app: Info.plist names no $RB_MACOS_EXECUTABLE" >&2; problems=1; }
    for suffix in "${RB_MACOS_HELPERS[@]}"; do
        name="$RB_MACOS_EXECUTABLE Helper$suffix"
        path="$contents/Frameworks/$name.app/Contents"
        [[ -x "$path/MacOS/$name" ]] \
            || { echo "!! $app: no executable $name in $name.app" >&2; problems=1; }
        grep -q "<string>$name</string>" "$path/Info.plist" 2> /dev/null \
            || { echo "!! $app: $name.app's Info.plist names no $name" >&2; problems=1; }
    done
    return "$problems"
}
