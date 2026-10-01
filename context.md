# Project Context: Cross-Platform Offline BLE Messaging + File Transfer

## 0. Purpose of this document

This file is intended to be dropped directly into a coding-agent
workspace (Codex, Claude Code, etc.) as the primary project context.

The project is an **offline, cross-platform, infrastructure-free
messaging and file-transfer system** inspired by BitChat.

The most important requirement is:

> Two nearby devices should be able to exchange text and files even when
> there is no Internet connection, no Wi-Fi network, no Wi-Fi hotspot,
> no cellular data, and no cable. Bluetooth Low Energy (BLE) is the
> initial transport.

Target platforms:

-   iOS / iPhone
-   Linux laptops, especially Ubuntu
-   Windows laptops
-   Later: Android and macOS if useful

The project should NOT unnecessarily reinvent components that already
exist in open-source BitChat implementations. The preferred strategy is
to study, reuse, adapt, or port existing protocol/networking code where
licensing permits, while keeping our project modular enough to diverge
where necessary.

------------------------------------------------------------------------

# 1. Core user scenario

A user is in an environment such as a university computer lab where:

-   the laptop has no Internet access,
-   Wi-Fi may be unavailable or intentionally disabled,
-   a hotspot may not work,
-   USB/cables are unavailable or inconvenient,
-   Bluetooth is available.

The user has useful information on an iPhone and wants to move it to a
Linux or Windows laptop.

Examples:

-   send a URL,
-   send copied text,
-   send code,
-   send notes,
-   send a small image,
-   send a PDF,
-   send a source-code archive,
-   send arbitrary files.

The system should also support laptop-to-laptop communication:

-   Linux \<-\> Linux
-   Windows \<-\> Windows
-   Windows \<-\> Linux
-   iPhone \<-\> Linux
-   iPhone \<-\> Windows

No server should be necessary for local operation.

------------------------------------------------------------------------

# 2. Non-negotiable constraints

## 2.1 Offline operation

The core application MUST NOT require:

-   Internet access,
-   a cloud server,
-   an account,
-   a phone number,
-   DNS,
-   a Wi-Fi access point,
-   a Wi-Fi hotspot,
-   cellular service.

BLE should be sufficient for V1.

Optional Internet transports may be considered much later, but they must
never become a dependency of local operation.

## 2.2 Cross-platform protocol

The application protocol must not depend on a platform-specific
file-sharing feature such as:

-   AirDrop,
-   Windows Bluetooth File Transfer,
-   Nearby Share / Quick Share,
-   platform-specific OBEX assumptions.

Instead, all clients implement the same application-layer protocol.

Conceptually:

    iPhone --------\
                    \
    Linux ---------- BLE protocol ---- messages/files
                    /
    Windows -------/

## 2.3 Direct communication first

V1 should prioritize reliable direct communication:

    Device A <---- BLE ----> Device B

Do NOT begin by implementing a sophisticated mesh.

A successful first milestone is literally:

    iPhone -> "hello world" -> Linux laptop

followed by:

    Linux -> "hello back" -> iPhone

Then make that reliable before adding files or relays.

------------------------------------------------------------------------

# 3. Existing open-source work to leverage

Research performed before beginning this project found several highly
relevant repositories.

## 3.1 Official BitChat repository

Repository:

https://github.com/permissionlesstech/bitchat

This is the most important reference implementation.

As of the research date, the project implements a decentralized
peer-to-peer messenger with BLE mesh support. The repository is
primarily Swift for iOS/macOS.

Important existing concepts include:

-   BLE peer discovery
-   BLE connections
-   binary packet protocol
-   fragmentation
-   message deduplication
-   TTL-based relay
-   mesh routing
-   Noise-based private-message encryption
-   media transfer
-   store-and-forward functionality
-   background BLE behavior

The repository is released under the Unlicense/public-domain style
licensing according to its repository documentation. Verify the current
LICENSE before copying code.

Particularly important file:

https://github.com/permissionlesstech/bitchat/blob/main/bitchat/Protocols/BitchatProtocol.swift

`BitchatProtocol.swift` defines application-layer packet structures and
encoding/decoding behavior. Study this before designing a new packet
format.

Do NOT casually create a completely incompatible protocol before
determining whether using or extending BitChat's protocol would save
substantial work.

The official project has also added image/audio media over BLE and has
had explicit work on reliability for larger BLE transfers. Those
implementations should be inspected when implementing arbitrary file
transfer.

