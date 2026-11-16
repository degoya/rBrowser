# shellcheck shell=bash
#
# Bundled plugins for the fixtures of the generated wiki references (keyboard-reference.sh,
# mcp-reference.sh): `sample-tool` (prefix `samp`) with a command run by its tool `run`, a
# ui-only command without a group (so in `plugins`) and an `evaluates_input` tool; `core-twin`,
# whose tool `settings_set` is a core tool's name; `test-only`, carrying the marker TEST-ONLY,
# which no page lists. Sourced, never run.
#
#   source "$ROOT/scripts/tests/lib/plugin-fixture.sh"
#   plugin_fixture "$fixture"

plugin_fixture() {
    local root="$1"
    mkdir -p "$root/plugins/sample-tool/locales" "$root/plugins/core-twin" "$root/plugins/test-only"
    cat > "$root/plugins/sample-tool/manifest.toml" << 'EOF'
manifest_version = 1
id = "sample-tool"
name = "Sample Tool"
mcp_prefix = "samp"

[locales]
en = "locales/en.json"

[[tools]]
name = "run"
description = "plugin.sample-tool.tools.run"
input = { type = "object", required = ["depth"], properties = { depth = { type = "integer" } } }

[[tools]]
name = "exec"
description = "plugin.sample-tool.tools.exec"
evaluates_input = true

[[commands]]
id = "run"
title = "plugin.sample-tool.commands.run"
shortcut = "Ctrl+Shift+Space V X"
group = "view"
tool = "run"

[[commands]]
id = "panel"
title = "plugin.sample-tool.commands.panel"
shortcut = "Ctrl+Shift+Space X S"
mcp_omitted = "ui-only: a panel"
EOF
    cat > "$root/plugins/sample-tool/locales/en.json" << 'EOF'
{ "plugin": { "sample-tool": {
  "commands": { "run": "Run the sample", "panel": "Open the sample panel" },
  "tools": { "run": "Runs the sample in the views of the target.", "exec": "Runs a script." }
} } }
EOF
    cat > "$root/plugins/core-twin/manifest.toml" << 'EOF'
manifest_version = 1
id = "core-twin"
name = "Core Twin"
mcp_prefix = "settings"

[[tools]]
name = "set"
description = "plugin.core-twin.tools.set"
EOF
    cat > "$root/plugins/test-only/manifest.toml" << 'EOF'
manifest_version = 1
id = "test-only"
name = "Test Only"
mcp_prefix = "testonly"

[[tools]]
name = "hidden"
description = "plugin.test-only.tools.hidden"

[[commands]]
id = "hidden"
title = "plugin.test-only.commands.hidden"
shortcut = "Ctrl+Shift+Space X T"
tool = "hidden"
EOF
    : > "$root/plugins/test-only/TEST-ONLY"
}
