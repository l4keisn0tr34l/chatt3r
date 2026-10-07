# file transfer audit — no new wire protocol yet

source pins and legal restrictions: [upstream reuse audit](upstream-reuse-audit.md). this is source analysis, **not** a successful chatt3r file-radio test. the original text-only `chatt3r.rs:Receiver::receive` ignored `0x22` and `baseline/protocol.rs:Reassembler::accept` rejects file fragments; naive file support was impossible. a later first implementation step added `baseline/file_packet.rs`, a **pure, bounded canonical-v2-layout TLV codec**, exposed from `src/lib.rs` and unit-tested with `test-vectors/file-payload-v2.json`. its reader deliberately rejects legacy lengths and multiple content TLVs that upstream can tolerate. it was **not wired into** `chatt3r` in that first checkpoint. the Python CLI is GPL-3.0 reference, not drop-in reusable code.

## existing BitChat public file envelope

- `localPackages/BitFoundation/Sources/BitFoundation/MessageType.swift` maps **outer `0x22`** to `fileTransfer`; `bitchat/Services/BLE/BLEService.swift:1503` sends a *signed* version-2 broadcast packet containing `BitchatFilePacket.encode()` TLVs. Header includes sender ID, ms timestamp, TTL, payload length u32, signature flag and 64-byte Ed25519 signature (see `BinaryProtocol.swift`, `BitchatPacket.swift`). File TLV is **not a separate series of app-level chunks**.
- `bitchat/Protocols/BitchatFilePacket.swift:15-152`: tag `01`: optional UTF-8 filename (tag u8 + u16 big-endian length); `02`: declared file size (tag + u16 length **4** + u32 BE); `03`: optional UTF-8 MIME (u16 length); `04`: full content (tag + **u32 BE length** + bytes). Encoder stores the whole file in one payload; decoder accepts old 8-byte size and old u16 content length when canonical 4-byte length is impossible, skips unknown tags and can append several content tags. It does not itself guarantee declared size equals content length or include a content SHA-256 field. Reject missing/empty content in Swift. `bitchat-android/.../model/BitchatFilePacket.kt` encodes the same current canonical fields, although its top comment still describes obsolete 8-byte/u16 lengths; **trust code, not comment**. Android's decoder requires filename and its file-size value uses signed `Int` converted to `Long`; interoperability should be tested with small files and explicit size comparison.
- `localPackages/BitFoundation/Sources/BitFoundation/{BinaryProtocol,CompressionUtil,PacketPayloadLimits}.swift`: zlib-compresses eligible/reducible payloads (threshold 100 bytes), includes original length in the compressed payload, enforces per-type decompressed/frame caps. Signatures use **canonical re-encoding** (TTL zero, RSR cleared, default padding, compression if eligible); verification requires reproducing those bytes, not just signing an uncompressed raw file. Public file is **signed but plaintext**. `protocol.rs:Packet::sign` rejects payload >99 bytes, so chatt3r cannot send compatible files today. The Python CLI's `protocol.py:BitchatPacket.encode` explicitly *never compresses on send*, including in its `data_for_signing`; inspect/fixture-test highly compressible files against stock Swift before trusting the CLI as a reference for signed large payloads.
- `BLEOutboundFragmentPlanner.swift`, `BLEFragmentAssemblyBuffer.swift`, `BLEFragmentHandler.swift`: BLE splits the **complete encoded outer packet** into type-`0x20` unsigned fragment packets (`8-byte random ID | u16 index | u16 total | original type byte | chunk`). The receiver groups by sender+fragment ID, bounds allocations by claimed type, rebuilds the outer packet, decodes/decompresses it and verifies the *original* signature before accepting the file (`BLEFileTransferHandler.swift:handle`). Fragment frames are not individual signed files or independent resumable chunks. Android `mesh/FragmentManager.kt` caps incoming fragment sets at **256** and **1 MiB** per set, 64 active / 4 MiB global (`util/AppConstants.kt:39-45`); Swift allows more generally but explicitly restricts private-media v1 to 256 for Android interop (`BLEOutboundFragmentPlanner.swift`). Effective BLE value limit must be measured/capped per session; chatt3r's 128 is operator-selected, not a media throughput guarantee.
- Swift `FileTransferLimits.swift`: generic payload <=**1 MiB**, images/voice <=**512 KiB**. Android `MediaSendingManager.kt:MAX_FILE_SIZE` references `AppConstants.Media.MAX_FILE_SIZE_BYTES` (roughly 9.87 MiB) but Android's fragment assembly <=1 MiB and 256 fragments: UI admission **does not prove** end-to-end 10 MiB delivery. Even a 1 MiB content payload gains TLV/packet overhead and can exceed a 1 MiB reassembly cap; the safe interoperable maximum is smaller and must be measured. Python CLI `media.py` caps at 1 MiB. Use the strictest peer/frame/fragment bound, verify metadata/content and test particular app versions.