## 3.2 BitChat Android

Repository organization:

https://github.com/permissionlesstech

Relevant repository:

`permissionlesstech/bitchat-android`

This provides another implementation of the protocol in Kotlin and is
valuable because comparing the Swift and Kotlin implementations helps
distinguish:

-   protocol behavior that is genuinely cross-platform,
-   iOS-specific implementation details,
-   Android-specific implementation details.

Even if Android is not a V1 target, this repository is useful as a
second protocol implementation.

## 3.3 bitchat-terminal

Repository:

https://github.com/ShilohEye/bitchat-terminal

This may be the most useful starting point for the laptop client.

It is a Rust terminal client intended to communicate over the BitChat
BLE protocol.

The repository states that:

-   it works offline over BLE,
-   it is written in Rust,
-   it uses `btleplug`,
-   Linux is supported,
-   it is intended to interoperate with BitChat clients,
-   Windows has not been thoroughly tested and may require changes.

Instead of building a Linux BLE client from zero, START by cloning and
testing this repository.

Questions to answer experimentally:

1.  Can `bitchat-terminal` discover an iPhone running BitChat?
2.  Can it receive public messages?
3.  Can it send messages that the iPhone receives?
4.  Which portions are Linux-specific?
5.  Does it compile on Windows?
6.  If Windows compilation fails, which modules fail?
7.  Is its protocol implementation current relative to official BitChat?
8.  Can its BLE transport layer be separated from its terminal UI?
9.  Can its protocol code be extracted into a reusable Rust crate?
10. Can arbitrary file-transfer packet types be added without breaking
    BitChat compatibility?

## 3.4 btleplug

Repository:

https://github.com/deviceplug/btleplug

`btleplug` is a Rust asynchronous cross-platform Bluetooth Low Energy
library.

It currently provides host/central-side BLE support across major
desktop/mobile platforms, including Linux and Windows.

Important architectural warning:

`btleplug` is primarily a **central/client-side** BLE library. It does
not by itself solve every peripheral/GATT-server requirement.

This matters because a symmetric BitChat-style architecture may require
devices to advertise and/or expose GATT services, not merely scan and
connect.

Therefore do NOT assume:

    "btleplug is cross-platform"
        =>
    "all BLE roles required by our protocol are automatically cross-platform"

Investigate peripheral/GATT-server options separately where needed.

Possible references mentioned by btleplug include peripheral-oriented
Rust projects such as `bluster` and `ble-peripheral-rust`, but their
current platform support must be verified before adoption.

------------------------------------------------------------------------

# 4. Recommended strategic direction

There are two possible approaches.

## Approach A: Extend BitChat compatibility

Use the existing BitChat protocol as much as possible.

Advantages:

-   iOS application already exists.
-   Existing BLE discovery and messaging behavior.
-   Existing fragmentation.
-   Existing mesh concepts.
-   Existing encryption.
-   Existing Rust terminal implementation.
-   Existing Android implementation.
-   Less protocol design from scratch.
-   Potential interoperability with normal BitChat clients.

Disadvantages:

-   BitChat protocol may evolve.
-   Desktop implementation may lag behind official clients.
-   Arbitrary file transfer may require protocol extensions.
-   Some BitChat features are unnecessary for our use case.
-   Full compatibility can constrain design decisions.

## Approach B: New protocol inspired by BitChat

Reuse BLE techniques and code patterns while defining a new protocol.

Advantages:

-   complete control,
-   easier arbitrary file transfer,
-   simpler protocol,
-   can optimize specifically for desktop \<-\> phone transfer.

Disadvantages:

-   must implement iOS app ourselves,
-   must implement protocol ourselves,
-   must solve discovery, fragmentation, encryption, routing,
    deduplication, etc.,
-   duplicates substantial existing work.

## Current recommendation

START WITH APPROACH A.

Do not fork everything immediately.

First prove that:

    official BitChat iPhone
             <->
    bitchat-terminal on Linux

actually communicates.

Then modify/fork the desktop side.

Only introduce protocol extensions where the existing protocol cannot
satisfy the project's goals.

A likely long-term architecture is:

    BitChat-compatible core protocol
               +
    our optional capability negotiation
               +
    arbitrary file-transfer extension

This preserves compatibility while allowing our clients to expose
additional functionality.

------------------------------------------------------------------------

# 5. Proposed repository architecture

