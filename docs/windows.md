# trying chatt3r on windows

**status: experimental.** the user confirmed **windows → iphone** text with
the native ble central/client; iphone → windows has not been separately
confirmed. on a different windows pc, `--host` **did** advertise, get a
subscriber and receive an ATT write, but rejected it after a subscription
change. a later run still rejected a write after a **different or multiple
subscriber** warning; the current code logs the subscriber count to tell
those apart. **no signed pc ↔ linux text has been observed yet**. please share the exact stage/error
rather than unpairing or assuming a successful scan means chat works.

skip wsl for the first attempt: wsl2 normally doesn't expose the windows
bluetooth adapter as a linux `hci` device. use **powershell on windows itself**.

## one-time setup

1. use windows 10/11 with a bluetooth le adapter; enable bluetooth in windows
   settings. a listed bluetooth device doesn't prove peripheral/advertising
   support. one tested pc did reach `advertisement: started`; don't assume a
   different pc/adapter can host GATT.
2. install [git for windows](https://git-scm.com/download/win),
   [rustup for windows](https://rustup.rs), and
   [visual studio build tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   with the **desktop development with c++** workload and windows sdk. use
   rust's `x86_64-pc-windows-msvc` toolchain. downloads need internet once.
3. open a **new** powershell window (so `cargo` is on your path). no admin
   powershell is needed for the build or normal chat.

from powershell, choose a directory you own and run:

```powershell
git clone https://github.com/l4keisn0tr34l/chatt3r.git
Set-Location .\chatt3r
cargo build --locked --manifest-path .\desktop\bitchat-terminal\Cargo.toml --bin chatt3r
$client = '.\desktop\bitchat-terminal\target\debug\chatt3r.exe'
& $client --help
```

the central/client worked on one Windows pc. the `--host` build **ran and
advertised on another**; the new subscription fix is not yet hardware-tested.
if a build fails, save its first full compiler error and adapter details.
`scripts/chatt3r` is a **bash/linux launcher**, not a powershell script. running the binary directly needs `--write-limit` for
chat; `128` below is an operator-selected starting limit, **not a measured mtu**.

## first physical check: windows ↔ iphone

on the iphone, enable bluetooth permission for stock bitchat, open its
**bluetooth public room**, and leave it unlocked with the app in the foreground.
this does not require you to pair the phone in windows settings first.

```powershell
# no connection, writes, or chat packets
& $client --scan-only --scan-seconds 30

# only if scanning found an actual bitchat candidate
& $client --write-limit 128 --debug --scan-seconds 90
```

in chat, wait for `connected and subscribed` and a peer announcement. send a
short non-private `hi` both ways; check **both screens**. a successful windows
GATT write is not an iphone delivery receipt. `/peers`, `/quit`, ctrl-c and
ctrl-d are the supported exit/status shortcuts. `--debug` can include public
text and peer ids; review logs before sharing. maximum text is **99 utf-8
bytes**, not 99 characters; no files, dms or message encryption yet.

**important:** the successful linux `--direct-le <phone-address>` workaround
uses a linux-only l2cap/att socket. it **does not run on windows**. if windows
finds a phone but its generic connection chooses the wrong bluetooth profile,
we need a separate windows-specific fix; don't copy the linux pairing workaround
or assume wsl will provide it.

## laptop ↔ windows pc: try the new host role

both computers need the updated commit. **windows advertises; linux scans.**
keep the iphone out of this test. on windows, from your existing checkout in
powershell (or repeat the clone/build setup above):

```powershell
git pull --ff-only
cargo build --locked --manifest-path .\desktop\bitchat-terminal\Cargo.toml --bin chatt3r
$client = '.\desktop\bitchat-terminal\target\debug\chatt3r.exe'
& $client --host --write-limit 128 --name windows-pc --debug
```

look for `[host] windows gatt host started ... waiting for one subscriber`.
if `GATT service creation failed`, the PC has **not started advertising**:
stop the Linux test. the new host build prints peripheral-role support, the
named Windows `BluetoothError` **and its numeric value**. if it reports
`resource_in_use`, close any previous `chatt3r.exe --host` window and retry
once; don't reset Bluetooth or unpair anything. for `not_supported` or another
error, share the full line, Windows version and adapter model before changing
the implementation. a working central client does not establish GATT-server
support.

once creation succeeds, status lines show `advertisement: started` or a
detailed Windows Bluetooth error. an initial `stopped` is allowed up to 10
seconds while Windows starts
the radio. if you see `started_without_all_advertisement_data`, continue with
the Linux **scan-only** check: it may lack the service UUID and is *not* proof
of discovery. if it aborts or times out, save the **complete** `advertisement`
and `bluetooth` status/error lines plus the adapter model. a working central
client does **not** prove peripheral support; don't change pairings. keep this
process and powershell window open during the next steps.

on the **linux laptop**, from the same updated checkout:

```bash
git pull --ff-only
./scripts/chatt3r --build
CHATT3R_LE_PEER= ./scripts/chatt3r --scan-only --scan-seconds 30
CHATT3R_LE_PEER= ./scripts/chatt3r --debug --scan-seconds 90 --name laptop
```

`CHATT3R_LE_PEER=` disables this laptop's **local iphone shortcut for just
that command**; it does not erase the saved phone address or its pairing.
the scan-only command is read-only and exits after 30 seconds. if it finds a
live candidate with the BitChat service, run the final command while the
windows host is still advertising. on windows, look for `one central
subscribed` (this alone does **not** prove which device connected). the host
now waits for a **signed incoming announcement** before its first notification;
`--debug` then shows the session ATT MTU and capped frame limit. on linux,
look for `connected and subscribed`, peer announcements and text on **both**
terminals. send a short `hi from laptop` and `hi from pc` and record which
arrived. `--debug` may log public message contents and peer ids.

if the windows host starts but linux sees no live service, capture both debug
logs and the adapter model. if linux sees **multiple unnamed candidates**, it
could select the iphone instead of the pc: close the iphone app (no unpairing)
and correlate the Windows console's `one central subscribed` with Linux's
connection before concluding the pc link works. one Linux hardware attempt discovered and subscribed but its
first announcement write returned ATT `0x11`. later Windows runs on another PC reached advertisement `started` and a
subscriber but rejected inbound writes with `0x03`: the first saw an empty
subscriber snapshot; the next saw a **different or multiple** subscriber(s).
these are **not confirmed to be the same Linux link**. the host now prints
`subscriber conflict: count=...` and, if a write follows, match booleans
without displaying device IDs. `count=1` with a changed ID needs a different
fix from `count>1` (another subscribing device). close the iphone app, then
check for `[host] inbound GATT frame queued`
on the next physical run, not just `one central subscribed`. the host logs
reasons but not remote addresses or text. `0x11` can also come from a different
GATT server, so check both consoles before changing the frame size.

if linux sees the service but the generic BlueZ
connection picks classic/audio instead, don't unpair devices: that needs a
separate, explicit LE-only connection test with the **pc's current LE address**,
not the saved iphone address. ask before trying an address so we don't connect
to the wrong device. if a subscription works but writes/notifications fail,
try `--write-limit 36` on **both** machines on a fresh run; `128` was not a
measured negotiation. neither machine resends possibly delivered messages
automatically after a disconnection.

this is public text, not authenticated device pairing or encryption. only
signed peer announcements and signed text are validated at the app layer; no
files or delivery receipts. see [laptop-to-laptop.md](laptop-to-laptop.md).

## if it doesn't build or discover

```powershell
# read-only: identify the adapter and whether windows reports it healthy
Get-PnpDevice -Class Bluetooth | Format-Table Status, FriendlyName -AutoSize

# read-only: toolchain details if compilation fails
rustc --version
cargo --version
```

if `cargo` isn't recognized, reopen powershell after rustup installation. if a
build requires uncached crates, do the one-time online build above; later,
rebuild from cached dependencies with:

```powershell
cargo build --offline --locked --manifest-path .\desktop\bitchat-terminal\Cargo.toml --bin chatt3r
```

no packages are fetched in offline mode. phone/pc BLE messaging itself does not need wifi or internet.

if scanning finds nothing, record the scan output, installed iphone app version,
windows version, and bluetooth adapter model. if it connects but fails at
gatt/subscription, preserve that exact error. don't remove bonds or restart
the adapter as a first response. no automatic live-session reconnect or
message replay is implemented.

## pc ↔ linux remains hardware-unverified

windows `--host` now uses the native windows gatt service provider instead of
btleplug's central path; both roles feed the same signed packet/chat code.
this linux laptop's realtek adapter still rejects **local** advertising, but
it can scan/connect as a central. the other Windows PC has shown an actual
`started` host and inbound write, **not yet a signed message or notification
received on both machines**. see
[laptop-to-laptop.md](laptop-to-laptop.md) for the transport/role plan. do not
call a two-laptop exchange working until packets appear on both machines
without a phone.
