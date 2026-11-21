# Changes

## 0.1.0

The first version (RB-094-05): what a page loads and stores before any consent - the requests
to other sites (eTLD+1, approximately) by host and kind (script, pixel, font, iframe, ...), the
cookies by name, domain and lifetime (never a value) and the localStorage keys - and after
"Reject all" of its cookie banner once more, with the differences. Rules:
`request.before_consent`, `cookie.before_consent` (warnings), `request.after_reject`,
`cookie.after_reject` (errors). The banner detection and the "Reject all" click are the collector
`src/collect.js` over the consent managers of `data/cmp.json` (OneTrust, Cookiebot,
Usercentrics, Didomi, Borlabs, Klaro, CookieYes, GDPR Cookie Consent, Complianz, Quantcast,
TrustArc and a generic banner by its button text); their consent cookies and common session and
CSRF cookies count as essential. The tool and the command run in a fresh session of
their own, discarded afterwards: the core's consent probe (RB-094-09, capability
`consent-probe`), asked for with the page and the collectors and answered with the capture
`$consent`; a `checks_run` reads the target view as it is, read only, and names the caveats. The MCP tool `consent_check` (`url`, `export`), the check for
`checks_run`, the command `check` (Ctrl+Shift+Space K Y) and the panel; every answer, the panel,
the export and each finding's data carry the notice: a technical pre-check, no legal advice.
Settings: `consent-check.allowHosts`, `consent-check.rejectStep`,
`consent-check.essentialCookies`.