A possible monorepo:

    offline-link/
    |
    +-- README.md
    +-- CONTEXT.md
    +-- LICENSE
    |
    +-- protocol/
    |   +-- SPEC.md
    |   +-- packet-types.md
    |   +-- file-transfer.md
    |   +-- compatibility.md
    |   +-- test-vectors/
    |
    +-- desktop/
    |   +-- Cargo.toml
    |   +-- crates/
    |       +-- protocol/
    |       +-- transport/
    |       +-- transfer/
    |       +-- crypto/
    |       +-- cli/
    |
    +-- ios/
    |   +-- ...
    |
    +-- tests/
    |   +-- interoperability/
    |   +-- packet-vectors/
    |
    +-- docs/
        +-- architecture.md
        +-- ble-notes.md
        +-- threat-model.md

However, if forking `bitchat-terminal`, preserve its structure initially
rather than performing a giant refactor before anything works.

Rule:

> Working interoperability first. Beautiful architecture second.

------------------------------------------------------------------------

# 6. Layered architecture

Keep these concerns separate.

## Layer 1: BLE transport

Responsibilities:

-   Bluetooth adapter detection
-   permissions
-   scanning
-   advertising if supported/required
-   GATT discovery
-   connecting
-   disconnecting
-   characteristic reads/writes
-   notifications
-   MTU awareness
-   reconnect logic
-   platform-specific BLE behavior

This layer should know as little as possible about chat messages or
files.

Ideal abstraction:

    trait Transport {
        discover()
        connect(peer)
        send(bytes)
        receive() -> bytes
        disconnect()
    }

The exact Rust API can differ, but preserve the conceptual separation.

## Layer 2: framing / protocol

Responsibilities:

-   packet header
-   protocol version
-   packet type
-   message ID
-   sender/peer identity
-   payload length
-   fragmentation metadata
-   TTL if relaying
-   checksums/authentication metadata where appropriate

Input:

    application payload

Output:

    sequence of transport-sized frames

## Layer 3: reliability

BLE characteristic writes are not equivalent to a reliable file-transfer
protocol.

Implement application-level concepts where necessary:

-   transfer ID
-   chunk number
-   total chunks or byte offsets
-   acknowledgements
-   retry
-   timeout
-   duplicate handling
-   resume
-   cancellation

For chat messages, reliability can remain lightweight.

For files, correctness matters substantially more.

## Layer 4: application objects

Examples:

    TextMessage
    FileOffer
    FileAccept
    FileReject
    FileChunk
    FileAck
    FileComplete
    FileCancel
    PeerAnnouncement

## Layer 5: UI

Initially:

-   CLI/TUI on Linux/Windows.

Later:

-   desktop GUI if desired,
-   custom iOS UI if we stop relying on the official BitChat client.

Do NOT tightly couple BLE code to terminal rendering.

------------------------------------------------------------------------

# 7. Proposed file-transfer protocol

This is a starting design, NOT a final standard.

## 7.1 Capability negotiation

When peers connect, exchange capabilities.

Example conceptual object:

    HELLO {
        protocol_version
        client_name
        platform
        capabilities: [
            TEXT,
            FILE_TRANSFER_V1,
            RESUME,
            ENCRYPTION,
            RELAY
        ]
        max_chunk_size
    }

If both devices advertise `FILE_TRANSFER_V1`, enable file-transfer UI.

This allows standard BitChat peers to continue functioning without
understanding our extension.

## 7.2 File offer

Sender transmits metadata before bytes.

Conceptually:

    FILE_OFFER {
        transfer_id
        filename
        mime_type
        total_size
        sha256
        proposed_chunk_size
    }

Receiver may answer:

    FILE_ACCEPT

or:

    FILE_REJECT

Possible rejection reasons:

-   user rejected,
-   insufficient storage,
-   unsupported file size,
-   unsupported protocol,
-   duplicate transfer.

## 7.3 Chunking

Example:

    FILE_CHUNK {
        transfer_id
        chunk_index
        offset
        data
    }

Do NOT hard-code a huge BLE payload.

Determine effective payload from negotiated/observed MTU and protocol
overhead.

The transfer layer should split file data accordingly.

## 7.4 Acknowledgements

Simple V1:

    send chunk
    receive ACK
    send next chunk

This will be slow but extremely easy to debug.

After correctness is proven, optimize with a sliding window:

    chunks 100..115 sent
    receiver ACKs range / bitmap
    retransmit missing chunks

Do not optimize prematurely.

## 7.5 Integrity

Sender computes SHA-256 before transfer.

Receiver computes SHA-256 after reconstruction.

