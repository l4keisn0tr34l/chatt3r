# Upstream reconnaissance and baseline status

## Source pins

Analysis performed against the existing clean checkouts (not a fresh remote fetch):

| Repository | Commit | Role / license |
| --- | --- | --- |
| `permissionlesstech/bitchat` | `5e9287fae1e5fea80ca741d4ea669829dc16f144` | Swift reference; `LICENSE` is Unlicense |
| `ShilohEye/bitchat-terminal` | `cabb6ae8f640d605b4f08673a6aba8b9b4be12d2` | Desktop development input; README says Public Domain, no standalone LICENSE |
| `permissionlesstech/bitchat-android` | `1b8a8252d89770ab07ab11a3d72e682a9311adf7` | Independent reference; `LICENSE.md` is GPL-3.0; no code copied |

Initial desktop changes were isolated on `offline-link/linux-iphone-baseline`.
Implementation commit: `cdbf33169286d4004217941fdf058ef4fa545835`,
`add the first chatt3r text client`. The project is now packaged as a single
`chatt3r` repository on `main`, with Rust source under `desktop/bitchat-terminal/`
and that upstream/implementation history retained as ancestors. The original
upstream source pin above remains the comparison base. Optional reference
checkouts under `upstream/` are ignored and not shipped. Do not update those
references silently; record new pins when re-analyzing.
No btleplug fork is needed. Cargo resolves `btleplug 0.11.8`; commit the
application lockfile for reproducible subsequent builds.

## Local observations

- Ubuntu kernel `6.8.0-142-generic`, x86_64.
- Rust exists in `~/.cargo/bin`, but that directory was absent from this shell's PATH.
- `rustc 1.97.1`, `cargo 1.97.1`; D-Bus development library `1.14.10` and C compiler available.
- `bluetoothctl list/show` finds a powered `hci0` supporting central and peripheral roles.
- Unmodified desktop `cargo build` **passes**. No root build, pairing changes,
  Bluetooth restart, or system permission changes were necessary.
- Added the isolated `src/bin/chatt3r.rs` / `src/baseline/protocol.rs` harness;
  legacy code changes are limited to fixing an existing test's misspelled
  `FLAG_HAS_CHANNEL` reference to `MSG_FLAG_HAS_CHANNEL`.
- Current `cargo test --offline --locked` passes **25 tests** (4 legacy, 21 harness); 2 interactive harness tests are ignored and covered by the PTY script.
  Two terminal-only tests are skipped by the default unit run and exercised
  separately in four PTY cases by `tests/ui-smoke.py`.
  `cargo build --offline --locked --bins`, harness Clippy with `-D warnings`,
  targeted rustfmt check, and `git diff --check` pass.
- Tests cover exact header bytes, an independently generated Ed25519 fixture,
  padded signing input, mutable TTL, signature tampering, announcement identity,
  configurable fragmentation, duplicates/conflicts, assembly caps/expiry,
  truncation/random hostile bytes, terminal control escaping, stable peer colors,
  `NO_COLOR`, editable-draft preservation across cancelled readline futures,
  Ctrl-C/Ctrl-D exit, and terminal mode restoration.
- Interactive input uses `rustyline-async 0.4.9` (Unlicense, reviewed registry
  source/examples), not a home-grown raw-mode editor. Colors/log suppression
  affect presentation only; the BLE/protocol paths are unchanged.
- A 12-second normal-user `chatt3r --scan-only` run successfully started/stopped
  discovery but saw no BitChat advertisements. Exact output is in the test guide.
- The user has now **confirmed bidirectional Linux/iPhone text**. The installed
  iPhone app version may differ from the pinned Swift source. Repeated exchanges
  with Wi-Fi/cellular disabled have not been recorded.
- Windows has neither been built nor tested. Linux/laptop symmetric BLE is
  not implemented by the upstream terminal's central-only transport.

## BLE discovery and packet path

Swift reference paths below are relative to `upstream/bitchat/`:

- `bitchat/Services/BLE/BLEService.swift:185-191`: production service
  `F47B5E2D-4A9E-4C5A-9B3F-8E1D2C3A4B5C`; testnet ends `4B5A`.
- Characteristic: `A1B2C3D4-E5F6-4A5B-8C9D-0E1F2A3B4C5D`.
- `BLEService+LinkLayerPeripheralRole.swift:36-46`: one characteristic with
  notify, write, write-without-response, read; readable/writeable permissions.
