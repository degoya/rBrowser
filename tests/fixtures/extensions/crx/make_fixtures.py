#!/usr/bin/env python3
"""Writes the CRX3 fixtures of `crates/rbrowser/src/extensions/crx3` (RB-080-23).

    python3 -I tests/fixtures/extensions/crx/make_fixtures.py

Needs the `cryptography` package. Every run makes fresh keys, so the id in `fixtures.json`
changes with the files; the tests read both. No private key is kept.

The format (Chromium `components/crx_file/crx3.proto`): "Cr24", version 3, the header's
length, the header (a `CrxFileHeader` protobuf: repeated `sha256_with_rsa` = 2, repeated
`sha256_with_ecdsa` = 3, each an `AsymmetricKeyProof { public_key = 1, signature = 2 }`, and
`signed_header_data` = 10000, a `SignedData { crx_id = 1 }`), then the ZIP. Each proof signs
"CRX3 SignedData\\x00" + the little-endian length of `signed_header_data` + `signed_header_data`
+ the ZIP.
"""

import base64
import hashlib
import io
import json
import os
import sys
import zipfile

from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, padding, rsa

HERE = os.path.dirname(os.path.abspath(__file__))
MAGIC = b"Cr24"
PREFIX = b"CRX3 SignedData\x00"


def varint(value):
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def field(number, payload):
    """A length-delimited protobuf field."""
    return varint((number << 3) | 2) + varint(len(payload)) + payload


def spki(key):
    return key.public_key().public_bytes(
        serialization.Encoding.DER, serialization.PublicFormat.SubjectPublicKeyInfo
    )


def crx_id_bytes(public_der):
    return hashlib.sha256(public_der).digest()[:16]


def crx_id_text(public_der):
    return "".join(chr(ord("a") + (b >> 4)) + chr(ord("a") + (b & 0x0F)) for b in crx_id_bytes(public_der))


def make_zip(files):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in files:
            archive.writestr(name, data)
    return buffer.getvalue()


def build(zip_bytes, crx_id, rsa_keys=(), ec_keys=()):
    signed_header = field(1, crx_id)
    message = PREFIX + len(signed_header).to_bytes(4, "little") + signed_header + zip_bytes
    header = b""
    for key in rsa_keys:
        signature = key.sign(message, padding.PKCS1v15(), hashes.SHA256())
        header += field(2, field(1, spki(key)) + field(2, signature))
    for key in ec_keys:
        signature = key.sign(message, ec.ECDSA(hashes.SHA256()))
        header += field(3, field(1, spki(key)) + field(2, signature))
    header += field(10000, signed_header)
    return MAGIC + (3).to_bytes(4, "little") + len(header).to_bytes(4, "little") + header + zip_bytes


def manifest(version, manifest_version=3):
    return json.dumps(
        {
            "manifest_version": manifest_version,
            "name": "rBrowser fixture: hello",
            "version": version,
            "content_scripts": [{"matches": ["http://*/*"], "js": ["content.js"]}],
        }
    ).encode()


CONTENT = b"document.documentElement.setAttribute('data-rbrowser-extension', 'hello');\n"


def write(name, data):
    with open(os.path.join(HERE, name), "wb") as handle:
        handle.write(data)


def main():
    developer = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    other = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    publisher = ec.generate_private_key(ec.SECP256R1())
    dev_der = spki(developer)
    crx_id = crx_id_bytes(dev_der)

    files_v1 = [("manifest.json", manifest("1.0.0")), ("content.js", CONTENT)]
    files_v2 = [("manifest.json", manifest("2.0.0")), ("content.js", CONTENT)]
    valid = build(make_zip(files_v1), crx_id, [developer], [publisher])
    write("valid.crx", valid)
    write("valid-2.crx", build(make_zip(files_v2), crx_id, [developer], [publisher]))
    # The ZIP's last byte flipped: the proofs no longer match.
    write("bad-signature.crx", valid[:-1] + bytes([valid[-1] ^ 0xFF]))
    # Another extension altogether: signed by another developer key, which is the declared id.
    write("wrong-id.crx", build(make_zip(files_v1), crx_id_bytes(spki(other)), [other], [publisher]))
    # The declared id is the developer's, but no proof carries that key.
    write("no-developer.crx", build(make_zip(files_v1), crx_id, [other], [publisher]))
    write("truncated.crx", valid[: len(valid) // 2])
    write("no-publisher.crx", build(make_zip(files_v1), crx_id, [developer], []))
    write("zip-slip.crx", build(make_zip(files_v1 + [("../evil.js", CONTENT)]), crx_id, [developer], [publisher]))
    write("mv2.crx", build(make_zip([("manifest.json", manifest("1.0.0", 2)), ("content.js", CONTENT)]), crx_id, [developer], [publisher]))
    write(
        "fixtures.json",
        json.dumps(
            {
                "about": "Made by make_fixtures.py; read by crates/rbrowser/src/extensions/crx3 tests.",
                "id": crx_id_text(dev_der),
                "otherId": crx_id_text(spki(other)),
                "publisherKeyHash": hashlib.sha256(spki(publisher)).hexdigest(),
                "developerKey": base64.b64encode(dev_der).decode(),
                "publisherKey": base64.b64encode(spki(publisher)).decode(),
                "version": "1.0.0",
                "updateVersion": "2.0.0",
            },
            indent=2,
        ).encode()
        + b"\n",
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