If hashes match:

    FILE_COMPLETE_OK

Otherwise:

    FILE_COMPLETE_HASH_MISMATCH

Never silently report success if integrity verification fails.

## 7.6 Resume

V1 can omit resume.

V2 should allow:

    FILE_RESUME_REQUEST {
        transfer_id
        next_missing_offset
    }

or a bitmap/range representation for missing chunks.

Persist enough transfer metadata to survive a temporary BLE disconnect.

------------------------------------------------------------------------

# 8. BLE-specific concerns

BLE is not a byte-stream socket.

Do not mentally model it as TCP.

Important constraints include:

-   GATT services and characteristics
-   central/peripheral roles
-   advertisement limits
-   characteristic notification behavior
-   characteristic write modes
-   negotiated MTU
-   OS scheduling
-   connection intervals
-   iOS background restrictions
-   Windows BLE API differences
-   Linux BlueZ behavior
-   adapter/driver differences

The protocol must tolerate:

-   disconnects,
-   duplicated application packets,
-   delayed packets,
-   varying MTUs,
-   peers disappearing,
-   different OS BLE behavior.

------------------------------------------------------------------------

# 9. Linux implementation

Likely stack:

-   Rust
-   Tokio
-   btleplug
-   BlueZ underneath on Linux

Start with `bitchat-terminal`.

Do NOT initially write raw BlueZ D-Bus code unless `btleplug` cannot
provide something required.

Useful Linux debugging tools may include:

    bluetoothctl
    btmon
    journalctl
    systemctl status bluetooth

The application should provide verbose logging modes, e.g.:

    RUST_LOG=debug
    RUST_LOG=trace

Log:

-   adapter discovered
-   scan started/stopped
-   advertisement discovered
-   peer ID
-   connection
-   service discovery
-   characteristic UUIDs
-   MTU if available
-   frame TX/RX
-   packet decode failures
-   fragmentation/reassembly
-   transfer progress
-   ACK/retry
-   disconnect reason

Do NOT log private plaintext or cryptographic secrets by default.

------------------------------------------------------------------------

# 10. Windows implementation

The desired outcome is one Rust desktop codebase that can target both:

    cargo build --release

on Linux and Windows.

Use conditional compilation only for genuinely platform-specific
behavior.

Example conceptual structure:

    transport/
        mod.rs
        common.rs
        linux.rs
        windows.rs

Before writing Windows-specific code, attempt to compile the selected
`bitchat-terminal`/btleplug stack on Windows and record the exact
failures.

Do not assume README claims equal verified support.

Create an interoperability matrix:

  Sender    Receiver     Text   File Notes
  --------- ---------- ------ ------ -------
  iPhone    Linux                    
  Linux     iPhone                   
  iPhone    Windows                  
  Windows   iPhone                   
  Linux     Windows                  
  Windows   Linux                    
  Linux     Linux                    
  Windows   Windows                  

Every supported pair should eventually have a reproducible test.

------------------------------------------------------------------------

# 11. iOS strategy

Do NOT start by building a new iOS application.

Phase 1:

Use the official BitChat iOS app as the known-good phone endpoint.

This immediately lets us test the desktop client.

If standard BitChat cannot support arbitrary files through our
extension, there are three possibilities:

1.  contribute file-transfer support upstream,
2.  fork the iOS BitChat client and add our extension,
3.  build a smaller custom iOS client using relevant open-source BitChat
    code.

Option 2 is likely much cheaper than Option 3.

If a fork is created, retain CoreBluetooth behavior and protocol code
wherever practical.

------------------------------------------------------------------------

# 12. Mesh networking

Mesh is NOT a V1 requirement.

Eventually we want:

    Phone A
       |
    Laptop B
       |
    Laptop C
       |
    Phone D

where B/C may relay a packet even when A and D are outside direct radio
range.

Potential fields:

    packet_id
    origin_id
    destination_id
    ttl
    hop_count

Each relay:

1.  receives packet,
2.  checks packet ID against deduplication cache,
3.  discards duplicate if already seen,
4.  decrements TTL,
5.  forwards if TTL \> 0.

BitChat already implements mesh concepts. Study its implementation
instead of inventing a routing algorithm immediately.

File relaying is much harder than text relaying and should come
significantly later.

------------------------------------------------------------------------

# 13. Security

Security should be designed deliberately, but do not block the first BLE
interoperability proof on building a new cryptosystem.

Never invent custom cryptography.

