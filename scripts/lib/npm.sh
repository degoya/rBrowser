# shellcheck shell=bash
#
# The npm install rule of scripts/check.sh's npm stages (RB-080-25): a front end's node_modules/
# is installed with `npm ci` when it is missing and when its package-lock.json is newer than the
# install's own record, node_modules/.package-lock.json (npm writes it with every install) - a
# lock file changed by a merge, a dependency update or `npm install` never goes unnoticed. An
# install as new as the lock file is kept.
#
#   source "$ROOT/scripts/lib/npm.sh"
#   rb_npm_install_needed "$dir"      # 0 when `npm ci` is due, 1 when the install is current
#   rb_npm_install_if_stale "$dir"    # `npm ci` in $dir when it is due
#
# Tests: scripts/tests/npm.sh (a fake npm).

rb_npm_install_needed() {
    local dir="$1"
    [[ -d "$dir/node_modules" ]] || return 0
    [[ "$dir/package-lock.json" -nt "$dir/node_modules/.package-lock.json" ]]
}

rb_npm_install_if_stale() {
    local dir="$1"
    rb_npm_install_needed "$dir" || return 0
    (cd "$dir" && npm ci)
}
