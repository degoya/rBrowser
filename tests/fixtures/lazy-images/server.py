#!/usr/bin/env python3
"""The fixture server of RB-099-28: serves this folder on 127.0.0.1:18768 like
`python3 -m http.server`, every file uncached, plus

* /never.svg - never answered: the connection is held open for 300 s, so never.html has an
  image that is still loading when a full-page capture's pre-pass runs out of its deadline.

    python3 server.py [port]
"""

import functools
import http.server
import os
import sys
import time

HOLD_SECONDS = 300


class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path.split("?")[0] == "/never.svg":
            time.sleep(HOLD_SECONDS)
            self.send_error(504)
        else:
            super().do_GET()

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18768
    folder = os.path.dirname(os.path.abspath(__file__))
    handler = functools.partial(Handler, directory=folder)
    with http.server.ThreadingHTTPServer(("127.0.0.1", port), handler) as server:
        print(f"serving {folder} on http://127.0.0.1:{port}/")
        server.serve_forever()


if __name__ == "__main__":
    main()
