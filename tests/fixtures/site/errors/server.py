#!/usr/bin/env python3
"""The fixture server of RB-080-09: serves tests/fixtures/site on 127.0.0.1 like
`python3 -m http.server`, plus /errors/boom.js answered with 500.

    python3 server.py [port]                       # http, default port 18766
    python3 server.py [port] --tls cert.pem key.pem

With --tls the page is https, so errors/index.html's http script is mixed content (Chromium blocks
it). The certificate is the tester's own (e.g. mkcert for 127.0.0.1); none is checked in.
"""

import functools
import http.server
import os
import ssl
import sys

SITE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path.split("?")[0] == "/errors/boom.js":
            body = b"// error-report fixture: 500\n"
            self.send_response(500)
            self.send_header("Content-Type", "text/javascript")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            super().do_GET()


def main(argv):
    port = int(argv[1]) if len(argv) > 1 and argv[1].isdigit() else 18766
    server = http.server.ThreadingHTTPServer(
        ("127.0.0.1", port), functools.partial(Handler, directory=SITE)
    )
    if "--tls" in argv:
        at = argv.index("--tls")
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(argv[at + 1], argv[at + 2])
        server.socket = context.wrap_socket(server.socket, server_side=True)
    scheme = "https" if "--tls" in argv else "http"
    print(f"serving {SITE} on {scheme}://127.0.0.1:{port}/errors/", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main(sys.argv)