If maintaining BitChat compatibility, investigate and reuse its current
Noise-based session behavior.

Desired properties eventually:

-   confidentiality
-   integrity
-   peer authentication/verification
-   replay protection
-   forward secrecy where practical

Important distinction:

    "Bluetooth connection exists"

does NOT automatically mean:

    "application messages are end-to-end secure against all relevant threats."

For development, create a threat model documenting:

-   passive nearby listener,
-   active BLE peer,
-   impersonation,
-   replay,
-   malicious relay,
-   corrupted transfer,
-   resource exhaustion,
-   malicious filenames,
-   path traversal,
-   oversized metadata,
-   malformed packets.

File receiving MUST sanitize filenames.

Never allow a received filename such as:

    ../../.ssh/authorized_keys

to control an arbitrary filesystem path.

Default received files to a dedicated downloads directory.

------------------------------------------------------------------------

# 14. Protocol robustness

All decoders must treat remote input as untrusted.

Check:

-   minimum header length
-   maximum packet size
-   valid enum/type values
-   valid fragment indexes
-   total fragment limits
-   maximum filename length
-   maximum declared file size
-   integer overflow
-   allocation limits
-   UTF-8 handling
-   duplicate chunks
-   conflicting chunks
-   malformed hashes

Avoid allocating memory directly from an untrusted declared length
without a configured limit.

For large files, stream to disk rather than storing the entire transfer
in RAM.

------------------------------------------------------------------------

# 15. Testing strategy

## Unit tests

Test:

-   packet encode/decode
-   fragmentation
-   reassembly
-   malformed packets
-   duplicate packets
-   TTL behavior
-   chunk ACK state
-   SHA-256 verification
-   filename sanitization

## Property/fuzz testing

Protocol parsers are good fuzzing targets.

Random bytes should produce:

-   a valid parsed packet, or
-   a controlled parse error,

not a panic.

## Test vectors

Create protocol fixtures that every platform can consume.

Example:

    test-vectors/
        hello.bin
        text-message.bin
        file-offer.bin
        file-chunk.bin
        fragmented-message.json

For each vector document:

-   semantic object,
-   exact encoded bytes,
-   expected decoded representation.

This is crucial for Swift/Rust/Kotlin interoperability.

## Physical-device tests

BLE behavior cannot be validated completely using unit tests.

Test on actual hardware.

Minimum physical test set:

-   iPhone + Ubuntu laptop
-   iPhone + Windows laptop
-   Ubuntu laptop + Windows laptop

------------------------------------------------------------------------

# 16. Development milestones

## Milestone 0 - repository reconnaissance

Clone and inspect:

    permissionlesstech/bitchat
    ShilohEye/bitchat-terminal
    deviceplug/btleplug

Produce `docs/upstream-analysis.md` containing:

-   relevant source files,
-   protocol structures,
-   service UUIDs,
-   characteristic UUIDs,
-   BLE roles,
-   fragmentation logic,
-   encryption path,
-   Linux-specific code,
-   reusable modules,
-   licensing notes.

Do this BEFORE substantial implementation.

## Milestone 1 - existing interoperability

Goal:

    official BitChat iPhone <-> bitchat-terminal Linux

Success criteria:

-   Linux discovers phone/mesh peer.
-   Phone discovers Linux peer as applicable.
-   Linux receives text.
-   iPhone receives text.
-   repeated tests work without manual Bluetooth pairing hacks if
    protocol does not require them.

Document exact commands and logs.

## Milestone 2 - clean desktop core

Refactor only after Milestone 1 works.

Separate:

-   BLE transport
-   BitChat protocol
-   application state
-   CLI

Add automated packet tests.

## Milestone 3 - Windows

Make desktop client compile and run on Windows.

Success:

    Windows <-> iPhone text

then:

    Windows <-> Linux text

## Milestone 4 - file transfer prototype

Do NOT begin with a 500 MB file.

First transfer:

    hello.txt

containing:

    hello from offline-link

Then:

-   1 KB
-   100 KB
-   1 MB
-   10 MB

Verify SHA-256 every time.

## Milestone 5 - robust transfer

Add:

-   progress
-   cancellation
-   timeout
-   retry
-   disconnect recovery
-   resume

## Milestone 6 - UX

CLI commands might be:

    /peers
    /msg <peer> <text>
    /send <peer> <path>
    /accept <transfer>
    /reject <transfer>
    /transfers
    /cancel <transfer>

Eventually a TUI/GUI can wrap the same core.

