# longer public text: iphone hardware checkpoint

status (2026-10-08): `chatt3r` can encode, sign, fragment, verify and display
public text up to **1,024 UTF-8 bytes** in software tests. after the
reference-zlib correction in `8699b17`, the user confirmed **100-, 256- and
1,024-byte numbered ASCII messages in both directions between Linux and
stock iPhone BitChat**, with a supporting Linux transcript. short text also
worked. the earlier outbound 100-byte failure is retained below as history.
native Windows longer text and physical reconnect remain unverified. stop
at this completed phone text checkpoint; broader discovery/roles are separate.

## source and data flow

the reference is Unlicensed Swift BitFoundation at
`5e9287fae1e5fea80ca741d4ea669829dc16f144`, inspected locally:
`BinaryProtocol.swift`, `BitchatPacket.swift`, `CompressionUtil.swift`,
`Constants.swift` and `MessagePadding.swift`. these supply the wire format,
100-byte compression threshold, byte-diversity gate, raw-DEFLATE length
prefix, TTL-zero signature input and default padding rules. the bounded
Rust implementation and receiver tests are chatt3r work; no GPL/AGPL code
was copied.

outgoing chat:

```text
terminal UTF-8 line (<=1,024 bytes)
  → chatt3r.rs:chat
  → protocol.rs:Packet::sign
  → text_payload.rs:compress_text (only public message type 0x02)
  → TTL-zero, signature-free, default-padded preimage → Ed25519
  → protocol.rs:frames (whole signed packet → unsigned 0x20 fragments)
  → chatt3r.rs:send → existing LinkWriter
```

`baseline/text_payload.rs`, exported by `src/lib.rs`, uses `flate2` with its
reference-zlib backend at level 5. it compresses only when the source
has at least 100 bytes, passes Swift's diversity gate and the raw compressed
bytes are smaller. the original length prefix is u16 for v1, u32 for v2.
messages below 100 bytes retain their old uncompressed wire/signing bytes;
the original independent short-text fixture still passes. announcements and
leave packets keep the previous <=99-byte uncompressed signing subset.

incoming chat:

```text
existing notification → Receiver::receive
  → Packet::decode / Reassembler::accept
  → sender/recipient/type/timestamp checks and announced signing key
  → Packet::verified_payload: verify exact wire preimage first
  → bounded raw-DEFLATE expansion, if compressed
  → UTF-8 check → decoded-content duplicate check → existing terminal output
```

signed text is not decompressed until its signature passes. expansion needs
the exact declared size, stream end and full input consumption; overflow,
truncation and trailing compressed bytes are rejected. decoded text is
capped at 1,024 bytes and encoded text assemblies at **1,379 bytes**, including
the bounded possible header/recipient/signature/padding overhead. the
existing collector still permits at most 16 pending assemblies, 256 parts
and 30-second expiry.

unsigned compressed `0x20` frames can be expanded only within a 1,024-byte
fragment-payload bound, with an expanded public-text marker `0x02`. their
contents remain untrusted until the complete outer message is verified.
media-marked fragments stay in the existing opt-in file path, with its own
collector, signature, MIME and storage checks. integration tests exercise
both text receivers with file receive disabled/enabled and a mixture of
plain and compressed text fragments.

the launcher still supplies a configured **128-byte value limit**; this is
not a measured MTU. text-only direct LE retains **MTU 185** and file-opted
direct LE retains **517**. startup, discovery and transport roles were not
changed. messages are not automatically replayed after an ambiguous failure.

## evidence and limits

### successful phone retest (2026-10-08)

the user ran the existing known-phone direct-LE launcher in text-only mode
and explicitly reported that all three sizes worked both ways. the Linux
transcript records signed peer discovery, short incoming text, the outgoing
samples, and authenticated/decompressed incoming samples at the expected
decoded byte counts. outbound receipt is established by the user's phone
observation; a successful BLE write alone would not establish it.

| decoded UTF-8 bytes | Linux → iphone compressed wire payload | outgoing frames at configured limit 128 | iphone → Linux decoded bytes | physical result |
| ---: | ---: | ---: | ---: | --- |
| 100 | 48 | 2 | 100 | user-confirmed both ways |
| 256 | 90 | 2 | 256 | user-confirmed both ways |
| 1,024 | 325 | 5 | 1,024 | user-confirmed both ways |

all three outer messages were compressed. incoming 100/256-byte messages
arrived as type `0x02` values of 134/176 bytes. the incoming 1,024-byte
message was assembled after three type `0x20` notifications of 150, 72 and
132 bytes; individual fragment compression flags were not recorded.
negotiated ATT MTU was **185**, value limit **182**; configured outgoing
limit stayed **128**, which is a separate operator limit.

startup first reported a missing phone service and retried after five
seconds, then connected in the same process. this confirms one startup
wait-to-ready transition, not recovery after an established link disconnects.
the transcript ends with a LEAVE submission; it does not establish resource
cleanup or a returning-peer reconnection. app/iOS version and Wi-Fi/cellular
switch states were not recorded for this run. no OS pairing change was shown.

this establishes compressed numbered-ASCII interoperability on this phone
link, including fragmented 1,024-byte delivery. Unicode, high-diversity
uncompressed long text, repeated reliability, native Windows longer text,
and file-mode text under the new backend remain separate physical tests.
the four earlier JPEG saves remain evidence; this text-only run did not
receive a file. raw device addresses, peer IDs and full transcripts are not
included in this record.

### failed first phone checkpoint

the user reported the failed outbound 100-byte message on 2026-10-08. no
exact payload or transmit metadata was retained, so the phone's rejection
reason is unknown. short text still worked in the same connection.

