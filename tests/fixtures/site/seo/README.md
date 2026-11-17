# SEO check fixtures (RB-060-08)

Pages the native tests of `plugins/seo` serve through a `MemoryHost`
(`plugins/seo/tests/fixtures.rs`) at `https://fixture.test/<folder>/`; the expected findings are
the snapshots in `plugins/seo/tests/snapshots/`. `canonical-js/rendered.html` is what the view's
DOM holds after the page's script ran; `broken/` is served with `X-Robots-Tag: noindex`;
`clean/robots.txt` is the site's `robots.txt` for every fixture.

## hreflang and parity (RB-060-09)

`plugins/seo/tests/hreflang.rs` serves each language folder of `hreflang-broken/` and
`hreflang-ok/` at `https://fixture.test/<set>/<lang>/` with the response headers of the set's
`headers.json` (`Link`, `Content-Language`), its `robots.txt` as the origin's and its
`sitemap.xml`; the findings are the snapshots `plugins/seo/tests/snapshots/hreflang-*.txt` (K8).
`hreflang-broken/` holds a missing return link (`fr` does not link `en-UK`), `en-UK`, a
canonical to another language (`de` to `en`), a `Link` header that contradicts the head (`de` and
`fr` swapped on `en`), no `x-default`, and a sitemap entry without a counterpart (`es`, which
answers 404). `hreflang-ok/` is the same set done right. `parity/` holds the rendered DOM of a
desktop view (an `h1` only there) and a mobile view (JSON-LD only there).

## Social previews, well-known files, Microdata and the head (RB-080-11)

`plugins/seo/tests/groups.rs` serves each folder's `index.html` on the fixture site of
`plugins/seo/tests/common/` - which also serves `clean/llms.txt`, `clean/security.txt` (expires
2027-06-30; the tests' clock is 2026-11-12) and the share image `/img/share.jpg` (1200 x 630) -
and holds the findings of a broken fixture to `plugins/seo/tests/snapshots/<folder>.txt`; an ok
fixture has none. `social-broken/` has a relative `og:image` (served as a 600 x 300 PNG) and no
`twitter:card`; `wellknown-broken/` an `llms.txt` without H1 and a `security.txt` without
`Contact` that expired (both served at the site's root for its test); `microdata-broken/` a
Product without `name` whose `sku` contradicts the JSON-LD; `head-broken/` a `<meta charset>`
behind a comment of more than 1024 bytes and a `<title>` after a script. `social-ok/` is the page
the six preview cards are built from (`plugins/seo/tests/snapshots/social-ok.preview.json`, which
the panel's Vitest renders).