## Milestone 7 - optional mesh extensions

Only after direct transfer is dependable.

------------------------------------------------------------------------

# 17. Initial commands for Codex to perform

When starting in a machine with Internet access, Codex should first
inspect upstream rather than immediately generating an application from
scratch.

Suggested workflow:

    mkdir offline-link-work
    cd offline-link-work

    git clone https://github.com/permissionlesstech/bitchat.git upstream-bitchat
    git clone https://github.com/ShilohEye/bitchat-terminal.git upstream-bitchat-terminal
    git clone https://github.com/deviceplug/btleplug.git upstream-btleplug

Then inspect repository licenses and git history.

Useful searches:

    rg "UUID|service|characteristic" upstream-bitchat
    rg "fragment|Fragment|reassembl" upstream-bitchat
    rg "Noise|noise" upstream-bitchat
    rg "BLE|Bluetooth|CoreBluetooth" upstream-bitchat

    rg "btleplug|UUID|characteristic" upstream-bitchat-terminal
    rg "fragment|reassembl" upstream-bitchat-terminal
    rg "protocol|packet" upstream-bitchat-terminal

Codex should identify the actual protocol and BLE implementation files
before changing anything.

------------------------------------------------------------------------

# 18. Rules for the coding agent

1.  **Do not rewrite working upstream code merely for stylistic
    reasons.**

2.  **Do not invent a new wire protocol until the existing BitChat
    protocol has been analyzed.**

3.  **Keep the project functional after small steps.**

4.  **Prefer a minimal working vertical slice over many incomplete
    modules.**

5.  **Do not claim Windows support merely because code compiles. Test it
    on hardware.**

6.  **Do not assume BLE is a reliable ordered byte stream.**

7.  **Do not hard-code a single BLE MTU.**

8.  **Do not invent cryptography.**

9.  **Treat all received packets/files as hostile input.**

10. **Maintain protocol test vectors.**

11. **Record upstream commit hashes when behavior is analyzed.**

12. **Keep upstream modifications isolated and easy to diff.**

13. **Prefer compatibility with existing BitChat clients where
    practical.**

14. **Do not make Internet connectivity a requirement.**

15. **Direct text messaging is more important than mesh for V1.**

16. **Direct file transfer is more important than file relaying.**

------------------------------------------------------------------------

# 19. Questions the agent must answer during reconnaissance

Before implementing file transfer, answer these with references to
source files:

### BLE

-   What service UUID does BitChat advertise?
-   Which characteristics are used?
-   Which side behaves as central?
-   Which side behaves as peripheral?
-   Does each BitChat device perform both roles?
-   How does discovery work?
-   How are connections initiated?
-   What assumptions does the Rust terminal client make?

### Protocol

-   Exact packet header layout?
-   Version field?
-   Message type field?
-   Sender/recipient representation?
-   TTL representation?
-   Packet/message ID?
-   How is fragmentation encoded?
-   How is reassembly performed?
-   How are duplicates detected?
-   Maximum payload assumptions?

### Encryption

-   Which packets are plaintext?
-   Which are encrypted?
-   How is Noise used?
-   How is peer identity bound to a session?
-   Can our file-transfer payload use the same encrypted session?

### Media

-   How does current BitChat transfer images/audio?
-   Are media bytes encrypted before fragmentation?
-   Is there already generic binary payload support?
-   Could arbitrary files reuse the media pipeline?
-   What size/reliability limits currently exist?

### Desktop

-   Which modules in `bitchat-terminal` are platform-independent?
-   Which modules are Linux-specific?
-   Does Windows compile?
-   If not, what exact APIs/dependencies fail?
-   Does the terminal client implement the latest protocol version?

------------------------------------------------------------------------

# 20. Likely opportunity: reuse BitChat media transfer

Before implementing `FILE_CHUNK` from scratch, inspect current official
BitChat media transfer.

The official project's release history indicates that images and voice
notes are transferred over BLE mesh, and later releases include
reliability and encryption fixes for media.

This means BitChat may already contain much of what arbitrary file
transfer requires:

    binary object
        ->
    metadata
        ->
    fragmentation
        ->
    encrypted payload
        ->
    BLE transmission
        ->
    reassembly

If so, the cleanest implementation may be:

    existing media transfer
            +
    generic MIME/type metadata
            +
    filename
            +
    save-to-disk behavior
            =
    arbitrary file transfer

Do not build a second fragmentation system until this has been
investigated.