- `BLERadioController.swift:60-90`: advertise service UUID; scan for that UUID.
- `BLEService+LinkLayerCentralRole.swift`: connect, discover service/characteristic,
  subscribe to notifications. Swift supports both roles.
- In the first topology Linux is central and iPhone is peripheral: Linux writes
  to the characteristic; iPhone sends notifications back through the subscription.
  Linux does not have to advertise for this topology.
- Swift peripheral write ingress: `BLEInboundWriteBuffer.swift` ->
  `BLEService+LinkLayerPeripheralRole.swift:258-320` -> decoded packet ingress ->
  `BLEReceivePipeline.swift` -> handlers in `BLEService.swift`.
- Public text: `BLEAnnounceHandler.swift` establishes discovered signing keys;
  `BLEPublicMessageHandler.swift:75-126` requires a valid signature and interprets
  payload as plain UTF-8. Use the local Bluetooth/mesh public room, not geohash/DM.
- Desktop legacy `src/main.rs:348-426` scans all services, picks the first match,
  connects/subscribes, sends a legacy key exchange and nickname-only announcement.
  Notifications enter `parse_bitchat_packet` around line 1728.

## Current wire format (not the terminal's legacy format)

The context's `bitchat/Protocols/BitchatProtocol.swift` is now a high-level
reference. Actual framing has moved to:

- `localPackages/BitFoundation/Sources/BitFoundation/BinaryProtocol.swift`
- `.../BitchatPacket.swift`, `MessageType.swift`, `PeerID.swift`, `MessagePadding.swift`
- Announcement TLVs: `bitchat/Protocols/Packets.swift:6-147`

All multi-byte fields are big-endian:

| Offset | Field |
| --- | --- |
| 0 | version u8: 1 or 2 |
| 1 | type u8 |
| 2 | TTL u8 (default 7) |
| 3..10 | timestamp u64, Unix milliseconds |
| 11 | flags: recipient 1, signature 2, compressed 4, route 8 (v2), RSR 16 |
| 12..13 / 12..15 | payload-only length u16 (v1) / u32 (v2) |
| 14 / 16 | raw sender ID, 8 bytes |
| next | optional raw recipient ID, 8 bytes; all FF means broadcast |
| next | optional v2 route: u8 count + 8-byte hops |
| next | payload; compression includes original-size preamble u16/u32 |
| next | optional Ed25519 signature, 64 bytes |
| trailing | optional padding |

Sender ID is SHA-256(Noise static X25519 public key), first **8 raw bytes**
(`PeerID.swift:132-133`); its display is 16 hex characters. There is no outer
message ID. `BLEReceivePipeline.swift` deduplicates on sender, timestamp, type,
and a payload digest. `MeshMessageIdentity.swift` derives timeline IDs.

Current relevant types: announce `01`, public message `02`, leave `03`, Noise
handshake `10`, Noise encrypted `11`, fragment `20`, sync request `21`, file `22`.
Unknown types must not be interpreted as legacy messages.

Announcements contain u8 type/u8 length TLVs: nickname `01`, Noise public key
`02` (32 bytes), Ed25519 public key `03` (32 bytes), optional neighbors `04`,
capabilities `05`, bridge cell `06`. `BLEAnnounceHandlingPolicy.swift` checks
sender/key derivation, freshness, signature, and signing-key continuity.

Signing (`BitchatPacket.toBinaryDataForSigning`, `NoiseEncryptionService.swift:661`):
re-encode without signature, TTL fixed to zero, RSR cleared, with automatic
compression and **default padding**. Padding buckets are 256/512/1024/2048,
chosen to fit unsigned length + 16; pad bytes equal pad length; skip padding if
more than 255 bytes would be needed. Actual public frames can be unpadded;
the signature input is still padded. This detail must have a regression test.

## Why unmodified desktop cannot interoperate with current iOS

`desktop/bitchat-terminal/src/main.rs` assumes July 2025 semantics:

- `02` is key exchange, `04` is text, `05/06/07` are fragment phases.
- Announces are just nickname bytes, not identity TLVs.
- Peer IDs are 8 ASCII hex characters from 4 random bytes, not key-derived raw IDs.
- Text uses `BitchatMessage`'s old structured binary payload, not UTF-8 content.
- Old X25519/HKDF/AES-GCM exchanges are not current Noise sessions.
- Parser's `HEADER_SIZE = 13` is off by one (actual v1 is 14), enabling malformed
  slice bounds; signatures are skipped rather than verified.
- Fragment sizing assumes fixed limits and startup writes bypass fragmentation.

