# First chatt3r Linux ↔ iPhone test

**Status: user-confirmed bidirectional text.** The user reported that both
message directions worked after connecting/subscribing and receiving a signed
`iphone` announcement. Installed app/iOS version and repeated offline exchanges
are not recorded yet; complete the checklist below before calling it reliable.

## Setup

1. Install stock BitChat on iPhone. Record its app version and iOS version.
2. Enable Bluetooth and grant BitChat Bluetooth permission. Keep the app open
   in the foreground, in the **Bluetooth/mesh public room**, not geohash or DM.
3. For this narrow first test, use a short iPhone nickname such as `iphone`
   (ideally ≤8 UTF-8 bytes), with only these two BitChat clients nearby. Longer
   announcement metadata may cross the compression limit unsupported here.
4. On Ubuntu, check the adapter. Do not pair devices or change permissions
   unless a concrete error demonstrates that it is necessary:

```bash
bluetoothctl list
bluetoothctl show
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --locked --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r
cargo test --locked --manifest-path desktop/bitchat-terminal/Cargo.toml
```

No sudo build/install is needed. Missing dependencies on another Ubuntu machine
may require installing `build-essential pkg-config libdbus-1-dev bluez` beforehand.
Network access is only for setup; runtime uses local BlueZ/BLE, no server.

## Discovery-only check

From the workspace root:

```bash
./desktop/bitchat-terminal/target/debug/chatt3r --scan-only --scan-seconds 30
```

This never connects or sends application packets. It should print
`[scan] BitChat service found`. No advertisement observed means check the
foreground app, Bluetooth permission, range, and production/testnet app build.
It is not proof that the protocol is incompatible.

## Bidirectional short public text

```bash
./desktop/bitchat-terminal/target/debug/chatt3r --write-limit 128 --name chatt3r-linux --debug
```

`128` is an **initial operator-selected characteristic value limit**, NOT a
measurement of negotiated MTU. The program sizes upstream-format fragments to
fit this configurable limit. btleplug 0.11.8 does not expose a portable negotiated
MTU. If GATT rejects writes, capture the exact error and try `--write-limit 64`.
Links with values smaller than the 35-byte minimum fragment overhead cannot use
this harness; do not claim universal BLE support. Do not infer payload limit or
MTU from a single successful write with response (it may be a long GATT write).

1. Wait for `[ble] connected and subscribed` and a signed `[peer]` announcement.
   The CLI sends its own signed announcement immediately and every 15 seconds.
2. Send `hello from iphone` in the iPhone Bluetooth public room. Confirm the
   Linux screen displays it with the correct nickname.
3. Type `hello from linux` in the terminal. Confirm it appears on iPhone.
   A `[tx] GATT write complete` line alone **does not prove delivery**.
4. Use `/peers` to inspect discovered peers. `/announce` resends our announcement.
5. Repeat with numbered short messages in both directions at least 10 times.
6. Disable Wi-Fi and cellular data on iPhone and Wi-Fi on Linux, leave Bluetooth
   enabled, and repeat. No hotspot, cable, or Internet-dependent room.
7. `/quit` or Ctrl-C disconnects. Restart the desktop process and repeat to test
   reconnection. Identity is deliberately ephemeral per process in this harness.

For everyday chat, omit `--debug` from the command above. The normal UI shows
messages and one-time peer notices, not recurring frame/sync/announce logs.
Each peer gets a consistent nickname color and your messages are cyan. Editable
`you>` input keeps your draft when messages arrive; arrow keys recall session-only
history. `NO_COLOR=1` disables colors; piped output stays plain. `/quit` and restart
to load a newly built binary. Ignored/malformed frame diagnostics require `--debug`.

Messages are limited to **99 UTF-8 bytes**, not 99 characters. Public plaintext
is signed but not confidential, not Noise-authenticated. Do not send secrets.
The opt-in **small public file receiver** is software-tested; the first real
iPhone image log **shows no saved file**. repeated type-`0x20` values were
rejected as truncated or unsupported, before file verification/saving. No file sending, private media,
encryption sessions, auto-reconnection, delivery ACKs or gossip sync is implemented.

`[drop]` errors are intentional diagnostics, not panics. A compressed/routed
frame or oversized announcement needs follow-up support. A short nickname and
isolated two-device test may avoid compressed announcements; this is a temporary
baseline limitation, not a protocol rule. If a signed public message arrives
before a valid announcement, wait for the next announce and resend the message.

## retry the first receive-only public image (opt-in; still unproven)

keep your working phone link and bond unchanged. the stock iPhone UI in the
inspected source can **send a public image/voice note**, not necessarily an
arbitrary document. create a dedicated existing directory you own; nothing
is stored unless you opt in. use a harmless small public image: the decoded
file cap remains **64 KiB**, encoded signed outer frame about **65 KiB**, and
only 16 saves are allowed per run. larger images are not supported.

