# Changes

## Unreleased

- Changed (RB-080-85): the panel is built on Nuxt UI - Refresh as an icon button, one status line,
  a card per platform with its variant, how sure its rule is and a link to the rule's source.
- Added (RB-080-99): every platform's card follows its published rule (`data/social.json`
  version 2: `twitter:card`, the share image's real size, always large, or unverified) with the
  reason in words; a toggle shows the other variant below the chosen one; where no rule is
  published or the image's size is unknown both variants are shown, marked. New platforms:
  iMessage (in the default selection), Telegram, Microsoft Teams, Mastodon, Bluesky, Threads,
  Signal. New finding `social.card_small` (Info): a chosen platform shows the image small
  although `twitter:card` asks for a large one. Discord cuts at its documented 70 and 350.
- Changed (RB-080-62): the panel is laid out with the panel kit - Refresh as an icon button,
  the status line with an icon, the cards as kit cards with their layout as a badge.
RB-080-11: social previews (`plugin.seo.social_preview`, `Ctrl+Shift+Space K G`, MCP
`seo_social_preview`): the SEO panel shows the link previews of Facebook, X, LinkedIn, Slack,
WhatsApp and Discord from the raw HTML (OG, then Twitter, then title and description; X the other
way round), cut at the lengths of `data/social.json` (versioned), the share image fetched by the
component and shown as a `data:` URI. Rule groups: `social` adds `image_relative`,
`image_unreachable`, `image_type`, `image_small` (`seo.social.imageMinWidth`/`imageMinHeight`,
1200 x 630), `image_ratio` (1.91:1), `twitter_card`, `url_canonical`; `wellknown` (`llms.txt` in
the llmstxt.org format and its links, `seo.wellknown.maxLinks`; `security.txt` per RFC 9116:
place, HTTPS, `text/plain`, `Contact`, `Expires` ahead and within a year;
`plugin.seo.check_wellknown`, `Ctrl+Shift+Space K W`); `microdata` (Microdata and RDFa Lite:
type, the types of `data/schema.json`, required properties of Product, Article, BreadcrumbList,
Organization, FAQPage and their parts, contradictions to the JSON-LD); `head` (the raw head:
charset in the first 1024 bytes and first, viewport and title before resources, preconnect,
synchronous scripts before CSS, head elements in the body, the rendered order as a hint). Every
group switches off with `seo.<group>.enabled`; `seo_check` takes `groups`.

RB-060-09: the hreflang set (`scope: "hreflang"`, `plugin.seo.hreflang`, `Ctrl+Shift+Space K H`):
the alternates of the head, the `Link` header and the XML sitemap (`xhtml:link`, indexes
followed), every alternate that answers 200 followed, at most 50 pages, each origin's robots.txt
kept. Rule group `hreflang`: the sources agree, valid codes with a correction (`en-UK` to
`en-GB`, `de_DE` to `de-DE`), absolute URLs, no duplicates, self reference, `x-default`
(`seo.requireXDefault`), the locales of `seo.locales`, each target 200 without a redirect,
indexable, self-canonical (a canonical to another language named as such), `lang` and
`Content-Language` matching the code (and the text, with `seo.hreflang.checkContentLanguage`),
return links as a matrix in the report. Across the set: shared titles and descriptions, alt
texts left untranslated (`images.alt_untranslated`). Parity of the views (`parity: true`,
`plugin.seo.parity`, `Ctrl+Shift+Space K P`): title, h1, canonical, hreflang, JSON-LD types and
the visible main text of every Chromium view showing the page against the reference view
(`seo.parity.reference`, `seo.parity.textSimilarity`).

## 0.1.0

The first version (RB-060-08): checks a page, or a mini crawl of up to `seo.maxPages` pages of its
origin (robots.txt kept, four requests a second), for the SEO essentials over three sources - the
raw response (status, redirect chain, `X-Robots-Tag`, `Link`), the raw HTML and the rendered DOM
of the view that shows it, whose differences are findings of their own (`render.differs`, such as
a canonical set only by JavaScript). Rule groups: `index`, `canonical`, `meta` (title,
description, `lang`, viewport, charset, duplicates across a crawl), `headings`, `social` (Open
Graph basics), `structured` (JSON-LD syntax), `site` (the sitemap in robots.txt and reachable) and
the alt text group `images` (missing, empty where not decorative, linked without a name, file
name, placeholder, "image of", too long, duplicate, repeating its caption, another language,
`input type=image`, `area`, inline SVG, share image alt, large background images). Shares
`img-alt-missing`, `link-name-missing` and `html-lang-missing` with `a11y-axe`. Settings in
Checks > SEO; MCP `seo_check` and the command `plugin.seo.check` (`Ctrl+Shift+Space K S`) write
`.rbrowser/out/seo/<run>/report.{json,html}`.
