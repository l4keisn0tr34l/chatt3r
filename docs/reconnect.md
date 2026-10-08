# getting the iphone talking again

if it stops working, don't start unpairing everything. bitchat uses its own ble
connection; normal bluetooth file sharing / manual pairing isn't required.

also: the laptop can't flip the iphone's bluetooth switch for you. the phone
steps below are manual. the script handles starting the laptop client.

## on the iphone

1. turn bluetooth on in **settings → bluetooth**.
2. check **settings → privacy & security → bluetooth → bitchat** is enabled.
3. open bitchat's **bluetooth public room**, not a location/geohash room or dm.
4. leave the app visible and the phone unlocked until the laptop connects.
5. use a short nickname like `iphone` for this early client.

if it's still not showing up: fully close and reopen bitchat, return to the
bluetooth public room, then try a longer scan. no unpairing or bluetooth reset
is needed for this check.

## on the laptop — the usual way

```bash
chatt3r
```

that's it. the launcher supplies the tested 128-byte frame limit. incoming
messages won't mess up the line you're typing.

if your shell hasn't picked up the updated alias yet:

```bash
source ~/.zshrc
chatt3r
```

or skip the alias entirely — this works from any folder on this machine:

```bash
~/dev/offline-link/scripts/chatt3r
```

on another machine, use wherever you cloned the repo instead. the launcher
finds the repo relative to its own location, not your current folder.

## if it's acting weird

```bash
# chat with bluetooth logs. this is not a help flag.
chatt3r --debug --scan-seconds 90

# print the cheat sheet without connecting
chatt3r --help

# check whether your alias points at the launcher
type chatt3r

# inspect the laptop bluetooth adapter; changes nothing
chatt3r --doctor

# only if the laptop adapter says powered: no
bluetoothctl power on

# scan for bitchat without connecting or sending anything
chatt3r --debug --scan-only --scan-seconds 30
```

what to look for:

- no **live** peer found: nothing connected. messages can't arrive yet. check
  the phone steps above; don't treat a message typed on the phone as a send.
  a paired device in `bluetoothctl devices` is not a live bitchat advertisement.
  if the phone *is* advertising but bluez hides its uuid, see direct le below.
- `live bitchat candidate`, but its characteristic is unavailable: bluez may
  remember an old service uuid even when the app's service isn't ready. the
  client skips this address and keeps scanning for another live candidate until
  the scan deadline. reopen the phone app in the foreground. a cached uuid is
  not proof of a working chat link.
- connected, but notification subscription times out: the client now retries
  setup up to three times, disconnecting only that peer between attempts. keep
  the iphone unlocked. this does not restart bluetooth or change pairings.
- `connected and subscribed`, then `iphone is nearby`: the link is ready. try
  `hi` in both directions before anything longer.
- connection drops during chat: direct LE with `--wait-for-peer` returns to
  waiting for the same configured phone. ordinary scan/client and Windows
  host paths still require restarting. messages are never automatically replayed.
- rejected packet: `--debug` shows the reason. public text up to 1,024 UTF-8
  bytes is software-tested; phone 100/256/1,024-byte numbered ASCII has been
  confirmed both ways. native Windows longer text remains pending.

## if bluez picks the phone's audio profile instead of bitchat le

on some paired dual-mode iphones, the radio advertises bitchat but bluez's
cached device profile omits its uuid. bluez's generic `connect()` may then try
classic hands-free/audio and immediately fail. this is **not** fixed by pairing
again; that just changes the cache temporarily.

first identify **your own phone's** public identity address with
`bluetoothctl devices` and `bluetoothctl info <address>`. with the phone nearby
and bitchat foregrounded, substitute that address (do not literally copy this
example):

```bash
chatt3r --direct-le AA:BB:CC:DD:EE:FF --debug
```

this linux-only path opens a direct le att socket, verifies the bitchat gatt
service and notify/write characteristic, subscribes, then uses the **same**
packet validator and terminal chat. it does not unpair, reset bluetooth, or
switch on a file-transfer feature. the 128-byte frame limit is capped against
the negotiated att mtu. two-way short text is user-confirmed on physical
devices. only use an address you identified; it does **not** search every
nearby phone. add `--wait-for-peer` to keep waiting after a lost link; without
it this explicit command still exits on link failure. recovery is currently
software-tested and needs the device checkpoint below.

once you've confirmed text in both directions, you can make the plain launcher
use that phone without publishing your address. put this in **your own**
`~/.zshrc`, substituting your address, then `source ~/.zshrc`:

```bash
export CHATT3R_LE_PEER=AA:BB:CC:DD:EE:FF
```

`chatt3r` will use direct le **and wait for the phone app to become ready**;
`chatt3r --debug` shows each setup attempt. you can start it before opening
bitchat; `/peers`, `/announce` and `/quit` are available while waiting, and
Ctrl-C/Ctrl-D also exit. the client backs off from five to 30 seconds between
failed startup attempts. after a write failure, disconnect or notification
stream closure, it releases the old connection and returns to waiting with
the same bounded backoff. it sends fresh presence on the next connection;
user messages and incomplete files are not replayed. fresh peer announcements
are required, while duplicate-message history and the per-run file quota
remain bounded across reconnects. the same terminal, unfinished draft and
in-memory history remain open across link changes. `/peers` shows zero after
a lost link. `/announce` without a link reports waiting; it does not advertise
or send a packet. text submitted while waiting or during another BLE write
is rejected with an explicit response, never queued. it cannot launch the ios app or promise
background advertising. permission, configuration and unsupported-adapter
failures still exit rather than retrying forever.
after a successful direct-LE setup, OS error 38 (`ENOSYS`) during a later
setup is retried with the same backoff. on cold startup it still exits.
Linux Bluetooth can use this errno for an unmapped connection status;
it does not always mean an absent syscall. socket setup diagnostics now
include their stage and retain the underlying errno.
`--scan-only`, `--doctor`, `--build`, and an explicit `--direct-le` still do what
you ask. without the local setting, opt in with `--direct-le <address>
--wait-for-peer`. this setting is only on your machine, not in the git repo.

