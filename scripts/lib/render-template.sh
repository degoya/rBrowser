# shellcheck shell=bash
#
# Fills a packaging template's `@NAME@` placeholders (RB-090-13, RB-110-04): what scripts/package-managers.sh
# and scripts/package-repo.sh share. Copied from rCommander's scripts/lib/render-template.sh.
#
#   source "$ROOT/scripts/lib/render-template.sh"
#   rb_render_template <template> <output> NAME=value...
#
# A placeholder left over after every pair is an error: the template names a value the caller
# does not know, and a package manifest with `@SHA256@` in it would be published as it is. The
# caller turns off Bash 5.2's patsub_replacement, which would put the matched text in place of a
# `&` in a value.

rb_render_template() {
    local template="$1" output="$2" content pair
    shift 2
    content="$(< "$template")"
    for pair in "$@"; do
        content="${content//@${pair%%=*}@/${pair#*=}}"
    done
    if [[ "$content" =~ @[A-Z0-9_]+@ ]]; then
        echo "error: $template keeps ${BASH_REMATCH[0]} after rendering" >&2
        return 1
    fi
    printf '%s\n' "$content" > "$output"
}
