# chatt3r

text from your iphone to your linux laptop, over bluetooth. no wifi, hotspot,
cable, account, or server needed by the desktop client during chat.

built on the bitchat ecosystem instead of starting a new protocol from scratch.
the longer-term goal is an offline link for text **and files** across linux,
windows, and iphone. right now, it's an early **public-text-only** client.

> public messages are not encrypted. use test text, not secrets.

## where it's at

| feature | status |
| --- | --- |
| linux ↔ stock iphone bitchat text | user-confirmed on real devices |
| quiet chat, colored nicknames, editable input | implemented; terminal-tested |
| startup reconnect attempts | bluez path: up to three per candidate; direct le: optional cancellable wait (closed-app wait observed, reopen transition pending) |
| known dual-mode phone whose bluez profile hides bitchat | linux `--direct-le` path; bidirectional iphone text user-confirmed, including an offline run |
| longer text / compression | not supported yet; max 99 utf-8 bytes |
| photos, voice notes, arbitrary files | not supported by chatt3r yet |
| private/encrypted messages | not supported yet |
| windows → stock iphone text | user-confirmed on native windows; reverse direction not separately confirmed |
| linux ↔ windows pc text | windows host physically advertised and got a subscriber/write, then our too-strict subscription handler rejected it (`ATT 0x03`); fix awaiting physical retest. **no confirmed signed text** |

the user confirmed an iphone ↔ linux text exchange on the direct le path with
the iphone offline. exact wifi/cellular switch states, multiple repetitions,
and background startup recovery still need a recorded test. automatic recovery
from a cold notification subscription or a missing app service on the **default**
bluez path is not yet physically validated. this isn't a finished, guaranteed-reliable file-sharing app.

## get it running

these steps are for ubuntu. **windows:** see the experimental
[windows setup and run guide](docs/windows.md); the windows gatt **host**
is experimental, and a pc ↔ linux signed text exchange is not yet confirmed.
windows → iphone text is user-confirmed. for a coding-agent handoff, start at
[AGENTS.md](AGENTS.md) and [docs/codex-handoff.md](docs/codex-handoff.md).

### 1. set up the laptop

