#!/usr/bin/env python3
"""The login fixture of RB-020-07: a page on 127.0.0.1:18766 that logs in with a form POST and
keeps the login in a session cookie (no expiry), for the Windows acceptance of sessions.

* GET  /        - "Logged in as <name>" or the login form;
* POST /login   - logs in as the posted name, sets `sid`, redirects to /;
* GET  /logout  - drops the login;
* GET  /posts   - how many login POSTs each name sent, as JSON (one per session with the
  leader-per-session rule).

    python3 sessions-server.py [port]
"""

import html
import http.server
import json
import secrets
import sys
import urllib.parse

LOGINS = {}
POSTS = {}
FORM = """<!doctype html><title>Sessions fixture</title>
<h1>Logged out</h1>
<form method="post" action="/login"><input name="name" value="admin"><button>Log in</button></form>"""


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/posts":
            self.reply(200, "application/json", json.dumps(POSTS))
        elif self.path == "/logout":
            LOGINS.pop(self.sid(), None)
            self.redirect("/")
        else:
            name = LOGINS.get(self.sid())
            body = FORM if name is None else (
                "<!doctype html><title>Sessions fixture</title>"
                f"<h1>Logged in as {html.escape(name)}</h1><a href=\"/logout\">Log out</a>")
            self.reply(200, "text/html; charset=utf-8", body)

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        form = urllib.parse.parse_qs(self.rfile.read(length).decode())
        name = (form.get("name") or ["user"])[0][:40]
        POSTS[name] = POSTS.get(name, 0) + 1
        sid = secrets.token_hex(16)
        LOGINS[sid] = name
        self.redirect("/", cookie=f"sid={sid}; Path=/; HttpOnly; SameSite=Lax")

    def sid(self):
        for part in (self.headers.get("Cookie") or "").split(";"):
            key, _, value = part.strip().partition("=")
            if key == "sid":
                return value
        return None

    def redirect(self, to, cookie=None):
        self.send_response(303)
        self.send_header("Location", to)
        if cookie:
            self.send_header("Set-Cookie", cookie)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def reply(self, status, content_type, body):
        data = body.encode()
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18766
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
    print(f"sessions fixture on http://127.0.0.1:{port}/")
    server.serve_forever()


if __name__ == "__main__":
    main()
