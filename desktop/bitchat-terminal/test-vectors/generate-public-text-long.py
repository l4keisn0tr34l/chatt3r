#!/usr/bin/env python3
"""Independent synthetic text fixtures, not stock-phone captures.

Python zlib and cryptography generate raw-DEFLATE/signing inputs independently
of Rust. The literal signing seed is public test material, never an identity.
"""
import json
import zlib

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

key = Ed25519PrivateKey.from_private_bytes(bytes([0x42]) * 32)
public = key.public_key().public_bytes(
    serialization.Encoding.Raw, serialization.PublicFormat.Raw
)


def case(label, payload, version=1, compressed=True):
    width = 2 if version == 1 else 4
    if compressed:
        encoder = zlib.compressobj(level=6, wbits=-15)
        deflate = encoder.compress(payload) + encoder.flush()
        assert zlib.decompress(deflate, wbits=-15) == payload
        wire_payload = len(payload).to_bytes(width, "big") + deflate
    else:
        wire_payload = payload
    prefix = (bytes([version, 2, 0]) + bytes.fromhex("0102030405060708")
              + bytes([4 if compressed else 0])
              + len(wire_payload).to_bytes(width, "big") + bytes([0x11]) * 8
              + wire_payload)
    bucket = next((n for n in (256, 512, 1024, 2048) if len(prefix) + 16 <= n), len(prefix))
    pad = bucket - len(prefix)
    preimage = prefix + (bytes([pad]) * pad if 0 < pad <= 255 else b"")
    signature = key.sign(preimage)
    wire = prefix[:2] + b"\x07" + prefix[3:11] + bytes([2 | (4 if compressed else 0)]) + prefix[12:] + signature
    return dict(label=label, version=version, compressed=compressed,
                decoded_hex=payload.hex(), signing_hex=preimage.hex(), wire_hex=wire.hex())


# A valid UTF-8 payload with 243 distinct byte values exercises the Swift
# byte-diversity gate without relying on one compressor's entropy estimate.
diverse = ("".join(chr(i) for i in range(128))
           + "".join(chr(i << 6) for i in range(2, 32))
           + "".join(chr(0x100 + i) for i in range(64))
           + "".join(chr(max(0x800, i << 12)) for i in range(16))
           + "".join(chr(max(0x10000, i << 18)) for i in range(5))).encode()
diverse += b"a" * (1024 - len(diverse))
assert len(set(diverse)) >= 231 and len(diverse) == 1024

cases = [case("unchanged-99", b"a" * 99, compressed=False),
         case("threshold-100", b"a" * 100),
         case("text-256", b"hello nearby " * 19 + b"123456789"),
         case("unicode-1024-v1", "🙂".encode() * 256),
         case("unicode-1024-v2", "🙂".encode() * 256, version=2),
         case("diverse-1024", diverse, compressed=False)]
assert len(bytes.fromhex(cases[2]["decoded_hex"])) == 256
print(json.dumps(dict(description="synthetic signed public text; not Swift byte-identity or radio evidence",
                     seed_hex="42" * 32, public_key_hex=public.hex(), cases=cases), indent=2))