an isolated comparison using the pre-correction built Rust compressor reproduced
a canonical-signature incompatibility. Apple's documented
[COMPRESSION_ZLIB configuration](https://developer.apple.com/documentation/compression/compression_zlib)
is reference zlib level 5, raw DEFLATE, window bits -15, memory level 8 and
default strategy. pinned Swift `BitchatPacket.toBinaryDataForSigning`
re-encodes the decoded payload, and `NoiseEncryptionService.verifyPacketSignature`
verifies that representation. Python reference zlib with those settings
produces different bytes from the previous miniz level-6 encoder:

| synthetic text | decoded bytes | miniz bytes | reference zlib bytes | signature verifies after reference re-encoding |
| --- | ---: | ---: | ---: | --- |
| repeated `a` | 100 | 16 | 6 | no |
| numbered sample from the generator below | 100 | 46 | 46 | no |
| numbered sample | 256 | 88 | 88 | no |
| numbered sample | 1,024 | 322 | 323 | no |

both streams decode to identical text. these are synthetic comparisons with
public test keys, not Apple execution or phone logs. they demonstrate a
defect in the outgoing canonical representation and likely explain the radio
failure. changing miniz to level 5 also produces different bytes for both
100-byte samples, so a level-only adjustment is insufficient. the previous
round-trip tests did not catch it: they verified incoming
Python fixtures and locally generated Rust packets separately, without
requiring outgoing bytes to match the canonical encoder.

the correction uses reference zlib at level 5 through the existing `flate2`
crate's `zlib-default` feature. the lock adds `libz-sys` and its `vcpkg`
build helper, removes the miniz backend, and leaves other package versions
unchanged. `libz-sys` uses installed reference zlib on Linux where available
or builds its bundled C source; Windows builds the C source with the existing
C++ build tools unless an installed zlib is selected. no new runtime process
or Python requirement is introduced.

the independent generator now uses Apple's documented settings and includes
the numbered 100/256/1,024-byte samples. the protocol test requires **every
outgoing preimage and signed packet to equal the independent fixture bytes**,
in addition to incoming verification and fragmented round trips. the earlier
miniz output fails this requirement. short-text fixtures still pass.
the shared `flate2` decoder also uses reference zlib; existing malformed
stream and file-receiver tests pass. file packet format, bounds, storage,
opt-in behavior, BLE roles and MTUs are unchanged.

59 Linux tests pass (10 library, 49 binary; three additional interactive tests
are covered by six PTY smoke cases). strict Linux Clippy, launcher smoke,
terminal PTY smoke, the Windows target check and strict Windows Clippy pass.
the launcher rebuilt `--bin chatt3r`. Windows checks establish types, not
native PC radio behavior.

`test-vectors/public-text-long.json` is independently generated by Python
zlib and cryptography. cases cover 99, 100, 256 and 1,024 bytes, v1/v2,
Unicode and an uncompressed high-diversity payload. tests check exact
received preimages/signatures, configurable fragments, duplicate delivery,
mutable TTL, invalid signatures, invalid UTF-8, hostile lengths and bounded
inflation. the original short-text and file tests also pass. these are
synthetic fixtures, not captured stock-phone output.

Apple verifies a canonical re-encoding that may recompress the payload.
the corrected outgoing representation matches independent reference-zlib
fixtures at Apple's documented settings. the phone retest above establishes
physical interoperability for the three numbered-ASCII samples, without
claiming byte identity for every payload or installed app version. inbound
verification still uses the received representation without recompressing
untrusted input.
public text remains plaintext with ephemeral signing identities, no Noise
authentication or application delivery receipts. file sending remains absent.

## phone checkpoint procedure (completed for numbered ascii)

with the user's ready iPhone and explicit consent, use the existing known
phone link and keep BitChat unlocked in its Bluetooth/mesh public chat:

```bash
./scripts/chatt3r --build
./scripts/chatt3r --name laptop --debug
```

the user's local saved-phone setting selects the existing direct-LE path.
do not change that setting, reset the adapter or remove bonds. the binary is
already rebuilt for this checkpoint; the build command above also makes
later sessions load the current code. leave file mode off for the first text
check so it uses the proven text MTU 185.

1. exchange a short numbered message both ways and verify both screens.
2. exchange harmless **100-byte, 256-byte and 1,024-byte** texts one at a
   time, both ways. compare the full text on both screens. this crosses the
   previous 99-byte limit and exercises compression/fragmentation as needed.
3. optionally repeat with 256 copies of `🙂` (1,024 UTF-8 bytes).
4. retain direction, byte count, observed arrival, and a few redacted
   `[tx]`, `[rx-text]`, `[rx-shape]` or `[drop]` metadata lines. include no
   addresses, peer IDs, raw packet bytes, private text or full transcripts.

generate printable ASCII samples in a separate terminal; copy only the body
line after each size label:

```bash
python3 - <<'PY'
body = ''.join(f'{i:04d}-' for i in range(205))
for size in (100, 256, 1024):
    print(f'{size} UTF-8 bytes; copy the next line:')
    print(body[:size])
PY
```

if a short regression fails or longer text is rejected, stop and report the
first failing direction/size. do not raise bounds or replay an ambiguous
message automatically. a GATT write alone is not receipt. diagnose the
protocol result before further changes.

the remaining PC checkpoint requires the Windows PC to pull/build/run natively using its
existing `--host` and Linux's existing `--desktop-peer` commands in
[windows.md](windows.md), then exchange the same sizes both ways. no PC radio
test has been performed in this software session. zero-argument connectivity
is explicitly deferred to a separate audit.
