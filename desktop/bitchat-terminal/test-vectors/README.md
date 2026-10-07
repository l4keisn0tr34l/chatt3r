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

`file-payload-v2.json` specifies only the **inner** canonical type-`0x22`
file TLV for a small octet-stream file. It was independently laid out using
Python standard-library big-endian integers following the pinned Unlicensed
Swift `BitchatFilePacket.swift`, not captured phone traffic or a Swift/Kotlin
runtime cross-check. `cargo test --lib` checks the bounded pure Rust codec.
It has **no** signed outer v2 packet, canonical compression, fragmentation,
filesystem write, BLE exchange or application delivery receipt. Do not use
it as proof that file transfer works with the stock app.
