# Changes

## 0.1.0

The first version (RB-093-03): the response headers of a page's main document and the cookies it
sets, held to the common rules - `csp.missing`, `csp.unsafe` (`'unsafe-inline'`/`'unsafe-eval'`
that work), `csp.wildcard`, `csp.object_src`, `csp.base_uri`, `hsts.missing` and `hsts.short`
(never over `http:` or on a loopback host), `frame.missing`, `nosniff.missing`,
`referrer.missing`, `referrer.unsafe`, `permissions.missing`, `cors.credentials`, `cookie.flags`
(one finding per cookie naming the missing `Secure`, `HttpOnly`, `SameSite`; a cookie the answer
deletes - `Max-Age` 0 or an `Expires` before the answer's `Date` - is no live cookie), `cookie.prefix`
and `redirect.downgrade` (an HTTPS page that redirects to HTTP on the way).
The document is asked for once per URL through the host's `http` capability (HEAD, GET when HEAD
is refused, redirects followed up to five); the cookies a view holds for its page come through
CDP `Network.getCookies`; a cookie's value is dropped where it is read. One finding per URL and
environment, no view; the rules Lighthouse audits too share its audit ids. The MCP tools
`headers_check` and `headers_compare`, the check for `checks_run`, the commands `check`
(Ctrl+Shift+Space K U) and `compare_environments` (Ctrl+Shift+Space E H) and the comparison panel
(header x environment, the differences marked). Setting: `security-headers.hstsMinAge`.