on ubuntu, install [rust](https://rustup.rs) and the build/bluetooth prerequisites:

```bash
sudo apt install build-essential pkg-config libdbus-1-dev bluez
```

then clone and build, as your normal user:

```bash
git clone https://github.com/l4keisn0tr34l/chatt3r.git
cd chatt3r
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --locked --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r
```

internet is needed for initial installs/downloads, not for the actual message
exchange. the launcher never downloads dependencies automatically.

### 2. get the iphone ready

- install stock bitchat.
- enable bluetooth and its permission for bitchat.
- use a short nickname like `iphone` for the first test.
- open the **bluetooth public room**, not a geohash/location room or dm.
- keep the phone unlocked and the app visible while connecting.

manual bluetooth pairing isn't required. pairing in system settings alone does
not establish a bitchat connection.

### 3. start chat

```bash
./scripts/chatt3r
```

wait for a nearby peer, then send `hi` in both directions.

```text
iphone is nearby
[iphone] hi from iphone
[you] hi from linux
you>
```

nicknames are colored per peer; your messages are cyan. incoming messages keep
the draft you're typing intact. arrow keys edit/recall input; history stays in
memory, not on disk. ctrl-c, ctrl-d, or `/quit` exits.

`[you]` means the bluetooth writes completed, not that the phone confirmed
delivery. check the phone screen; application delivery receipts aren't wired yet.

## make it a one-word command

put an alias in `~/.zshrc` using the absolute path to **your** checkout. for the
existing local setup at `~/dev/offline-link`:

```bash
alias chatt3r='"$HOME/dev/offline-link/scripts/chatt3r"'
```

reload it:

```bash
source ~/.zshrc
chatt3r
```

if you cloned into `~/chatt3r`, use that path instead. the launcher itself works
from any folder because it resolves paths relative to its own location.

## commands you'll actually use

| shell command | what it does |
| --- | --- |
| `chatt3r` | quiet chat |
| `chatt3r --debug` | chat with connection, tx/rx, and rejected-packet logs |
| `chatt3r --help` | print the cheat sheet; no connection |
| `chatt3r --name laptop` | choose a nickname, max 24 utf-8 bytes |
| `chatt3r --scan-only --scan-seconds 30` | scan without connecting or sending |
| `CHATT3R_LE_PEER= chatt3r --debug --scan-seconds 90` | scan for a pc host instead of using this laptop's saved phone shortcut; see [windows test](docs/windows.md) |
| `chatt3r --direct-le <phone-address>` | linux-only LE connection for a known phone when bluez picks classic/audio instead |
| `CHATT3R_LE_PEER=<phone-address> chatt3r` | locally select that direct LE phone and wait for its app to become ready; never publish your real address |
| `chatt3r --doctor` | inspect the laptop adapter; changes nothing |
| `chatt3r --build` | rebuild offline from cached dependencies, then exit |
| `NO_COLOR=1 chatt3r` | disable nickname colors |

without an alias, replace `chatt3r` with `./scripts/chatt3r` from the repo root,
or use the launcher's absolute path.

inside chat, at the `you>` prompt:

```text
/peers
/announce
/quit
```

`/announce` resends your presence, not your last message. `--debug` is a launch
option, not an in-chat command, and it does **not** mean “print help”.

## two limits worth knowing

### the 128-byte frame limit

the launcher supplies `--write-limit 128`, the value used in our initial hardware
tests. this caps each outgoing frame **including protocol overhead**; larger
packets are fragmented.

it's not a measured mtu, not a file-size limit, and not a promise that every
adapter supports it. another link may need a different value:

```bash
chatt3r --write-limit 64 --debug
```

### the 99-byte text limit

messages currently have to fit in **99 utf-8 bytes**. plain english characters
usually use one byte; emoji and other unicode characters may use more.

- longer outgoing messages are rejected, not truncated.
- longer incoming messages are rejected; `--debug` shows the reason.
- changing the frame limit won't raise this text limit.

this temporary restriction avoids the upstream compression threshold until we
implement compatible compression and signature handling. a 500-character or
500-byte message won't work yet.

## if it stops talking

the full command-and-phone checklist is in **[docs/reconnect.md](docs/reconnect.md)**.
start with:

```bash
chatt3r --debug --scan-seconds 90
```

| symptom | what it means / what to try |
| --- | --- |
| no live bitchat peer found | keep the app visible; if the phone advertises but bluez hides its uuid, use [direct le recovery](docs/reconnect.md#if-bluez-picks-the-phones-audio-profile-instead-of-bitchat-le) |
| bitchat characteristic unavailable | a live address may carry a cached uuid; the client skips it and looks for another |
| notification subscription times out | setup retries up to three times; keep the iphone unlocked |
| connected, but waiting for an announcement | wait for a peer; try a short iphone nickname and inspect debug logs |
| chat disconnects | quit/restart; live-session recovery isn't implemented yet |
| text is rejected | check the byte limit and the debug rejection reason |

startup retries disconnect only the selected peer, not the adapter, and don't
change pairings. cached devices are ignored until live scan evidence arrives;
missing-service candidates are skipped rather than retried on the same address.
for a configured known phone, direct le can **start waiting before the app is
open** (ctrl-c cancels); startup attempts back off from five to 30 seconds.
it can't launch the iphone app or guarantee ios background advertising. permission failures and unsupported characteristic properties
are not retried. chat messages are **never automatically replayed**.

finding a service uuid isn't enough: wait for a peer announcement. in debug mode,
look for `connected and subscribed` before testing messages. logs can include
public text, nicknames, and peer ids; review them before sharing.

## can i send files from iphone?

not to chatt3r yet. stock bitchat's inspected iphone composer has photo/camera
attachments and voice notes, but chatt3r doesn't receive or save those packets.
there's no `/send` command.

the upstream binary envelope supports filenames, mime types, pdfs, and generic
bytes. that doesn't prove the stock app has an arbitrary-file picker: the ui we
checked has no general files/pdf import control. installed releases may differ.

first we'll reuse the existing media format to receive a small phone attachment.
then add safe saves, consent, integrity checks, and arbitrary-file support.

## update and test

quit your current chat before rebuilding:

```bash
git pull --ff-only
./scripts/chatt3r --build
./scripts/chatt3r
```

if an update introduces dependencies you haven't downloaded, repeat the one-time
online `cargo build --locked` command from setup, then return to offline use.

for development checks, with cargo on your path:

```bash
cargo test --offline --locked --manifest-path desktop/bitchat-terminal/Cargo.toml
cargo clippy --offline --locked --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r -- -D warnings
python3 tests/launcher-smoke.py
python3 desktop/bitchat-terminal/tests/ui-smoke.py
```

currently: 31 passing unit tests, launcher tests with fake tools, and four linux
pty cases for draft redraw, colors, and exit/terminal restoration. two interactive
tests are skipped in the normal unit run and exercised by the pty script.

one unit test also simulates two signed peers exchanging fragmented public text
without bluetooth. it exercises the shared receiver, **not** laptop ↔ laptop
advertising or a real radio link. software tests aren't a substitute for hardware
tests. the repeatable physical
procedure is in [docs/linux-iphone-test.md](docs/linux-iphone-test.md).

## what's next

1. **harden text:** repeated offline tests, saved preferences, reliable reconnects,
   larger-message support, and a cleaner desktop core.
2. **remove the phone dependency:** a different windows pc actually
   advertised, subscribed a central and received a write. our handler
   mistook a momentary empty subscription snapshot for permanent disconnect
   and rejected that write; the new policy awaits a hardware retest. verify
   signed text in both directions in **one simultaneous linux ↔ windows run**.
   linux ↔ linux still needs a working advertising adapter and a linux gatt
   server; local bluez registration fails on this controller. see
   [docs/laptop-to-laptop.md](docs/laptop-to-laptop.md).
3. **add files:** receive a small phone attachment, then verified arbitrary files.
   an iphone fork may need macos/xcode if stock ui can't expose the needed flow.

mesh, speed optimizations, and a gui come later. working direct transfers first.

## repo layout

```text
scripts/chatt3r                     launcher
desktop/bitchat-terminal/            included rust source, not a submodule
  src/bin/chatt3r.rs                 application + ble startup
  src/baseline/protocol.rs           short-text codec and fragmentation
  src/baseline/ui.rs                 line editing and peer colors
  src/baseline/linux_att.rs          opt-in linux direct le/gatt transport
  src/baseline/windows_gatt.rs       experimental native windows gatt host
  src/baseline/host_policy.rs        single-central subscription checks
  test-vectors/                     shared packet fixtures
  tests/ui-smoke.py                  linux terminal tests
tests/launcher-smoke.py              launcher tests without bluetooth
docs/reconnect.md                   practical connection cheat sheet
docs/linux-iphone-test.md            physical test procedure
docs/upstream-analysis.md            source pins, compatibility findings
docs/laptop-to-laptop.md             desktop host plan and evidence (link not proven)
docs/windows.md                      experimental native windows build/run guide
docs/codex-handoff.md                current coding-agent hardware checkpoint
AGENTS.md                            coding-agent guardrails and entry point
context.md                          project requirements and roadmap
```

the linux direct le path is isolated in `desktop/bitchat-terminal/src/baseline/linux_att.rs`.
it bypasses bluez's generic connection **only** when selected explicitly, and
keeps the packet/ui code shared. see [docs/direct-le.md](docs/direct-le.md) for
the architecture and evidence. the legacy `bitchat` binary stays preserved as
reference; build/run `chatt3r`
explicitly or use the launcher. separate upstream checkouts aren't needed to
build. optional references live under ignored `upstream/` locally.

## credits and license

based on [shiloheye/bitchat-terminal](https://github.com/ShilohEye/bitchat-terminal)
and the protocol/ble behavior of
[permissionlesstech/bitchat](https://github.com/permissionlesstech/bitchat).
the desktop upstream history is retained, with analyzed commits documented in
[docs/upstream-analysis.md](docs/upstream-analysis.md).

[unlicense](LICENSE), consistent with the primary public-domain upstreams.
android's gpl source is a behavioral reference only; no android code is copied
or shipped here.