## one complete *existing-code* file path, and its limits

An **Android public small file** (not a chatt3r end-to-end test): `ui/media/FilePickerButton.kt` opens SAF `*/*`, `features/file/FileUtils.kt:copyFileForSending` copies the URI locally → `ui/ChatViewModel.kt:sendFileNote` → `ui/MediaSendingManager.kt:sendFileNoteAsync` checks disk size, MIME, reads whole `File.readBytes` → `model/BitchatFilePacket.kt:encode` → `mesh/UnifiedMeshService.kt:sendFileBroadcast` chooses BLE if enabled, otherwise Wi-Fi Aware → BLE `mesh/BluetoothMeshService.kt:sendFileBroadcast` wraps version-2 `FILE_TRANSFER`, signs it, then `FragmentingPacketSender`/`FragmentManager` build type-`0x20` frames → Android `BluetoothConnectionManager` GATT or Wi-Fi `WifiAwareMeshService` length-prefixed socket → receiver's fragment manager/packet processor → `mesh/MessageHandler.kt:handleBroadcastMessage` verifies peer admission, decodes file TLV → `features/file/FileUtils.kt:saveIncomingFile` persists it, then media UI/`FileViewerDialog.kt` offers saving/opening. For an **iOS receiver** instead: `BLEReceivePipeline.swift` → `BLEFragmentHandler.swift` → `BLEService.swift:handleFileTransfer` → `BLEFileTransferHandler.swift:handle` verifies signature before `BLEIncomingFileValidator` MIME/magic/size and `BLEIncomingFileStore.swift:save` sanitized unique atomic file and UI event. This traces *source paths*, not demonstrated Android→iPhone or chatt3r file compatibility; sender-generated packets still need cross-implementation fixture and physical tests.

A second desktop *reference* path exists in GPL Python: `bitchat_cli/ble_service.py:send_file` `Path.read_bytes` → `media.py:BitchatFilePacket.encode` → signed v2 `0x22` → `_broadcast` → `_fragment` → Bleak GATT writes → `_dispatch`/`FragmentReassembler` → `_handle_file` → `~/.bitchat/downloads`. That receive path **does not verify file signatures** and auto-opens untrusted files; don't copy its security behavior. chatt3r cannot complete this path today.

## private vs public, progress, failure and receipts

- Private files are **not simply raw outer `0x22`**: `bitchat/Protocols/BitchatProtocol.swift:NoisePayloadType.privateFile=0x20` holds the entire file TLV inside authenticated/encrypted Noise; the outer packet is `noiseEncrypted` (`0x11`) and is then fragmented. `BLEFileTransferHandler.swift:handlePrivatePayload` validates after Noise decrypt. Chatt3r has no Noise handshake/session; **don't** promise compatible private files by implementing only `0x22`. Android `mesh/PrivateMediaTransfer.kt` has its own negotiated private-media preparation and receipt rules.
- Progress: sender `BLEOutbound*` / Android `FragmentingPacketSender.kt` report queued/sent fragment counts via `TransferProgressManager`. That is **not remote saved-file confirmation**. Timeouts, bounded assembly expiry, cancellation and failure paths exist. Swift `BLEFragmentAssemblyBuffer.stalledBroadcastFragmentIDs` plus gossip `REQUEST_SYNC` can request missing **broadcast** fragment streams; this is opportunistic mesh recovery, not a universal per-chunk ACK or guaranteed file transfer. Directed fragments are excluded from that sync.
- **Private** media has a different, more sophisticated receipt story: Swift `BLEFileTransferHandler` saves before issuing stable-media ACK, uses durable `BLEPrivateMediaReceiptStore` and deduplicates accepted IDs; `ChatMediaTransferCoordinator.swift:~1256-1467` retains limited payloads for post-reconnect whole-file retry *only under a receipt/capability policy*. It is not generic resumable byte-offset transfer or proof of public-file end-to-end ACK. `BitchatFilePacket.swift:PrivateMediaMessageIdentity` derives a stable receipt ID only for eligible entropy-bearing iOS-generated image/voice filenames; Android/old iOS names may be ineligible. Never replay an ambiguous public write in chatt3r merely because these private-media features exist.
- Public receive has MIME allow-list/magic check and storage quota (`BLEFileTransferPolicy.swift`, `BLEIncomingFileStore.swift`), but generic octet-stream accepts bytes and filename must still be sanitized. Python auto-open and basename handling are not adequate security requirements. Current iOS composer `Views/ContentComposerView.swift` and `ViewModels/ChatMediaPreparation.swift` expose image and voice sending, **no generic import picker found** in pinned source; iOS receiving generic PDF/octet-stream is supported by `MimeType.swift`, not proof that stock installed iPhone can *send* arbitrary files. Android has a generic picker.

