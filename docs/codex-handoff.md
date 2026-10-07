# codex handoff: chatt3r

current snapshot: **user-confirmed two-way public text over BLE between the
Linux laptop and Windows PC, with iphone Bluetooth off.** this is one physical
session. the user later reported that the new desktop-only service **worked
on hardware with iphone Bluetooth on throughout**, but did not separately
confirm directions or provide simultaneous logs for that run. this is not a
finished multi-device link. check the
working tree, current code and [laptop-to-laptop.md](laptop-to-laptop.md)
before updating claims.

## purpose and evidence

build an infrastructure-free cross-platform link for nearby **text and files**.
BitChat supplies the protocol/compatibility reference, not an invention we
claim as ours. [context.md](../context.md) describes the long-term goal, not
what is shipped. no files, private messages, delivery receipts or full
live-session reconnect are implemented.

| path | evidence |
| --- | --- |
| stock iphone ↔ linux | bidirectional public text user-confirmed on physical BLE; includes an iphone-offline Linux direct-LE run |
| windows → iphone | user-confirmed native Windows BLE client text; iphone → windows not separately confirmed |
| linux ↔ windows pc | **user-confirmed two-way public text over BLE with iphone Bluetooth off** using the stock service. Linux received signed `windows-pc` announcement and text `yo`, `ok got it`; Linux sent `hi`, `yoooooooo`. user later reported desktop-only service worked with phone Bluetooth **on throughout**, but directions and Wi-Fi/cellular switch states weren't separately recorded |
| linux ↔ linux | not working; this laptop's Realtek adapter rejected a temporary BlueZ advertisement (`Invalid Parameters (0x0d)`) |
| software-only simulated peers | signed two-way text, fragmented frames, duplicate/rejection tests; **not** radio evidence |

public text is **not encrypted**. signing identities are ephemeral for each
run, not trusted device pairing. maximum text is **99 UTF-8 bytes**. the
launcher's `128`-byte frame limit is operator-selected, **not a measured MTU**.
a successful GATT write is not an application delivery receipt. never replay
an ambiguous message after link failure.

## architecture checkpoint: proven one-link text

```text
windows pc --host (WinRT GATT; proven stock service with phone off,
                    desktop-only service now default, radio test pending)
  ↑ central writes / ↓ characteristic notifications
linux laptop (btleplug fresh scan + connect + subscribe)
  ↔ shared BitChat-compatible signed packets, Receiver, terminal chat
```

- `desktop/bitchat-terminal/src/bin/chatt3r.rs`: `discover`, `LinkWriter`,
  `send`, `Receiver::receive`, `chat`, and Windows `--host` entry point. same
  packet validation and UI are used for the client and host transports.
  `--desktop-peer` selects UUID `88d5ec18-2621-4233-ad22-82702a601c97`
  for Linux/Windows central discovery; no flag still scans stock BitChat.
  Windows `--host` defaults to desktop; `--host --stock-host` retains the
  original phone-off host route for explicit regression tests.
- `desktop/bitchat-terminal/src/baseline/windows_gatt.rs`: native Windows
  GATT service under the desktop-only UUID, single ordered inbound write
  worker, subscribed-client checks, targeted notifications, reported ATT MTU
  cap and explicit cleanup. the host waits for a valid signed incoming
  announcement before notifying.
- `desktop/bitchat-terminal/src/baseline/host_policy.rs`: pure single-central
  subscription rules tested without Windows hardware. a briefly empty
  subscriber list gets a grace period; a different or additional subscriber
  causes the current host to fail closed.
- `desktop/bitchat-terminal/src/baseline/protocol.rs`: upstream-compatible
  wire format, signing and fragmentation. `baseline/linux_att.rs`: working
  separate Linux-only raw-ATT workaround for the known iphone; **don't
  regress** it to change the Windows host. `baseline/discovery.rs` rejects
  BlueZ cached UUIDs until there is fresh scan evidence.
- `desktop/bitchat-terminal/src/baseline/file_packet.rs` and `src/lib.rs`:
  a **pure, bounded canonical-v2-layout file TLV codec** with independent
  layout fixture (strict reader; no legacy-length or multi-content support),
  currently unit-tested only; `--bin chatt3r` does not import it, accept
  file frames, send media or save files. `baseline/file_wire.rs` is a pure
  signed-v2 `0x22` **wire-preimage** checker with Python-generated raw-DEFLATE
  and uncompressed fixtures; it does not inflate or validate compressed
  content. Apple's canonical recompression has **not** been cross-checked.
  bounded expansion, cross-language fixtures and larger per-type fragment
  bounds are still missing.