```bash
mkdir -m 700 -p "$HOME/chatt3r-file-test"
./scripts/chatt3r --build
./scripts/chatt3r --name laptop --debug --receive-files "$HOME/chatt3r-file-test"
```

the explicit `--build` matters: the launcher otherwise reuses an existing
older binary. if the first offline build lacks the newly added `flate2`
crate, run `cargo build --locked --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r`
once online, then rerun the launcher. this laptop's local `CHATT3R_LE_PEER`
setting makes the launcher choose its known phone with `--wait-for-peer` for
this run; on another Linux setup use the already working direct-LE mode.
do not add `--desktop-peer`, `--host` or `--scan-only`.

opt-in direct LE now asks for **ATT MTU 517** instead of the proven text
default **185**. note the `[ble] negotiated ATT MTU=… (value limit=…)` line.
if the negotiated value limit is **below 504**, file mode stops with a clear
error and the image cannot fit on this link; rerun without `--receive-files`
for the previously working public text. the user already confirmed MTU **517**
and receipt of full 504-byte file fragments. that run still failed on a
46-part image at the old 16 KiB assembly cap; two complete `0x20` frames
had compression flag `0x04` (not proven to be file frames). after rebuilding,
the file-only path allows bounded ~65 KiB assembly and bounded compressed
fragment decoding. if the iPhone resends a small image in its **public**
Bluetooth room after a signed announcement, a successful receive would
report `received-<random>.bin`, size and SHA-256; verify bytes/hash manually
and record whether it arrived once. this change is **software-tested only**.
files are saved as **generic `.bin` without using the untrusted filename or
auto-opening media**. only jpeg/png/gif/webp with
matching magic bytes and `application/octet-stream` are allowed for the first
test. the 128-byte write limit is not a measured MTU or a phone file size
promise. earlier runs recorded 504-byte file fragments arriving first as
182 bytes, then fully at value limit 514 but rejected after ~36/46 parts.
there is still **no confirmed saved image**. report just the negotiated MTU,
a few sanitized `[rx-shape]`/`[drop]` lines if any, and whether one `.bin`
was saved with matching contents; do not post device addresses, peer IDs,
raw media or a full debug transcript. changing `--write-limit` does not
change incoming frame size; don't reset Bluetooth or remove bonds. other
possible drops remain MIME, signature, 64 KiB content cap, ~65 KiB outer
frame, sender admission or unsupported private media.

## Record evidence

To record the interactive run with util-linux `script`:

```bash
mkdir -p logs
script -q -c './desktop/bitchat-terminal/target/debug/chatt3r --write-limit 128 --name chatt3r-linux --debug' logs/linux-iphone.typescript
```

The transcript includes public text, nicknames, and peer IDs. Use test content
only; review/redact before sharing. Debug metadata never intentionally prints
private keys or raw cryptographic secrets. Do not add private/local logs to git.

Save:

- Installed BitChat/iOS version, Linux adapter model, Rust and BlueZ versions.
- Source pins from `docs/upstream-analysis.md` and the desktop branch/diff.
- Exact invocation and configured write limit; measured MTU remains unknown.
- Each test message's direction and observed arrival (include phone observation).
- Offline settings, timing, retries, disconnect/restart behavior and errors.
- Whether all 10 exchanges passed. If not, retain failure evidence.

## later direct-le recovery evidence

on an app store v1.7.1 iphone, bluez's paired dual-mode identity omitted the
bitchat uuid even though a read-only hci capture showed the iphone advertising
it. generic `Device1.Connect()` tried hands-free/audio and failed. forced LE
GATT discovery found the bitchat service and notify/write characteristic
without unpairing. the opt-in linux direct le backend negotiated mtu 185,
subscribed, sent signed packets, and received signed peer announcements and
public text from the nickname `android`. the user confirmed that nickname is
their iphone and that **text worked in both directions with the iphone fully
offline**. raw captures were deleted. this is user-confirmed physical evidence;
precise switch settings, a numbered repeated run, and reconnection-after-range
still need recording. see [direct-le.md](direct-le.md).

## Local scan already performed

Command (normal user, no sudo):

```text
./desktop/bitchat-terminal/target/debug/chatt3r --scan-only --scan-seconds 12
```

Observed output:

```text
[ble] adapter=hci0 (usb:v1D6Bp0246d0548)
[scan] service=f47b5e2d-4a9e-4c5a-9b3f-8e1d2c3a4b5c; duration=12s; no pairing required
[scan] duration ended
```

No BitChat advertisement observed during this scan. Adapter access and scan
start/stop worked. No connection or message exchange was attempted.
