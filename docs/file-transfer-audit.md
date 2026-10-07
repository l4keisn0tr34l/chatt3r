# file transfer audit — no new wire protocol yet

source pins and legal restrictions: [upstream reuse audit](upstream-reuse-audit.md). the source analysis and software checkpoints below are followed by **one saved stock-iPhone JPEG on physical BLE**, verified locally by size/hash and full image decode and visually confirmed by the user as the image sent; original-byte/hash comparison is unavailable. the original text-only `chatt3r.rs:Receiver::receive` ignored `0x22` and `baseline/protocol.rs:Reassembler::accept` rejects file fragments; naive file support was impossible. a later first implementation step added `baseline/file_packet.rs`, a **pure, bounded canonical-v2-layout TLV codec**, exposed from `src/lib.rs` and unit-tested with `test-vectors/file-payload-v2.json`. its reader deliberately rejects legacy lengths and multiple content TLVs that upstream can tolerate. it was **not wired into** `chatt3r` in that first checkpoint. the Python CLI is GPL-3.0 reference, not drop-in reusable code.

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
still missing; the later opt-in path below now has one small stock-iPhone
JPEG receive result, without broader stock-app validation.

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
fragment assembly and guarded storage. the later saved-JPEG checkpoint
establishes only that one stock-iPhone receive case; it does not validate
outbound signing or general stock-client interoperability.

## opt-in receive-only path (one small iphone jpeg saved on radio)

`--receive-files <existing-dir>` on **Linux stock phone/direct-LE client only**
opts into a bounded public `0x22` path. file bytes arrive via the existing
btleplug notification or `baseline/linux_att.rs` direct-LE reader →
`chatt3r.rs:Receiver::receive`. the dedicated `baseline/file_fragments.rs`
collects only file-marked `0x20` chunks (max eight concurrent, 256 chunks,
~65 KiB **encoded outer frame** after the first hardware sizing failure,
30s expiry), leaving the proven text
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
in-memory edge cases and synthetic byte comparisons are software-tested;
Windows cross-target checks do not test a radio. **after the earlier failed
image attempts, one 3,485-byte iPhone JPEG was saved on radio and verified
locally**, as recorded below.
this is signed public plaintext, not pairing authentication, persistence
across runs, generic media support or delivery receipts. the stock iOS UI
inspected so far offers image/voice sending, not a proven generic picker.
see [linux-iphone-test.md](linux-iphone-test.md) for the narrow saved-image
evidence and a controlled repeat.

### first image radio observation

the user reported `[iphone] hi` public text, repeated type-`0x01`
166-byte values, many type-`0x20` **182-byte** values rejected with
`truncated packet` or unsupported flags, and later 175/103-byte type-`0x20`
values rejected as unsupported fragmented type. no saved `.bin` was reported.
`182` is the configured direct-LE session's ATT value limit (requested MTU
185), but the old log has **no claimed payload lengths, flags, fragment
indices or original types**; it does not identify which 0x20s carried the
image or establish actual ATT truncation. the pinned Swift source uses
`TransportConfig.bleDefaultFragmentSize=469` for public file fragments and
`BLEOutboundLinkPlanner.plan` does not fragment a `fragment` again. that
creates an MTU-mismatch hypothesis, **not a proven cause for the installed
app**. `protocol.rs:frame_shape` now reports *only* version, flags, declared
and actual frame sizes, and optional fragment index/count/original type for
bounded samples in `--debug`, never peer IDs or content. a repeat should
record a few sanitized `[rx-shape]` + `[drop]` lines and negotiated ATT MTU.
a **second run resolved the ambiguity for several file fragments**:
`version=1 kind=0x20 flags=0x00 declared_payload=482 actual=182
expected=Some(504) fragment=2..5/46 original_type=0x22`.
those values are incomplete signed-file *containers* and cannot be assembled
from the available 182 bytes. the 469-byte Swift default chunk plus a 22-byte
v1 header and 13-byte fragment prefix equals 504, matching the observed
header. this establishes a size mismatch on the tested link, not that the
stock phone supports a larger ATT MTU or that every fragment in the train
is the same. earlier `0x04` flagged fragments and later unsupported original
types remain separate; no saved image was reported.

