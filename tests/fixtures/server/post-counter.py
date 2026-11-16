#!/usr/bin/env python3
"""The fixture server of the sync jobs (RB-020-02, RB-020-03; K1, K2, K4): serves
tests/fixtures/site on 127.0.0.1:18766 like `python3 -m http.server`, plus

* POST /submit - counts the form posts (form.html), answers 303 to /form.html?posts=<n>;
* GET /count - the posts so far as JSON, `{"posts": n}`; the leader-only submit check (K4)
  expects exactly one per submit in the leader, whatever the number of views;
* POST /reset - the count back to 0;
* GET /search?q=... - a result page that shows the query (form.html's GET search, Enter).

    python3 tests/fixtures/server/post-counter.py [port]

Only for the local end-to-end runs (scripts/e2e-windows.sh, docs/sync-protocol.md); it binds
127.0.0.1 and keeps nothing on disk.
"""

import functools
import html
import http.server
import json
import os
import sys
import threading
import urllib.parse

DEFAULT_PORT = 18766
MAX_BODY = 64 * 1024


class Counter:
    def __init__(self):
        self.lock = threading.Lock()
        self.posts = 0

    def add(self):
        with self.lock:
            self.posts += 1
            return self.posts

    def reset(self):
        with self.lock:
            self.posts = 0

    def value(self):
        with self.lock:
            return self.posts


COUNTER = Counter()


class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        url = urllib.parse.urlsplit(self.path)
        if url.path == "/count":
            self.reply("application/json", json.dumps({"posts": COUNTER.value()}).encode())
        elif url.path == "/search":
            query = urllib.parse.parse_qs(url.query).get("q", [""])[0]
            body = (
                "<!doctype html><meta charset='utf-8'><title>Search</title>"
                f"<h1 id='result'>Results for {html.escape(query)}</h1>"
            )
            self.reply("text/html; charset=utf-8", body.encode())
        else:
            super().do_GET()

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        if length > MAX_BODY:
            self.send_error(413)
            return
        self.rfile.read(length)
        path = urllib.parse.urlsplit(self.path).path
        if path == "/submit":
            posts = COUNTER.add()
            self.send_response(303)
            self.send_header("Location", f"/form.html?posts={posts}")
            self.send_header("Content-Length", "0")
            self.end_headers()
        elif path == "/reset":
            COUNTER.reset()
            self.reply("application/json", b'{"posts": 0}')
        else:
            self.send_error(404)

    def reply(self, content_type, body):
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_PORT
    here = os.path.dirname(os.path.abspath(__file__))
    folder = os.path.normpath(os.path.join(here, "..", "site"))
    handler = functools.partial(Handler, directory=folder)
    with http.server.ThreadingHTTPServer(("127.0.0.1", port), handler) as server:
        print(f"serving {folder} on http://127.0.0.1:{port}/", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
