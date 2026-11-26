"""Two rules on the texts of every catalogue (RB-080-90), for check_i18n.py:

* **No error code in a text.** A `{code}` placeholder sets a raw code into what the person reads
  ("Failed: plugins.error.host_failed"). A failure is a sentence: its code's text and its
  reason's (`PanelTexts.errorText` in plugins/web-common/panel/texts.js, the core's
  `failure_message`). The keys whose `{code}` is no error code (a language code, an exit code)
  are CODE_IS_DATA.
* **A count has a plural form.** `{n} views` reads "1 views"; a count is written as an ICU plural,
  `{n, plural, one {# view} other {# views}}`. Flagged: a placeholder directly followed by an
  English plural noun (a word ending in `s`) where the text has no plural of that placeholder -
  except a verb or a unit after it (VERBS) and a placeholder that is no count (NOT_COUNTS).

PENDING holds the texts whose fix belongs to another job or needs the catalogue's consumer made
plural-aware first, each with who fixes it; an entry whose text no longer offends is ignored.
"""

import re

PLACEHOLDER = re.compile(r"\{([A-Za-z_][A-Za-z0-9_]*)\}\s+([A-Za-z]+s)\b")
CODE = re.compile(r"\{code\s*[},]")

# Words ending in `s` after a placeholder that are no plural nouns.
VERBS = frozenset("""
    is was has does as lacks lies stays submits runs sets needs matches lists leaves keeps gives
    draws differs comes belongs points loads hits answers redirects blocks this its us yes plus
    ms fps DevTools Expires
""".split())

# Placeholders that are no count: names, addresses, sizes as text, states.
NOT_COUNTS = frozenset("""
    name label url target status detail script stylesheet width height ratio factor actual
    baseline value family role source text other image sitemap breakpoint reason
""".split())

# Keys whose `{code}` is data, not an error code.
CODE_IS_DATA = frozenset([
    # A language or region code of hreflang (ISO 639-1, ISO 3166-1).
    "plugin.seo.findings.hreflang.code_invalid",
    "plugin.seo.findings.hreflang.code_suggestion",
    "plugin.seo.findings.hreflang.content_detected",
    "plugin.seo.findings.hreflang.content_language_mismatch",
    "plugin.seo.findings.hreflang.duplicate_code",
    "plugin.seo.findings.hreflang.lang_mismatch",
    "plugin.seo.findings.hreflang.return_missing",
    "plugin.seo.findings.hreflang.set_incomplete",
    "plugin.seo.findings.hreflang.source_conflict",
    "plugin.seo.findings.hreflang.source_missing",
    "plugin.seo.findings.hreflang.target_canonical_language",
    "plugin.seo.findings.hreflang.target_relative",
    # An HTTP status or a network error of the fetched page.
    "plugin.seo.findings.index.fetch_failed",
    # Chromium's process exit code.
    "startup.cefFailed.detail",
    "startup.cefFailed.detailNoLog",
    "startup.cefFailed.other",
])

# Offending texts left to their owner, with who fixes them.
_NUXT = "the panel's Nuxt UI rebuild (RB-080-84 to 86): its formatter is not plural-aware yet"
PENDING = {
    "plugin.seo.findings.images.alt_too_long": "the SEO findings formatter (plugin tests) sets placeholders only; the count is always over 125",
    "plugin.breakpoint-analyzer.reasons.boundary": "the analyzer's formatter (plugin wasm)",
    "plugin.console.errors.expression_too_long": _NUXT,
    "plugin.console.panel.truncated": _NUXT,
    "plugin.coverage.findings.unused-file": _NUXT,
    "plugin.coverage.panel.cellTitle": _NUXT,
    "plugin.coverage.panel.last": _NUXT,
    "plugin.error-report.html.count": "the report's own formatter (plugin wasm, html.rs)",
    "plugin.error-report.html.generated": "the report's own formatter (plugin wasm, html.rs)",
    "plugin.error-report.panel.count": _NUXT,
    "plugin.error-report.panel.reloaded": _NUXT,
    "plugin.error-report.panel.truncated": _NUXT,
    "plugin.error-report.panel.viewDot": _NUXT,
    "plugin.json-xml-viewer.viewer.items": "the viewer's page script formatter (labels.ts)",
    "plugin.json-xml-viewer.viewer.keys": "the viewer's page script formatter (labels.ts)",
    "plugin.json-xml-viewer.viewer.results": "the viewer's page script formatter (labels.ts)",
    "plugin.json-xml-viewer.viewer.budget": "the viewer's page script formatter (labels.ts)",
    "plugin.network-panel.mocks.imported": _NUXT,
    "plugin.network-panel.sessions.opened": _NUXT,
    "plugin.network-panel.status.truncated": _NUXT,
    "plugin.sitemap-capture.gallery.summary": "the gallery's own formatter (plugin wasm, gallery.rs)",
    "plugin.snippets.errors.code_too_long": _NUXT,
    "plugin.snippets.panel.running": _NUXT,
    "plugin.storage-panel.errors.too_many_pins": _NUXT,
    "plugin.storage-panel.panel.engineHint": _NUXT,
    "plugin.storage-panel.panel.results": _NUXT,
    "plugin.storage-panel.panel.truncated": _NUXT,
}


def has_plural(text, name):
    """Whether `text` writes placeholder `name` as an ICU plural."""
    return re.search(r"\{\s*" + re.escape(name) + r"\s*,\s*plural\s*,", text) is not None


def problems(flat, source):
    """The texts of catalogue `flat` (key -> text) that break a rule, as `source: ...` lines."""
    found = []
    for key, text in sorted(flat.items()):
        if key in PENDING:
            continue
        if CODE.search(text) and key not in CODE_IS_DATA:
            found.append(f"{source}: `{key}` sets an error code into the text ({{code}}); "
                         "say the failure as a sentence (PanelTexts.errorText)")
        for match in PLACEHOLDER.finditer(text):
            name, word = match.group(1), match.group(2)
            if word in VERBS or name in NOT_COUNTS or has_plural(text, name):
                continue
            found.append(f"{source}: `{key}`: `{{{name}}} {word}` has no plural form; write "
                         f"{{{name}, plural, one {{# ...}} other {{# {word}}}}}")
    return found