------------------------------------------------------------------------

# 21. Performance expectations

BLE is chosen for availability and infrastructure independence, NOT
maximum throughput.

Text should feel effectively instant at nearby range.

Small files should be practical.

Large files may be slow.

Do not market V1 as an AirDrop-speed replacement.

Measure instead of guessing.

Record:

-   negotiated MTU,
-   average throughput,
-   retransmission rate,
-   connection setup time,
-   1 MB transfer time,
-   10 MB transfer time,
-   failure rate.

Later, an architecture could support multiple transports:

    application
        |
    transport abstraction
        |
        +-- BLE
        +-- Wi-Fi Direct / local Wi-Fi (optional)
        +-- LAN (optional)

But BLE must remain a fully functional standalone transport.

------------------------------------------------------------------------

# 22. Definition of V1 success

V1 is successful when the following works reliably without Internet or
Wi-Fi:

### Scenario A

    iPhone
      |
     BLE
      |
    Ubuntu laptop

-   discover peer
-   send text both directions
-   transfer a small arbitrary file
-   verify SHA-256
-   save file

### Scenario B

    iPhone
      |
     BLE
      |
    Windows laptop

Same requirements.

### Scenario C

    Ubuntu laptop
          |
         BLE
          |
    Windows laptop

Same requirements.

Mesh is not required for V1.

A GUI is not required for V1.

Accounts are not required.

Cloud infrastructure is not allowed as a dependency.

------------------------------------------------------------------------

# 23. Immediate first task

Do NOT start by generating hundreds of files.

The first task is:

> Clone the official BitChat repository and `bitchat-terminal`.
> Determine whether the existing Rust terminal client can currently
> exchange a simple text message with the official iOS BitChat
> application. Document the exact protocol files, BLE UUIDs, packet
> path, build steps, and any compatibility problems. Make the smallest
> changes necessary to achieve reliable Linux \<-\> iPhone text
> exchange.

After that works, commit it.

Suggested commit:

    feat: establish BitChat-compatible Linux-iPhone BLE messaging baseline

Only then proceed toward Windows and arbitrary file transfer.

------------------------------------------------------------------------

# 24. Research sources used to create this context

Official BitChat: https://github.com/permissionlesstech/bitchat

Official BitChat protocol source:
https://github.com/permissionlesstech/bitchat/blob/main/bitchat/Protocols/BitchatProtocol.swift

Official BitChat organization / Android repository:
https://github.com/permissionlesstech

Rust terminal client: https://github.com/ShilohEye/bitchat-terminal

btleplug: https://github.com/deviceplug/btleplug

These URLs are research starting points. The coding agent should inspect
the current repository state and record exact commit hashes before
relying on implementation details.

------------------------------------------------------------------------

# 25. Final project principle

The project is not:

> "build another chat UI."

The project is:

> **build a reliable cross-platform offline data link between ordinary
> consumer devices, beginning with BLE, while reusing the mature parts
> of BitChat instead of unnecessarily rebuilding discovery, framing,
> fragmentation, encryption, and peer communication.**

The shortest useful path is therefore:

    existing BitChat ecosystem
             |
             v
    prove interoperability
             |
             v
    isolate reusable desktop core
             |
             v
    Linux + Windows
             |
             v
    arbitrary file transfer
             |
             v
    reliability/resume
             |
             v
    optional mesh

Keep that ordering unless testing reveals a concrete reason to change
it.

------------------------------------------------------------------------

# 26. iPhone bootstrap and staged client strategy

A critical practical constraint is that the developer's primary
workstation may be Linux, while custom iOS applications require Apple's
iOS build/signing toolchain.

Therefore the project MUST use a staged iPhone strategy.

## Stage 1: stock BitChat on iPhone

Do not build a custom iOS application for the initial interoperability
milestone.

Install the official BitChat application on the iPhone and use it as the
known-good BLE endpoint.

Initial topology:

    iPhone
    official BitChat
          |
          | BLE only
          |
    Ubuntu laptop
    modified/forked bitchat-terminal

The first proof must work with:

-   Wi-Fi disabled,
-   cellular data unavailable/disabled,
-   no hotspot,
-   no cable used for communication,
-   Internet unavailable during the actual message exchange.

Internet may of course be used beforehand to install software and clone
repositories.

The first success condition is bidirectional text between the stock
iPhone BitChat client and the Linux terminal client.

## Stage 2: determine whether stock BitChat is sufficient for files

Before modifying iOS, inspect the current BitChat media implementation.

