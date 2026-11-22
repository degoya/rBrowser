"""The restart rule of the settings lint (RB-094-23): every settings key is in one of two classes.

A key applies at once - the default, no annotation - or only at the next start of rBrowser, for
a reason the running app cannot get round (a CEF start parameter or command-line switch); such a
key carries `"x-restart": true` (crates/rb-config/src/annotate/restart.rs), and the settings
window shows "Applies after a restart" with its "Restart now" button. The rule: `x-restart` is
`true` or absent, and a key whose description or help text says it applies at the next start
carries it - so no key needs a restart without the window saying so. Used by settings_lint.py.
"""

import re

X_RESTART = "x-restart"

# "Applies at the next start.", "The change takes effect at the next start.", "Changes take
# effect after a restart." - not "loads at the next start" (a plugin) or "applies after the Node
# sidecar restarts" (a process of its own that starts again without rBrowser).
NEXT_START = re.compile(
    r"\b(?:applies|apply|takes? effect)\b[^.;:]*?"
    r"\b(?:at the next start|after a restart|when rBrowser (?:re)?starts(?: again)?)",
    re.IGNORECASE,
)


def says_next_start(text: object) -> bool:
    """Whether `text` says that a change applies only at the next start."""
    return isinstance(text, str) and NEXT_START.search(text) is not None


def problems(setting, known: dict) -> list[str]:
    """The restart problems of `setting`, its texts looked up in `known` (the catalogue)."""
    found = []
    value = setting.schema.get(X_RESTART)
    if value is not None and value is not True:
        found.append(f"{X_RESTART} is not true")
    texts = (known.get(setting.description_key), known.get(setting.help_key))
    if value is not True and any(says_next_start(text) for text in texts):
        found.append(
            f"its text says it applies at the next start but it has no {X_RESTART}: make it "
            "apply at once, or annotate it (crates/rb-config/src/annotate/restart.rs)"
        )
    return found
