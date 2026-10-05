# codex handoff: chatt3r

last reviewed after `a241f3c`. this is a **snapshot**, not evidence of a
subsequent hardware run. check `git status`, `git log -5 --oneline`, the current
code, and [laptop-to-laptop.md](laptop-to-laptop.md) before updating a claim.

## purpose and evidence

make an infrastructure-free, cross-platform nearby link for **text and files**,
starting with BLE. stock BitChat is the compatibility reference, not code we
claim to have invented. favor a direct peer link over mesh. [context.md](../context.md)
is a long-term roadmap; some old priorities in it and the user's local
`codexguide.md` predate the Windows host work. no files, private messages,
application delivery receipts, or full live-session reconnect are implemented.

| path | actual evidence |
| --- | --- |
| stock iphone ↔ linux | bidirectional public text user-confirmed on real BLE hardware, including an offline run via Linux direct LE |
| windows → stock iphone | user-confirmed text with the native Windows central client; iphone → windows **not** separately confirmed |
| two simulated desktop peers | signed text both directions, fragmentation, duplicate suppression and rejection unit-tested; **no radio** |
| linux ↔ windows pc | Linux discovered two unnamed live BitChat candidates, connected to **one**, found `WRITE | NOTIFY`, subscribed and tried an announcement; first write failed with ATT `0x11`. Windows console output was not provided, so **the candidate's identity, Windows advertisement, write reception and two-way packets are unverified** |
| linux ↔ linux | no physical peer test. local Realtek adapter rejected even a temporary BlueZ advertisement (`Invalid Parameters (0x0d)`) |

avoid collapsing discovery, GATT subscription, app-layer signed announcement and
received text into a single notion of "connected". text is limited to **99
UTF-8 bytes**, public and **not encrypted**. `128` is an operator-configured
frame limit, **not** a measured MTU. the initial Linux announcement at that
limit had two frames. ACKs are only GATT-level; delivery to an application is
not confirmed. signing identities are ephemeral per run, not an established
trust relationship.

## current architecture and where to look

```text
linux central (btleplug)                     windows peripheral (WinRT)
scan service UUID + fresh radio evidence  ←  GattServiceProvider advertisement
connect/discover/subscribe                ↔  GattLocalCharacteristic WRITE | NOTIFY
write frames                              →  ordered WriteRequested worker / bounded rx
receive notifications                     ←  NotifyValueForSubscribedClientAsync
                    shared packet validator + Receiver + terminal chat
```

- `desktop/bitchat-terminal/src/bin/chatt3r.rs`: `options`, `discover`,
  `prepare_link`/`connect_with_retry`, `LinkWriter`, `send`, `Receiver::receive`,
  `chat`, and the `--host` entry point. same signed packet and UI path for both
  transports; no automatic replay of user messages after chat starts.
- `desktop/bitchat-terminal/src/baseline/windows_gatt.rs`: Windows-only
  `GattHost::start`, `wait_for_client`, `connected`, `write`, `process_write`,
  advertisement event/status diagnostics and cleanup. it serves **one**
  subscribed central, not an authenticated named Linux identity. current
  diagnostic replies: `0x03` disallowed writer/subscription change, `0x07`
  nonzero offset, `0x0d` bad value length, `0x11` inbound queue full/closed.
  the **earlier** Windows version returned `0x11` for several unrelated
  rejection causes; do not infer MTU or queue saturation from the old trace.
- `desktop/bitchat-terminal/src/baseline/linux_att.rs`: separate Linux-only
  raw LE ATT workaround for the *known phone* when BlueZ picks classic/audio.
  don't rewrite this path to fix Windows and don't disclose the phone address.
- `desktop/bitchat-terminal/src/baseline/protocol.rs`: packet framing,
  fragment/reassembly, signature validation and fixtures.
- `desktop/bitchat-terminal/src/baseline/discovery.rs`: reject cached BlueZ
  profiles until a fresh discovery/radio update; two unnamed candidates are
  **ambiguous**. `scripts/chatt3r` defaults to a locally configured known
  phone unless `CHATT3R_LE_PEER` is explicitly empty for the PC test.
- [windows.md](windows.md): native PowerShell build/run and full two-machine
  procedure. [laptop-to-laptop.md](laptop-to-laptop.md): observed adapter
  blocker and role policy design (tie-break is **not shipped**).
  [direct-le.md](direct-le.md): proven Linux phone workaround.
  [upstream-analysis.md](upstream-analysis.md): source attribution and
  compatibility findings. build **`--bin chatt3r`**, not the legacy binary.