## known-phone recovery: device checkpoint

status (2026-10-09): the first physical app-close check **failed**. the client
received a signed LEAVE, detected notification closure and entered its
five-second retry, then exited on OS error 38. the correction below passes
software tests; physical close/reopen reconnection is still unconfirmed.

`retryable_direct_startup` now receives whether an earlier setup succeeded
in this process. only then can `ENOSYS` retry in opted-in wait mode. cold
startup `ENOSYS`, permissions, invalid configuration/data, missing/down
adapters and genuinely unsupported protocols remain fatal. no message replay,
adapter reset, bond change, protocol change or discovery/role change is added.
the persistent-room candidate also keeps commands available before setup and
through backoff. 61 unit tests, strict Linux/Windows checks, launcher smoke and nine PTY cases
pass; the binary is rebuilt. regression tests cover retry classification for
raw/contextual error 38 after a successful setup and permanent-error guards,
not a physical reconnection. PTY tests exercise the actual terminal and chat
session with simulated BLE, including draft preservation, no replay and
cancellation during a blocked write.

Linux's [`bt_to_errno`](https://github.com/torvalds/linux/blob/master/net/bluetooth/lib.c)
maps unknown Bluetooth status codes to `ENOSYS`. the original transcript did
not identify the socket stage or controller status, so this is a possible
kernel explanation, not a captured root cause. the new stage diagnostics
will distinguish socket creation/bind/readiness from connection completion.
use the existing local saved-phone setting, with file mode off for this first
check. quit any old client first, then:

```bash
./scripts/chatt3r --build
./scripts/chatt3r --name laptop --debug
```

1. start with BitChat closed. at `you>` try `/peers` (expect zero), `/announce`
   (expect waiting) and harmless text (expect not sent or queued). the client
   must remain open. then open BitChat on the unlocked iphone in its Bluetooth
   public room. wait for a signed peer
   announcement, exchange short numbered text both ways and check both screens.
2. close the iphone app, leaving the laptop process running. when the phone
   link ends, expect a waiting/retry message rather than a shell prompt.
   `/peers` must show zero and `/announce` must report waiting. keep the process open.
3. reopen BitChat in its Bluetooth public chat. expect a new connection and
   signed announcement; exchange new numbered text both ways without restarting
   the laptop client. allow for the five-to-30-second retry delay and setup time.
4. test `/quit` while connected. on separate runs test `/quit`, Ctrl-C and
   Ctrl-D while waiting; each should return to the shell with normal editing.

if closing the app does not end the link, record that observation rather than
claiming a reconnect. an out-of-range/return test can establish link recovery
separately. share only sanitized status/error lines and whether text arrived;
no device addresses, peer IDs, private text or full transcripts. check the
other screen before resending any message whose delivery was uncertain.
phone 100/256/1,024-byte numbered text is already confirmed both ways in
[the longer-text checkpoint](long-text-checkpoint.md); it does not need to
be established again to test this lifecycle fix. native Windows longer text
remains a separate physical gate.

## commands inside chat

these go at the `you>` prompt, not in your shell:

```text
/peers
/announce
/quit
```

`/announce` resends your presence, not your last message. ctrl-c / ctrl-d also
exit. `chatt3r --debug` belongs in your shell after exiting the chat.

## a couple more shortcuts

```bash
# use your own nickname (max 24 utf-8 bytes)
chatt3r --name laptop

# turn off colors, same chat behavior
NO_COLOR=1 chatt3r

# manual adapter checks, if you prefer those over --doctor
bluetoothctl list
bluetoothctl show
```

## update / rebuild

quit the running chat first. from the local checkout:

```bash
cd ~/dev/offline-link
git pull --ff-only
./scripts/chatt3r --build
./scripts/chatt3r
```

`--build` is offline and build-only. it uses cached dependencies and never
starts chat. if rust isn't installed or dependencies aren't cached yet, do the
one-time setup/build from the readme while internet is available.

## alias, if you need it again

this line is already in this machine's `~/.zshrc`. on a new setup, put it there
once, adjusting the path if needed:

```bash
alias chatt3r='"$HOME/dev/offline-link/scripts/chatt3r"'
```

then:

```bash
source ~/.zshrc
chatt3r --help
```

## quick reminders

- 128 is the outgoing frame limit, including protocol overhead. it isn't a
  measured mtu, and it isn't the text-length limit.
- the software text cap is **1,024 utf-8 bytes**; previously confirmed text
  was the <=99-byte subset. **100/256/1,024-byte numbered ASCII is now
  confirmed both ways on Linux ↔ iphone**; native Windows longer text
  remains unverified. see [long-text-checkpoint.md](long-text-checkpoint.md).
- public chat isn't encrypted. use test text, not secrets.
- four iphone JPEG receives are verified in opt-in file mode. file sending,
  private media and general media compatibility are not implemented.
- the script does not need internet to start an already-built client. it builds
  offline if the binary is missing and keeps your terminal attached for editing.
