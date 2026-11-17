#!/usr/bin/env python3
"""The storage fixture of RB-070-01: the pages of this folder on 127.0.0.1:18767, and an HttpOnly
session cookie `session_id` set with the first answer of each browser session, so the storage
panel shows (and deletes) a cookie that `document.cookie` cannot see.

    python3 tests/fixtures/site/storage/server.py [port]

Windows acceptance: a project with startUrl http://127.0.0.1:18767/, sessions ["admin"], three
breakpoints (two without a session, one with "admin"); see docs/roadmap/jobs/070-01-*.md.
"""

import http.server
import os
import secrets
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=ROOT, **kwargs)

    def end_headers(self):
        if "session_id=" not in (self.headers.get("Cookie") or ""):
            self.send_header("Set-Cookie", f"session_id={secrets.token_hex(8)}; HttpOnly; Path=/")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def do_GET(self):
        if self.path in ("/", ""):
            self.path = "/index.html"
        if not self.path.split("?")[0].endswith(".html"):
            self.send_error(404)
            return
        super().do_GET()


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18767
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
    print(f"storage fixture on http://127.0.0.1:{port}/")
    server.serve_forever()


if __name__ == "__main__":
    main()