Do not advertise legacy private messaging as compatible or secure. Keep legacy
source intact initially; a separate minimal `chatt3r` binary in the same package can test
current signed public text without rewriting its TUI, persistence, or crypto.

## Fragmentation / limits

- `BLEOutboundFragmentPlanner.swift`: split the **whole encoded original packet**,
  not just its payload. Outer type `20`; payload = ID[8], index u16, total u16,
  original type u8, chunk. Carry original sender/recipient/timestamp/TTL.
- `BLEFragmentAssemblyBuffer.swift`: key by sender + fragment ID, bounded
  assemblies, expiration, byte caps. `BLEFragmentHandler.swift` decodes the
  rebuilt original and checks its type/sender before normal delivery.
- Frame v1 length is u16; v2 enables larger payloads. Per-type caps live in
  `PacketPayloadLimits.swift`; there is no single safe global allocation size.
- `btleplug 0.11.8` exposes writes/notifications but no portable negotiated MTU
  accessor. The initial harness must use an explicit configurable characteristic
  value limit and label it as an operator-provided limit, **not measured MTU**.
  Reassess after hardware results; do not assume every link accepts 500 bytes.
- Apple `CompressionUtil.swift` uses COMPRESSION_ZLIB; legacy Rust uses LZ4.
  Initial short-text harness can explicitly reject compressed frames and keep
  outbound payloads below the **100-byte** compression threshold (`Constants.swift`,
  despite stale 256-byte comments in BinaryProtocol). Longer text and
  compressed signature canonicalization remain follow-up work.

## Encryption and media reconnaissance (not implemented yet)

- Public announcements/text are signed plaintext: no confidentiality.
- Current private traffic uses `Noise/NoiseProtocol.swift`, `NoiseSession.swift`,
  `Services/NoiseEncryptionService.swift`, `BLE/BLENoisePacketHandler.swift`;
  authenticated peer-state binds signing keys inside the Noise session.
- `BitchatProtocol.swift` defines inner private-file `20`; private media is Noise
  encrypted before fragmenting the outer `noiseEncrypted` packet. See
  `BLEPrivateMediaSessionStore.swift`, `BLEService.swift:2079` and
  `ChatMediaTransferCoordinator.swift`. A future desktop Noise implementation
  could reuse this path; the terminal's old encryption cannot.
- `Protocols/BitchatFilePacket.swift` already has generic filename, size, MIME,
  and binary content TLVs, including v2 4-byte size/content lengths.
- `Services/BLE/MimeType.swift` now includes PDF and application/octet-stream;
  `BLEFileTransferPolicy.swift` applies MIME, magic-byte, and size checks.
  `Views/ContentComposerView.swift:259-379` offers photo-library/camera input and
  microphone/voice recording. `ChatMediaPreparation.swift` prepares JPEG images
  and audio/mp4 voice notes. No general `fileImporter` or importing
  `UIDocumentPicker` was found in the checked iOS UI; the document picker in
  `ImagePreviewView.swift` is for export only. Therefore PDF/octet-stream
  envelope support does **not** prove arbitrary-file sending from stock iPhone.
  chatt3r currently ignores file type `22` and rejects fragments claiming media
  types; it cannot receive/save phone attachments yet. Inspect the installed
  app version before deciding whether an iOS fork is required.
- `FileTransferLimits.swift` caps any file payload at **1 MiB**, images and voice
  notes at **512 KiB**. Gossip fragment recovery and private media receipts exist. No claim yet that these constitute a
  resumable, SHA-256-verified arbitrary-file protocol.

## Reusable desktop portions and platform questions

Protocol/compression/fragmentation, encryption, persistence, and most terminal
rendering are Rust-only; `main.rs` intertwines BLE and application/UI state.
No raw BlueZ calls or Windows-target cfgs were found in upstream desktop source.
`btleplug` selects the platform backend, but its central-only role is inadequate
for desktop-to-desktop links without a separate GATT-server/advertiser.
Compilation on a Windows target and physical Windows BLE tests remain pending.

## Next gate

Build, unit tests, scan, and user-reported bidirectional text are complete.
Next complete the repeated offline checks in `docs/linux-iphone-test.md` with
stock BitChat on iPhone before claiming a reproducible reliable offline baseline.

### Physical connection follow-up

After the user opened stock BitChat, a scan found its production service. The
user's first connection attempt failed with `br-connection-busy`
(`org.bluez.Error.InProgress`). Added up to three bounded connection attempts
with per-peer disconnect cleanup on transient errors/timeouts and on final
failure/cancellation; no adapter restart, sudo, or pairing changes.