- [upstream-analysis.md](upstream-analysis.md) records source attribution;
  [direct-le.md](direct-le.md) records phone-path design/evidence;
  [windows.md](windows.md) has native PowerShell run commands;
  [laptop-to-laptop.md](laptop-to-laptop.md) records the hardware milestone
  and duplicate-link **design, not shipped**. build `--bin chatt3r`, not the
  preserved legacy `bitchat` binary.

## hardware sequence and current blockers

prior Windows runs showed peripheral support `true`, advertising `started`
(initial `aborted/success` could precede `started/success`), subscription,
then rejected writes with ATT `0x03`. the Windows GATT host reported a
**subscriber conflict with `count=2`**; the inbound writer was not its
initially selected central. turning iphone Bluetooth off allowed the later
signed two-way text session. that does **not prove** the iphone was the
second subscriber; closing its app alone had not prevented the conflict.
`count=2` means two WinRT subscribed sessions, not necessarily two physical
devices. don't unpair/reset devices to hide this issue. a new **desktop-only
service UUID** is now used by Windows `--host`; stock BitChat's upstream
iOS central filters for its own service UUID, and the Linux client must opt
in with `--desktop-peer`. this separates discovery, not packet format or
cryptographic trust. **after this document's earlier snapshot**, the user
reported the desktop-service test worked with phone Bluetooth on throughout;
we still lack direction-by-direction logs. fallback to the old stock-service
host only with explicit Windows `--host --stock-host` and Linux
`CHATT3R_LE_PEER= ./scripts/chatt3r ...`, with phone Bluetooth off.

in the successful run Linux logged two announcement fragments from
`windows-pc`, `[windows-pc] yo`, `[windows-pc] ok got it`, and outbound
`hi` / `yoooooooo`; the user confirmed the exchange worked. after a signed
`windows-pc left`, Linux's later periodic announcement failed because BlueZ's
`WriteValue` GATT D-Bus object disappeared. the user thinks they pressed
Ctrl-C on the Windows host. this is **consistent with peer shutdown**, not
a bad 128-byte limit. `chatt3r.rs` now labels that error as a disappeared
characteristic rather than suggesting MTU. do not auto-replay messages.

### next work, in order

1. **record the phone-on result accurately and, when practical, repeat it
   direction by direction** on the same pair. Windows runs `--host`; Linux
   Linux `./scripts/chatt3r --desktop-peer --scan-only --scan-seconds 30`
   and then `./scripts/chatt3r --desktop-peer --debug --scan-seconds 90
   --name laptop` (one shell line). the launcher bypasses its saved phone
   for this flag. the user says the new path worked while phone Bluetooth
   stayed on; record which texts arrived on each side, with sanitized
   simultaneous consoles. don't infer Wi-Fi/cellular switch states or
   multi-subscriber resilience from that report.
2. if `count=2` persists on the desktop service, design competing-subscriber
   handling: bind fragments and notifications to a selected GATT session
   after validation, reject/ignore others without blacklisting the chosen
   one, and don't silently switch peers or replay ambiguous writes. this
   would be a separate subsystem; a unique UUID alone is not authentication
   or general laptop-to-laptop role negotiation. keep the phone-off proven
   session as regression evidence. record Wi-Fi/cellular switch states if
   measuring a fully offline run.
3. test Windows `/quit`/Ctrl-C and Linux teardown, out-of-range behavior and
   restart explicitly. the client does not reconnect an established session
   or provide application delivery ACKs.
4. Linux ↔ Linux still needs a capable advertising adapter and a Linux GATT
   server. generic file receiving/sending comes after a stable phone-free
   transport. do not claim files are usable just because upstream wire
   formats can represent binary data.

## local checks and safety

```bash
cargo test --offline --locked --manifest-path desktop/bitchat-terminal/Cargo.toml
cargo clippy --offline --locked --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r -- -D warnings
python3 tests/launcher-smoke.py
python3 desktop/bitchat-terminal/tests/ui-smoke.py
cargo check --offline --locked --target x86_64-pc-windows-gnu --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r
cargo clippy --offline --locked --target x86_64-pc-windows-gnu --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r -- -D warnings
```

Windows-target checks **do not test the PC radio**. the Windows PC must
build/run the host natively. don't recommend WSL as a substitute for its
Bluetooth adapter. use lowercase architectural checkpoints with code paths,
data flow and evidence. don't edit/stage/commit the user's untracked
`codexguide.md`, real phone address, `~/.zshrc`, raw sensitive logs or Pi
session export. avoid bond deletion or adapter resets as routine fixes;
review staged changes and push tested, honestly labeled updates.