## first implementation checkpoint (software only)

independent synthetic v2 TLV fixture → `src/lib.rs: file_packet` →
`baseline/file_packet.rs:FilePayload::decode` checks strict tag/length,
UTF-8, size agreement, duplicate tags and a 64 KiB **parser** budget →
`FilePayload::encode` reproduces the fixture. tests include truncation,
unknown optional tags, oversized and random bytes. the wire layout and
`0x22` designation come from the pinned **Unlicensed Swift** source; the
Rust safety bounds and strict decoder are chatt3r-specific. no GPL source
was copied. **there is no BLE file path or disk output**; 64 KiB is not a
verified interoperable radio size (at the configured 128-byte frame cap,
Android's 256-fragment ceiling may be reached much earlier). at this earlier
checkpoint, outer signatures, compression and per-type fragment recovery were
still missing; the later opt-in candidate below is not stock-app-validated.

## outer-frame fixture checkpoint (software only)

`test-vectors/generate-file-wire-v2.py` independently builds signed v2
broadcasts over both a short uncompressed file TLV and a Python zlib level-6
**raw DEFLATE** version of a repetitive 450-byte TLV. It writes fixture
preimages and frames to `file-wire-v2.json`. `src/lib.rs: file_wire` parses
bounded `0x22` header/length/flags → rebuilds unsigned TTL-zero signing
bytes over the original wire payload and default padding → verifies Ed25519
with a **caller-supplied** public key. at that fixture checkpoint, only
uncompressed payload passed to `file_packet::decode`; compressed bytes were
not yet decompressed. the later opt-in path below adds bounded inflation. the
identity key must come from an independently validated announcement. even a valid
signature alone does not authenticate the physical device or prove delivery.

this tests Python-generated bytes against Rust (including tamper/length
rejection), **not Swift or Android output**. in particular, Apple canonical
signing re-encodes and may recompress differently from Python/zlib; matching
the signature over the received compressed *wire* bytes does not establish
Apple accepts an outbound packet or that a stock peer emitted it. that earlier
checkpoint changed no BLE code or disk files. the receive-only first-radio
candidate below adds bounded expansion, signed peer admission, file-only
fragment assembly and guarded storage; real stock-client testing is still
needed before claiming interoperability.

## opt-in receive-only first-radio candidate (software-tested, no phone file test)

`--receive-files <existing-dir>` on **Linux stock phone/direct-LE client only**
opts into a bounded public `0x22` path. file bytes arrive via the existing
btleplug notification or `baseline/linux_att.rs` direct-LE reader →
`chatt3r.rs:Receiver::receive`. the dedicated `baseline/file_fragments.rs`
collects only file-marked `0x20` chunks (max eight concurrent, 256 chunks,
16 KiB **encoded outer frame**, 30s expiry), leaving the proven text
`protocol.rs:Reassembler` unchanged. `file_wire.rs:FileWire` rejects wrong
flags/lengths, binds the outer sender/timestamp to the fragment metadata,
requires a recently verified peer announcement and checks the Ed25519
signature **before** raw-DEFLATE expansion. `flate2`'s pure Rust backend
inflates to an exact <= ~65 KiB TLV, requiring stream end, exact declared
length, no overflow and no trailing compressed bytes; `file_packet.rs`
checks TLV size agreement, single content tag and <=64 KiB content.
`file_store.rs:IncomingFiles` requires an explicitly chosen existing
non-symlink directory; it accepts a short JPEG/PNG/GIF/WebP only with matching
magic or `application/octet-stream`, creates a random `.bin` with no wire
filename, no auto-open, 0600 on Unix, and allows at most 16 saves per run.
all in-memory results and byte/hash checks are software-tested; Windows
cross-target checks do not test a radio. **no iPhone file was received yet.**
this is signed public plaintext, not pairing authentication, persistence
across runs, generic media support or delivery receipts. the stock iOS UI
inspected so far offers image/voice sending, not a proven generic picker.
see [linux-iphone-test.md](linux-iphone-test.md) for the pending physical test.

## answer to the proposed protocol

**Do we need FILE_OFFER / FILE_CHUNK / FILE_ACK at all?** **No for first compatible small-file transfers.** Reuse stock BitChat `0x22` with v2 payload, canonical compression/signing, standard `0x20` fragmentation and validated safe receiving. Keep files small within real peer limits and verify content/size on both ends as a local test. Do not prepend incompatible new offer/chunk/ack packets to stock clients. For **large files, durable end-to-end acceptance, resumable offsets, and transport switching**, the current public envelope and fragment gossip do not solve reliability; evaluate an optional negotiated extension/compatible private receipt behavior separately, only after hardware+version tests and explicit user approval. Transport writes and progress bars alone do not count as delivery.
