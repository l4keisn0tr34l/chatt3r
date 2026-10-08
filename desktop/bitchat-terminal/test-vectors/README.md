# BitChat baseline vectors

`public-text-v1.json` specifies exact on-wire bytes and the expected decoded
fields of a short signed v1 public message. The example seed is public test
material, never an application secret.

The vector was independently generated with Python `struct` and
`cryptography` Ed25519, manually following the pinned Swift framing/signing
rules. It is **not captured iPhone traffic or an executed Swift cross-check**.

To construct the signing input: hex-decode `signing_input_prefix_hex`, then
append `signing_padding_count` copies of `signing_padding_byte`. The input has
TTL zero, no signature flag/signature, and totals 256 bytes. The actual wire
packet has TTL 7 and a signature, with no trailing padding.

`cargo test --bin chatt3r` verifies this fixture against the Rust codec.
Future Swift/Kotlin tests can consume the same JSON. The timestamp is an
artificial layout fixture and intentionally fails live freshness checks.

`public-text-long.json` adds independent Python zlib/Ed25519 cases for
99/100/256/1,024-byte text, v1/v2, Unicode and high-diversity uncompressed
text. `generate-public-text-long.py` prints the deterministic fixture; its
seed is public test material. `cargo test --bin chatt3r` checks received
preimages/signatures, verified payloads and fragmented local round trips.
these are not Swift captures or proof that Apple recompresses to the same
bytes as Rust. stock-phone and native Windows radio tests are pending in
`docs/long-text-checkpoint.md`.

`file-payload-v2.json` specifies only the **inner** canonical type-`0x22`
file TLV for a small octet-stream file. It was independently laid out using
Python standard-library big-endian integers following the pinned Unlicensed
Swift `BitchatFilePacket.swift`, not captured phone traffic or a Swift/Kotlin
runtime cross-check. `cargo test --lib` checks the bounded pure Rust codec.
It has **no** signed outer v2 packet, canonical compression, fragmentation,
filesystem write, BLE exchange or application delivery receipt. Do not use
it as proof that file transfer works with the stock app.

`file-wire-v2.json` has two *synthetic* signed outer v2 `0x22` frames:
one with the uncompressed TLV above, one with a 450-byte TLV compressed
using Python zlib **raw DEFLATE** (level 6), a big-endian original-length
prefix and the compressed flag. Both sign a TTL-zero, signature-free,
default-padded frame with public test seed `42` repeated 32 times. Rebuild
with `python3 test-vectors/generate-file-wire-v2.py` (requires Python's
`cryptography` package); `cargo test --lib` checks exact preimages and
signatures in Rust. This verifies an independently generated layout, **not**
byte identity with Apple's Compression framework or an executed Swift/
Android cross-check. The pure `file_wire` module authenticates these *wire*
bytes against a caller-supplied key but does not decompress, admit a sender,
or pass files into the BLE chat.