`linux_att.rs` now requests **517** rather than **185** as the local ATT MTU
*only* for direct-LE sessions opted into `--receive-files`; the reciprocal
MTU reply advertises the same size. the peer's response is range-checked and
negotiated by minimum. `chatt3r.rs` refuses this opt-in session when its
value limit is below the observed **504** bytes (ATT MTU <507); the proven
text default remains 185. tests establish the policy and software packet
path; this was **not yet an image-transfer result**. a subsequent user test
confirmed this same link negotiated **ATT MTU 517/value 514**. it received
complete 504-byte `0x22`-marked fragments, but a 46-part stream hit the old
16 KiB file-assembly cap around part 36. two other `0x20` frames carried
compression flag `0x04` and complete 458/439-byte framing; their expanded
original types were **not** captured, so do not assume they belong to the
file. the phone's short public text still worked; no saved file was reported.

this establishes one **receiver-side** size limit after successful MTU
negotiation, plus an unsupported compression flag whose relationship to the
file has not been established:
`file_fragments.rs` now uses the already-bounded `FileWire` maximum for a
signed outer file (<= ~65 KiB), still 256 parts/8 simultaneous/30s; an image
larger than that remains unsupported. `file_wire.rs:inflate_fragment_payload`
allows <=1024 bytes of exact raw-DEFLATE *fragment* expansion with ratio,
stream-end and full-input checks. `file_fragments.rs:decode_file_candidate`
requires an unaddressed v1/v2 `0x20` frame with only compressed flag `0x04`,
checks lengths, inflates, then checks the **expanded** 13-byte prefix for
original `0x22`; non-file frames still follow the existing text path (and may
be rejected). file fragments remain unsigned until the **complete** outer
v2 file is bound to a validated announcement and signature-checked before
safe storage. synthetic 46/47-part and compressed-fragment tests pass. the
later small saved image exercises the receive path on radio, but its excerpt
does not establish the fragment compression flags or a >16 KiB assembly.
do not alter bonds, guess `--write-limit` (outbound only), or accept incomplete
files.

### first saved iphone jpeg (2026-10-07)

using receiver code at `dff1e31`, after the launcher rebuild and passing Linux unit, strict Clippy, launcher
and terminal PTY checks, the user reported a signed iPhone announcement,
type-`0x20` notification values of 458/328/80/168/504/431 bytes, and one save
of **3,485 bytes**. local read-only checks independently found a 3,485-byte
file with mode **0600**, `image/jpeg`, and a SHA-256 matching the receiver's
printed digest. Pillow verification and full decode passed: **JPEG, 252×448,
RGB**. the user then visually confirmed the saved image matches the image
sent. no original file is saved for byte/hash comparison. no media, local
filename/path, peer ID or raw transcript is retained here.

the exercised data flow is BLE notification →
`chatt3r.rs:Receiver::receive` → `file_fragments.rs:decode_file_candidate` /
`FileFragments::accept` → `Receiver::receive_file` → `FileWire::parse` /
`decode_verified` using the signed announcement's key → bounded file TLV
decode → `IncomingFiles::save` → synced random `.bin` and content hash.
the BitChat envelope, TLVs and fragment layout are upstream references;
the bounded Rust receiver and storage policy are chatt3r work. the save occurs
after signature, metadata, size and MIME checks; it is stronger evidence than
a subscription or successful transport write.

this establishes **one small public JPEG received and stored**, without
proving byte equality with the original photo, repeatability, file sending, private
media, delivery receipts, Swift compression byte identity or general media
compatibility. this run's supplied lines omit negotiated MTU, fragment
flags/count and outer compression, so the earlier 46-part image and bounded
compressed-fragment implementation still need specific radio confirmation.
the earlier **517/value 514** negotiation remains separate evidence.

## answer to the proposed protocol

**Do we need FILE_OFFER / FILE_CHUNK / FILE_ACK at all?** **No for first compatible small-file transfers.** Reuse stock BitChat `0x22` with v2 payload, canonical compression/signing, standard `0x20` fragmentation and validated safe receiving. Keep files small within real peer limits and verify content/size on both ends as a local test. Do not prepend incompatible new offer/chunk/ack packets to stock clients. For **large files, durable end-to-end acceptance, resumable offsets, and transport switching**, the current public envelope and fragment gossip do not solve reliability; evaluate an optional negotiated extension/compatible private receipt behavior separately, only after hardware+version tests and explicit user approval. Transport writes and progress bars alone do not count as delivery.
