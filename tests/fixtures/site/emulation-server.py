#!/usr/bin/env python3
"""The fixture server of RB-010-05: serves this folder on 127.0.0.1:18765 like
`python3 -m http.server`, plus

* /echo-headers - the request headers as JSON (names lower-case), so emulation.html shows the
  Accept-Language and the custom headers the view sent;
* /large.bin - 1 MiB of zeros, never cached, for the network throttling check.

    python3 emulation-server.py [port]
"""

import functools
import http.server
import json
import os
import sys

LARGE = b"\0" * (1024 * 1024)


class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/echo-headers":
            body = json.dumps({name.lower(): value for name, value in self.headers.items()})
            self.reply("application/json", body.encode())
        elif self.path == "/large.bin":
            self.reply("application/octet-stream", LARGE)
        else:
            super().do_GET()

    def reply(self, content_type, body):
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18765
    folder = os.path.dirname(os.path.abspath(__file__))
    handler = functools.partial(Handler, directory=folder)
    with http.server.ThreadingHTTPServer(("127.0.0.1", port), handler) as server:
        print(f"serving {folder} on http://127.0.0.1:{port}/")
        server.serve_forever()


if __name__ == "__main__":
    main()
