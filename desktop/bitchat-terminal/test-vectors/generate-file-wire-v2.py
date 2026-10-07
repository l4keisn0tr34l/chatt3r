#!/usr/bin/env python3
"""Synthetic layout fixture, not stock-client output. Run from any directory.

Uses only the Python standard library and cryptography for Ed25519. The
literal seed is public test material; never use it as an app identity.
"""
import json
import pathlib
import zlib

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives import serialization

here = pathlib.Path(__file__).resolve().parent
inner = json.loads((here / "file-payload-v2.json").read_text())
key = Ed25519PrivateKey.from_private_bytes(bytes.fromhex("42" * 32))
public = key.public_key().public_bytes(
    encoding=serialization.Encoding.Raw, format=serialization.PublicFormat.Raw
)


def file_payload(content):
    name = b"note.txt"
    mime = b"application/octet-stream"
    return (b"\x01" + len(name).to_bytes(2, "big") + name
            + b"\x02\x00\x04" + len(content).to_bytes(4, "big")
            + b"\x03" + len(mime).to_bytes(2, "big") + mime
            + b"\x04" + len(content).to_bytes(4, "big") + content)


def frame(payload, compressed):
    if compressed:
        compressor = zlib.compressobj(level=6, wbits=-15)  # raw DEFLATE
        deflate = compressor.compress(payload) + compressor.flush()
        assert zlib.decompress(deflate, wbits=-15) == payload
        wire_payload = len(payload).to_bytes(4, "big") + deflate
    else:
        deflate = None
        wire_payload = payload
    prefix = (bytes([2, 0x22, 0]) + bytes.fromhex("0102030405060708")
              + bytes([4 if compressed else 0]) + len(wire_payload).to_bytes(4, "big")
              + bytes.fromhex("1111111111111111") + wire_payload)
    bucket = next((n for n in (256, 512, 1024, 2048) if len(prefix) + 16 <= n), len(prefix))
    pad = bucket - len(prefix)
    signing = prefix + (bytes([pad]) * pad if 0 < pad <= 255 else b"")
    wire = (prefix[:2] + bytes([7]) + prefix[3:11]
            + bytes([2 | (4 if compressed else 0)]) + prefix[12:] + key.sign(signing))
    return {
        "compressed": compressed,
        "payload_length": len(payload),
        "raw_deflate_hex": deflate.hex() if compressed else None,
        "signing_input_prefix_hex": prefix.hex(),
        "signing_padding_byte": pad if 0 < pad <= 255 else None,
        "signing_padding_count": pad if 0 < pad <= 255 else 0,
        "wire_hex": wire.hex(),
    }


small = bytes.fromhex(inner["payload_hex"])
assert small == file_payload(b"hello from nearby\n")
large = file_payload(b"A" * 400)
fixture = {
    "description": "synthetic signed v2 0x22 public file outer frames, uncompressed and raw-DEFLATE; not stock-client captures",
    "generator": "generate-file-wire-v2.py; Python zlib raw DEFLATE level 6 + cryptography Ed25519; not Swift/Kotlin execution",
    "seed_hex": "42" * 32,
    "public_key_hex": public.hex(),
    "cases": [frame(small, False), frame(large, True)],
}
print(json.dumps(fixture, indent=2))