Two subsequent agent probes passed connection; the instrumented probe discovered
15 GATT characteristics and found the expected read/write/notify characteristic,
but `subscribe` timed out after 10 seconds. The probe sent no text and disconnected
on exit. Next check: unlocked iPhone with a freshly reopened foreground BitChat
Bluetooth room, then retry interactively. Root cause of subscription timeout is
not established. These probes did not verify bidirectional text.

The user's subsequent interactive run subscribed successfully, received verified
`iphone` announcements, and the user confirmed both text directions worked.
This is user-reported evidence, not an agent-observed phone screen or a complete
10-message offline repeat test. Wi-Fi/cellular state and installed versions are
still unknown. No encryption/Windows/file support is inferred from this result.

### Reconnect follow-up and launcher

A later reconnect again failed notification subscription on the first attempt;
its second fresh attempt subscribed and received verified `iphone` announcements.
The user then reported chat working again. Added bounded startup retries across
connection, GATT discovery, and notification subscription (three attempts total,
not nested retries). Only the chosen peer is disconnected between attempts;
permissions/unsupported characteristic properties are not retried. Once chat
starts, messages are never automatically replayed and live disconnects still
require restarting the client.

A subsequent launcher probe found a cached iPhone service UUID but enumerated
14 characteristics without the BitChat characteristic. Missing app service is
now treated as a bounded retryable startup condition, not proof that the app is
advertising/ready. This illustrates a discovery limitation: BlueZ's device UUID
list can include cached services. The automatic recovery path is unit-tested,
but recovery from these failures with the new helper is not yet physically
confirmed; do not claim the latest probe established chat.

A later failure report reproduced a 15-second scan with no BitChat service at
all; the adapter was powered, while the known iPhone profile was paired but
not connected and had no BitChat UUID. A prior connection attempt had selected
a cached UUID and reached GATT without the BitChat characteristic. We now
require a post-start radio event before selecting an address, prefer fresh
addresses over cached profiles, and skip an address with a missing app
characteristic rather than repeating setup on it. These changes passed 25 unit
tests, launcher/PTY smoke tests, and Clippy. The rebuilt binary's 10-second
scan still found no live peer, so phone-side advertising and successful
connection remain unverified. No adapter reset or pairing change was made.

`scripts/chatt3r` resolves paths relative to itself, supplies the operator's
128-byte frame limit, keeps the terminal attached, and builds offline only when
needed or explicitly requested. `--doctor` only reads adapter state. Launcher
smoke tests use fake tools, paths with spaces, and forwarded arguments/stdin;
no Bluetooth settings are touched. [docs/reconnect.md](reconnect.md) contains the
phone-side manual steps and the command cheat sheet.

### Paired dual-mode iphone: later root cause and linux recovery

The user identified the installed iphone app as app store v1.7.1. The matching
upstream tag `v1.7.1` advertises the same mainnet UUID as this client. A
consented one-time HCI monitor captured its connectable LE advertisement on
a resolvable private address, with the correct UUID bytes. BlueZ mapped it to
the paired public iphone identity but omitted BitChat from that profile's D-Bus
UUID list. A targeted `Device1.Connect()` returned `br-connection-canceled`;
bluetoothd logged hands-free/audio profile failures. Keeping an LE scan active
and pre-establishing an LE socket did not fix BlueZ's bearer choice.

A direct, read-only `gatttool -t public` probe found the BitChat GATT service
and notify/write characteristic **without unpairing**. A native Linux L2CAP
ATT socket then established an LE link and exchanged MTU; it required binding
the local ATT CID before connecting. The first prototype mistook a simultaneous
incoming ATT write from the dual-role phone for its own discovery response and
closed the socket. The native module now handles that request separately.
Afterwards, a physical run negotiated MTU 185, verified/subscribed to BitChat,
sent an announcement and received multiple verified signed `android` peer
announcements. `/peers` listed it and `/quit` closed the link, while the pairing
remained intact. A later user terminal log shows a signed `[android] yo works
right` reply and successful Linux GATT writes; the user confirmed `android`
is their iphone nickname. Linux → iphone **display** still needs explicit user
confirmation before claiming bidirectional text on the new backend. The raw HCI/strace files were deleted.
See [docs/direct-le.md](direct-le.md) for the architecture checkpoint and
limitations. The default btleplug central path stays unchanged; Linux-only
`--direct-le <address>` explicitly selects the new backend.