BitChat already supports binary media such as images and audio over BLE
mesh. Determine whether that subsystem can be generalized or reused for
arbitrary files.

Questions:

-   Is the binary media envelope generic enough for arbitrary bytes?
-   Where is MIME/media type encoded?
-   Is a filename supported?
-   What transfer size limits exist?
-   How are media fragments acknowledged/retried?
-   Is media encrypted before fragmentation?
-   Can an unknown/custom content type be ignored safely by stock
    clients?
-   Can arbitrary-file support be introduced as a backward-compatible
    capability?

If the stock iOS client can receive a generic payload with only minor
protocol-compatible changes on the desktop side, prefer that.

## Stage 3: fork iOS only when required

If arbitrary-file functionality requires changes on both peers, fork the
official BitChat iOS source instead of creating an iOS BLE application
from zero.

Preserve as much upstream functionality as possible:

-   CoreBluetooth transport
-   discovery/advertising behavior
-   protocol framing
-   fragmentation/reassembly
-   Noise sessions
-   peer identity
-   mesh routing
-   background behavior

Add only the minimum generic-file UI/protocol functionality needed.

A custom/forked iOS build will require access to macOS + Xcode and valid
Apple code signing/provisioning.

Do not make the custom iOS build a prerequisite for Milestone 1.

## Development principle

The Linux workstation should be able to perform almost all early
development.

Use the iPhone as a physical interoperability test device running stock
BitChat until a protocol extension genuinely requires an iOS fork.

------------------------------------------------------------------------

# 27. Upstream repository policy

The working project should distinguish between repositories that are
**primary development inputs** and repositories that are merely
**reference implementations**.

## Clone immediately

### 1. bitchat-terminal

    git clone https://github.com/ShilohEye/bitchat-terminal.git

This is the primary starting point for desktop development.

Do not merely install its binary. Keep a source checkout because it will
likely become the basis of our Linux/Windows client.

### 2. official BitChat iOS/macOS source

    git clone https://github.com/permissionlesstech/bitchat.git

Use this primarily as a protocol and BLE reference during early
development.

Do not attempt to build it on Linux.

Inspect it to understand the exact behavior expected by the iPhone app.

## Clone when protocol comparison becomes useful

### 3. official BitChat Android

    git clone https://github.com/permissionlesstech/bitchat-android.git

This is extremely useful as an independent implementation of BitChat
behavior.

When Swift behavior is ambiguous, compare the Kotlin implementation.

Because this repository is GPL-3.0, do not blindly copy source into a
differently licensed project. Treat it primarily as a
behavioral/reference implementation unless project licensing has
deliberately been made compatible.

## Usually do not clone initially

### btleplug

`bitchat-terminal` already depends on btleplug.

Do not clone btleplug merely to use the dependency; Cargo handles that.

Clone it only if:

-   debugging btleplug itself,
-   inspecting platform backend implementation,
-   patching Windows/Linux BLE behavior,
-   pinning a local fork.

If needed:

    git clone https://github.com/deviceplug/btleplug.git

## Suggested local workspace

    ~/dev/offline-link/
        context.md
        upstream/
            bitchat/
            bitchat-android/
        desktop/
            bitchat-terminal/

The exact directory names are not important, but keep pristine upstream
references separate from the desktop working tree.

Before making substantial changes, create a project fork/branch rather
than losing the ability to compare against upstream.

Record commit hashes:

    git -C upstream/bitchat rev-parse HEAD
    git -C desktop/bitchat-terminal rev-parse HEAD
    git -C upstream/bitchat-android rev-parse HEAD

Put these in `docs/upstream-analysis.md`.

------------------------------------------------------------------------

# 28. First physical test procedure

The coding agent should optimize for reaching this test as quickly as
possible.

1.  Install official BitChat on the iPhone.
2.  Enable Bluetooth and grant the application's required Bluetooth
    permissions.
3.  On Ubuntu, verify the Bluetooth controller exists.
4.  Build `bitchat-terminal` from source.
5.  Run it with its debug mode enabled.
6.  Verify that BitChat peers/advertisements are observed.
7.  Attempt iPhone -\> Linux text.
8.  Attempt Linux -\> iPhone text.
9.  Once communication is confirmed, disable Wi-Fi on both sides and
    ensure the exchange still succeeds over BLE.
10. Save relevant debug output and record exact upstream commit hashes.

Do not begin arbitrary file-transfer work until this baseline is
reproducible.
