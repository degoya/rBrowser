# shellcheck shell=bash
#
# The folder scripts/build-plugins.sh packs for a first-party plugin (RB-080-65). A plugin whose
# classic-script panel loads files of plugins/web-common/panel/ (the syntax highlighter, the
# panel kit) names them, one per line, in `plugins/<id>/web-common.txt` (`#` starts a comment);
# they are not checked in twice. The plugin's folder is copied to a stage - what rb-plugin-pack
# reads: manifest.toml, settings.schema.json, locales/, page/, panel/ - and each listed file is
# added to the stage's panel/. A name with a path, a missing file, or a panel file of the same
# name in the plugin itself (a stale copy) is refused. Without the list the plugin's own folder
# is packed as it is. Sourced, never run.
#
#   source scripts/lib/plugin-stage.sh
#   dir="$(rb_plugin_pack_dir <plugins-dir> <id> <stage-dir>)"

# Prints the folder to pack for plugin `id` of `plugins`; builds the stage in `stage` when the
# plugin lists web-common files. Returns 1 with the reason on stderr.
rb_plugin_pack_dir() {
    local plugins="$1" id="$2" stage="$3"
    local list="$plugins/$id/web-common.txt" member line name
    if [[ ! -f "$list" ]]; then
        printf '%s\n' "$plugins/$id"
        return 0
    fi
    rm -rf "$stage"
    mkdir -p "$stage/panel"
    for member in manifest.toml settings.schema.json locales page panel; do
        [[ -e "$plugins/$id/$member" ]] && cp -R "$plugins/$id/$member" "$stage/"
    done
    while IFS= read -r line || [[ -n "$line" ]]; do
        name="${line%%#*}"
        name="${name//[[:space:]]/}"
        [[ -n "$name" ]] || continue
        if [[ ! "$name" =~ ^[A-Za-z0-9_-][A-Za-z0-9._-]*$ ]]; then
            echo "!! plugins/$id/web-common.txt: '$name' is no file name of plugins/web-common/panel" >&2
            return 1
        fi
        if [[ ! -f "$plugins/web-common/panel/$name" ]]; then
            echo "!! plugins/$id/web-common.txt: plugins/web-common/panel/$name does not exist" >&2
            return 1
        fi
        if [[ -e "$plugins/$id/panel/$name" ]]; then
            echo "!! plugins/$id/panel/$name: a copy of a listed web-common file; delete it" >&2
            return 1
        fi
        cp "$plugins/web-common/panel/$name" "$stage/panel/$name"
    done < "$list"
    printf '%s\n' "$stage"
}
