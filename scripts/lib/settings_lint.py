"""The settings lint: every settings key of rBrowser has its place in the settings window.

    python3 -I scripts/lib/settings_lint.py <repo root>

Fails (exit 1), naming the file and the key, on: a key without x-category, x-section, x-order
or x-scope; a category that does not exist (a plugin cannot add a top-level category); a section
its category does not have; an x-order repeated inside a section (per source: the core, or one
plugin); a key defined twice; a local-only key whose x-scope is not ["local"]; a malformed or
missing i18n key - the label, description and help text of every key, the title of every
category and section; a path key without x-input, an x-input that is not "file" or "folder" or
sits on no string, an x-options that is no list of strings (RB-080-20); an option of a choice
without its name in the catalogue, a plugin key named `option` (RB-090-16, settings_options.py); a description or help text that
names an option by its id instead of its name (RB-090-29);
a notice's settings target or an `openSettings` key that opens no place in the window - no key,
category, section or key prefix (RB-090-85, settings_targets.py); a UI text that names a
settings path ("Settings > ...") without its link in crates/rbrowser/data/settings-links.json
(RB-094-21, settings_links.py); an `x-restart` that is not true, or a key whose text says it
applies at the next start without `x-restart` (RB-094-23, settings_restart.py); an `x-panel` on a
key that is no plugin's secret, of another shape or of no panel of its plugin, and a link to a
secret key that names no panel - a secret key is no place of the window (RB-094-27). It reads the core schemas (schema/*.json, schema/parts/*.json) and the settings schemas
of the bundled plugins (plugins/*/manifest.toml); the rules are crates/rb-settings's
(settings_schema.py). Exit 2 when the categories do not read. Compiles nothing: it runs in
scripts/check.sh --preflight through scripts/check-settings.sh.
"""

import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import settings_options  # noqa: E402
import settings_restart  # noqa: E402
import settings_schema  # noqa: E402
import settings_links  # noqa: E402
import settings_targets  # noqa: E402
from settings_schema import DataError  # noqa: E402


def lint(root: Path) -> tuple[list[str], str]:
    """The problems, one line each, and the summary line."""
    cats = settings_schema.categories(root)
    texts = settings_schema.catalogue(root)
    problems = []
    for category, sections in cats:
        titles = [f"settings.category.{category}"]
        titles += [f"settings.section.{category}.{section}" for section in sections]
        problems += [f"{settings_schema.CATALOGUE}: missing i18n key {key}" for key in titles
                     if key not in texts]
    settings, issues = settings_schema.read_all(root)
    for source, key, message in issues:
        problems.append(f"{source}: {key}: {message}" if key else f"{source}: {message}")
    try:
        plugin_texts = {plugin.id: plugin.catalogue for plugin in settings_schema.plugins(root)}
    except DataError:
        plugin_texts = {}
    for setting in settings:
        known = texts if setting.plugin is None else {**texts, **plugin_texts.get(setting.plugin, {})}
        for i18n_key in (setting.label_key, setting.description_key, setting.help_key):
            if i18n_key not in known:
                problems.append(f"{setting.source}: {setting.key}: missing i18n key {i18n_key}")
        for problem in settings_options.problems(setting, setting.root, known):
            problems.append(f"{setting.source}: {setting.key}: {problem}")
        for problem in settings_restart.problems(setting, known):
            problems.append(f"{setting.source}: {setting.key}: {problem}")
    target_problems, target_count = settings_targets.problems(root, settings, cats)
    problems += target_problems
    mention_problems, mention_count = settings_links.problems(root, settings, cats)
    problems += mention_problems
    core = sum(1 for setting in settings if setting.plugin is None)
    plugin_ids = {setting.plugin for setting in settings if setting.plugin is not None}
    summary = (
        f"settings lint: {len(settings)} keys ({core} core, {len(settings) - core} from "
        f"{len(plugin_ids)} plugin{'' if len(plugin_ids) == 1 else 's'}), "
        f"{len(cats)} categories, {target_count} notice targets, "
        f"{mention_count} settings mentions"
    )
    return problems, summary


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: settings_lint.py <repo root>", file=sys.stderr)
        return 2
    root = Path(argv[1]).resolve()
    try:
        problems, summary = lint(root)
    except DataError as error:
        print(f"!! the settings data does not read: {error}", file=sys.stderr)
        return 2
    for problem in problems:
        print(problem)
    if problems:
        print(f"!! {summary}: {len(problems)} problem(s)")
        return 1
    print(f"==> {summary}: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
