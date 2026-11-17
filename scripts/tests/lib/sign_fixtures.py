"""Fixtures for scripts/tests/sign-windows.sh (RB-090-03): files to sign and a local timestamp server.

    python3 -I sign_fixtures.py pe <out> [--signed]   # a minimal valid PE32+ image (x64)
    python3 -I sign_fixtures.py msi <out>             # a minimal compound file with an MSI's CLSID
    python3 -I sign_fixtures.py tsa <dir> <portfile>  # an RFC 3161 server on 127.0.0.1

The PE has one section and no imports - enough for Authenticode, never run. `--signed` fills the
security directory with a stand-in, as a vendor-signed file (Node, Microsoft's DLLs) carries one.
The timestamp server answers each request with `openssl ts -reply`, signed by <dir>/tsa.pem and
<dir>/tsa.key under <dir>/ts.cnf, and writes the port it bound to <portfile>. Nothing here is a
secret: the certificates the test makes live in a temporary directory and die with it.
"""

import http.server
import os
import struct
import subprocess
import sys
import tempfile

FILE_ALIGN = 0x200
SECURITY_DIR = 4


def pe_image(signed: bool) -> bytes:
    dos = b"MZ" + bytes(0x3A) + struct.pack("<I", 0x40)
    coff = struct.pack("<HHIIIHH", 0x8664, 1, 0, 0, 0, 0xF0, 0x0022)
    dirs = [(0, 0)] * 16
    if signed:
        dirs[SECURITY_DIR] = (2 * FILE_ALIGN, 16)
    optional = struct.pack(
        "<HBBIIIII", 0x20B, 14, 0, FILE_ALIGN, 0, 0, 0x1000, 0x1000
    ) + struct.pack(
        "<QIIHHHHHHIIIIHHQQQQII",
        0x140000000, 0x1000, FILE_ALIGN, 6, 0, 0, 0, 6, 0, 0,
        0x2000, FILE_ALIGN, 0, 3, 0x8160,
        0x100000, 0x1000, 0x100000, 0x1000, 0, 16,
    ) + b"".join(struct.pack("<II", *entry) for entry in dirs)
    section = b".text\0\0\0" + struct.pack(
        "<IIIIIIHHI", FILE_ALIGN, 0x1000, FILE_ALIGN, FILE_ALIGN, 0, 0, 0, 0, 0x60000020
    )
    headers = dos + b"PE\0\0" + coff + optional + section
    image = headers + bytes(FILE_ALIGN - len(headers)) + b"\xc3" + bytes(FILE_ALIGN - 1)
    if signed:
        image += struct.pack("<IHH", 16, 0x0200, 0x0002) + bytes(8)
    return image


def msi_image() -> bytes:
    """A compound file (v3, 512-byte sectors) with an empty root storage of the MSI CLSID."""
    end, free, fat_sect, no_stream = 0xFFFFFFFE, 0xFFFFFFFF, 0xFFFFFFFD, 0xFFFFFFFF
    header = bytes.fromhex("d0cf11e0a1b11ae1") + bytes(16)
    header += struct.pack("<HHHHH", 0x3E, 3, 0xFFFE, 9, 6) + bytes(6)
    header += struct.pack("<IIIIIIIII", 0, 1, 1, 0, 4096, end, 0, end, 0)
    header += struct.pack("<I", 0) + struct.pack("<I", free) * 108
    fat = struct.pack("<II", fat_sect, end) + struct.pack("<I", free) * 126
    name = "Root Entry".encode("utf-16-le") + b"\0\0"
    msi_clsid = bytes.fromhex("8410 0c00 0000 0000 c000 0000 0000 0046".replace(" ", ""))
    root = name + bytes(64 - len(name)) + struct.pack("<HBB", len(name), 5, 1)
    root += struct.pack("<III", no_stream, no_stream, no_stream) + msi_clsid
    root += bytes(4 + 16) + struct.pack("<IQ", end, 0)
    empty = bytes(64) + struct.pack("<HBB", 0, 0, 0) + struct.pack("<III", no_stream, no_stream, no_stream)
    empty += bytes(16 + 4 + 16) + struct.pack("<IQ", 0, 0)
    return header + fat + root + empty * 3


def serve_tsa(folder: str, portfile: str) -> None:
    class Handler(http.server.BaseHTTPRequestHandler):
        def do_POST(self) -> None:  # noqa: N802 - the name http.server calls
            query = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            with tempfile.TemporaryDirectory() as scratch:
                request = os.path.join(scratch, "request.tsq")
                reply = os.path.join(scratch, "reply.tsr")
                with open(request, "wb") as handle:
                    handle.write(query)
                done = subprocess.run(
                    ["openssl", "ts", "-reply", "-config", os.path.join(folder, "ts.cnf"),
                     "-queryfile", request, "-signer", os.path.join(folder, "tsa.pem"),
                     "-inkey", os.path.join(folder, "tsa.key"), "-out", reply],
                    capture_output=True, check=False,
                )
                if done.returncode != 0:
                    self.send_error(500)
                    return
                with open(reply, "rb") as handle:
                    body = handle.read()
            self.send_response(200)
            self.send_header("Content-Type", "application/timestamp-reply")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_args: object) -> None:
            pass

    server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
    with open(portfile + ".tmp", "w", encoding="ascii") as handle:
        handle.write(str(server.server_address[1]))
    os.replace(portfile + ".tmp", portfile)
    server.serve_forever()


def main() -> None:
    command, args = sys.argv[1], sys.argv[2:]
    if command == "pe":
        with open(args[0], "wb") as handle:
            handle.write(pe_image("--signed" in args[1:]))
    elif command == "msi":
        with open(args[0], "wb") as handle:
            handle.write(msi_image())
    elif command == "tsa":
        serve_tsa(args[0], args[1])
    else:
        sys.exit(f"unknown fixture {command}")


if __name__ == "__main__":
    main()
