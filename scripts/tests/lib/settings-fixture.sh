# shellcheck shell=bash
#
# A small repository for the settings tests (settings-lint.sh, settings-reference.sh). Sourced,
# never run. `settings_fixture <dir>` writes it and prints <dir>: the real categories
# (crates/rb-settings/src/category.rs) and their titles (`settings.category` and
# `settings.section` of locales/en.json and the wave parts not merged yet, as the part
# locales/parts/titles.json) copied from the checkout, a project schema in the shape schemars writes (five keys: startUrl, sync.scroll,
# sync.navigation, the local-only network.ignoreCertificateErrors and network.proxy), their
# labels, descriptions and help texts, and the bundled plugin `seo` with one key, checks > seo.
#
#   source "$ROOT/scripts/tests/lib/settings-fixture.sh"
#   fixture="$(settings_fixture "$SCRATCH/case")"

settings_fixture() {
    local dir="$1"
    mkdir -p "$dir/crates/rb-settings/src" "$dir/schema" "$dir/locales/parts" \
        "$dir/plugins/seo/locales"
    cp "$ROOT/crates/rb-settings/src/category.rs" "$dir/crates/rb-settings/src/"
    python3 -I - "$ROOT/locales" "$dir/locales/parts/titles.json" << 'EOF'
import json, sys
from pathlib import Path

def merge(target, source):
    for key, value in source.items():
        if isinstance(value, dict):
            merge(target.setdefault(key, {}), value)
        else:
            target[key] = value

titles = {"category": {}, "section": {}}
locales = Path(sys.argv[1])
for path in [locales / "en.json"] + sorted((locales / "parts").glob("*.json")):
    settings = json.loads(path.read_text(encoding="utf-8")).get("settings", {})
    for part in titles:
        merge(titles[part], settings.get(part, {}))
with open(sys.argv[2], "w", encoding="utf-8") as handle:
    json.dump({"settings": titles}, handle, indent=2)
EOF

    cat > "$dir/schema/config.v1.json" << 'EOF'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "$schema": { "type": "string" },
    "version": { "type": "integer" },
    "startUrl": {
      "type": "string", "default": "",
      "x-category": "project", "x-section": "start-url", "x-order": 1, "x-scope": ["project", "local"]
    },
    "sync": { "$ref": "#/$defs/Sync" },
    "network": { "anyOf": [{ "$ref": "#/$defs/Network" }, { "type": "null" }] }
  },
  "$defs": {
    "Sync": {
      "type": "object",
      "properties": {
        "scroll": {
          "type": "boolean", "default": true,
          "x-category": "sync", "x-section": "scroll", "x-order": 1, "x-scope": ["project", "local"]
        },
        "navigation": {
          "type": "boolean", "default": true,
          "x-category": "sync", "x-section": "navigation", "x-order": 1, "x-scope": ["project", "local"]
        }
      }
    },
    "Network": {
      "type": "object",
      "properties": {
        "ignoreCertificateErrors": {
          "type": "boolean", "default": false,
          "x-category": "network", "x-section": "certificates", "x-order": 1,
          "x-scope": ["local"], "x-local-only": true
        },
        "proxy": {
          "$ref": "#/$defs/Proxy",
          "x-category": "network", "x-section": "certificates", "x-order": 2,
          "x-scope": ["local"], "x-local-only": true
        }
      }
    },
    "Proxy": { "type": "object", "properties": { "server": { "type": "string" } }, "default": {} }
  }
}
EOF

    cat > "$dir/locales/en.json" << 'EOF'
{
  "settings": {
    "key": {
      "startUrl": {
        "label": "Start URL", "description": "The page every view opens first.",
        "help": "The URL every view loads when the project opens."
      },
      "sync": {
        "scroll": {
          "label": "Sync scrolling", "description": "Scroll every view together.",
          "help": "Scrolling one view scrolls the others to the same place."
        },
        "navigation": {
          "label": "Sync navigation", "description": "Navigate every view together.",
          "help": "A navigation in one view is replayed in the others."
        }
      },
      "network": {
        "ignoreCertificateErrors": {
          "label": "Ignore certificate errors",
          "description": "Accept invalid certificates on this machine.",
          "help": "For local servers with self-signed certificates."
        },
        "proxy": {
          "label": "Proxy", "description": "The proxy of this machine.",
          "help": "The proxy server every view connects through."
        }
      }
    }
  }
}
EOF

    cat > "$dir/plugins/seo/manifest.toml" << 'EOF'
manifest_version = 1
id = "seo"
version = "0.1.0"

[settings]
schema = "settings.schema.json"

[locales]
en = "locales/en.json"
EOF

    cat > "$dir/plugins/seo/settings.schema.json" << 'EOF'
{
  "type": "object",
  "properties": {
    "titleLength": {
      "type": "integer", "default": 60, "minimum": 10,
      "x-category": "checks", "x-section": "seo", "x-order": 1, "x-scope": ["project", "local"]
    }
  }
}
EOF

    cat > "$dir/plugins/seo/locales/en.json" << 'EOF'
{
  "plugin": {
    "seo": {
      "settings": {
        "titleLength": {
          "label": "Title length",
          "description": "The longest title the SEO check accepts.",
          "help": "A longer title is cut in search results."
        }
      }
    }
  }
}
EOF
    echo "$dir"
}