## the live blocker: first linux → candidate write

the user reported this Linux trace (identifiers omitted here):

```text
[scan] live BitChat candidate; name=(unnamed)     # printed twice
[ble] connection established; discovering GATT services
[ble] GATT discovery complete; 30 characteristics
[ble] subscribing ... properties=CharPropFlags(WRITE | NOTIFY)
[ble] connected and subscribed
[tx] type=0x01 payload_bytes=76 frames=2 configured_limit=128
Error: "BLE write failed: Operation failed with ATT error: 0x11"
```

**we have not received the corresponding Windows host output** or confirmation
that the iphone app was closed. the Windows host originally reported
"advertisement not usable"; a later fix waits through transient `Stopped`,
logs the Windows BluetoothError, and accepts a **partial** advertisement only
as a reason to try a Linux scan, not proof the service UUID was on air. there
has been no confirmed retest of the latest Windows diagnostic build.

### next decisive test, in order

1. ask for the existing Windows terminal output around that Linux run: ad
   status/error, whether it printed a subscription line, whether it stayed
   running, and whether stock BitChat was open on the iphone. don't share
   device addresses or private logs. an old `Linux subscribed` label was
   misleading; the updated text is `one central subscribed` because the host
   has not identified the central.
2. close the iphone **app** for a clean one-host test (leave bonds/settings
   intact). on Windows, update the checkout (if no conflicting local edits),
   build and run as a normal user in PowerShell:

   ```powershell
   git pull --ff-only
   cargo build --locked --manifest-path .\desktop\bitchat-terminal\Cargo.toml --bin chatt3r
   & .\desktop\bitchat-terminal\target\debug\chatt3r.exe --host --write-limit 128 --name windows-pc --debug
   ```

   wait for `started`, or treat `started_without_all_advertisement_data` as
   **partial**; `aborted`/timeout needs its BluetoothError, not an adapter
   reset. the PC's new code should print queued/rejected inbound writes
   without logging contents or addresses.
3. on the Linux laptop from its checkout, **disable its local phone shortcut
   for each command** (not the saved phone setting or its bond):

   ```bash
   CHATT3R_LE_PEER= ./scripts/chatt3r --scan-only --scan-seconds 30
   CHATT3R_LE_PEER= ./scripts/chatt3r --debug --scan-seconds 90 --name laptop
   ```

   correlate Windows subscription/write lines with the Linux timestamp. if
   Windows logs no subscriber, Linux may have selected another candidate. if
   Windows logs a rejection, use the **category** rather than guessing MTU;
   if it queues frames, trace ACKs and announcement/notification handling.
4. only after signed announcements and short `hi` appear **both ways** on both
   PCs without the phone may the matrix say laptop ↔ PC text works. test
   disconnect behavior and ambiguous writes separately; never replay a text
   the peer may already have received. don't promise files or delivery ACKs.

## local checks and guardrails

```bash
cargo test --offline --locked --manifest-path desktop/bitchat-terminal/Cargo.toml
cargo clippy --offline --locked --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r -- -D warnings
python3 tests/launcher-smoke.py
python3 desktop/bitchat-terminal/tests/ui-smoke.py
cargo check --offline --locked --target x86_64-pc-windows-gnu --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r
cargo clippy --offline --locked --target x86_64-pc-windows-gnu --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r -- -D warnings
```

`rustup target add x86_64-pc-windows-gnu` may be required for cross-check;
Cargo may need a one-time online fetch. those checks **do not exercise Windows
Bluetooth**. previous local runs passed 4 legacy unit tests, 28 `chatt3r`
unit tests (2 interactive cases intentionally ignored), launcher smoke and
4 Linux terminal PTY cases; Windows-target check/Clippy passed. new code must
be tested again. do not run WinRT host code on Linux or recommend WSL as a
substitute for native Windows BLE.

only the user's `codexguide.md` is currently untracked. don't stage/edit it,
read local `~/.zshrc` into a committed file, include real peer addresses, or
commit exported conversations. preserve Bluetooth pairings and existing
Linux ↔ iphone behavior. use lowercase, readable architectural checkpoints;
push **tested** changes with evidence labeled honestly. see [AGENTS.md](../AGENTS.md).
