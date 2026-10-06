# trying chatt3r on windows

**status: one-subscriber hardware success, not a finished peer.** the user
confirmed **linux ↔ windows pc public text** over BLE with the iphone's
Bluetooth off: Linux received the signed `windows-pc` announcement and the
Windows replies `yo` and `ok got it`, and the user confirmed the exchange.
when the phone was nearby with Bluetooth on, the Windows host reported **two
subscribers** and refused the inbound write. the host now advertises a
**desktop-only service UUID** and Linux opts into it with `--desktop-peer`, so
stock BitChat's filtered service scan should not connect to that host. this
new discovery route is compiled, **not yet hardware-tested with the phone
on**. `--host --stock-host` preserves the original service as an explicit
fallback for the proven phone-off test. phone-off was a test isolation step,
not an intended requirement. no private messages, files, delivery receipts or
live-session reconnect. windows → iphone text also worked on a different pc;
iphone → windows has not been separately confirmed.

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

the central/client worked on one Windows pc. `--host` advertised and
exchanged text with Linux on another pc under one-subscriber test conditions.
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
Windows `--host` now publishes only chatt3r's **desktop service**
`88d5ec18-2621-4233-ad22-82702a601c97`; its characteristic and signed
packet layer are unchanged. stock BitChat scans its own upstream service UUID,
so a separate desktop UUID should avoid the phone's opportunistic subscription
and the Linux scanner should ignore the phone. this isolates discovery,
**not** cryptographic device identity or genuine multi-client hosting.

for the next hardware check, leave phone Bluetooth **on**. if the new
service cannot advertise, the explicit `--stock-host` fallback below preserves
the previous phone-off route; don't alter pairings. on Windows, from your
existing checkout in PowerShell:

```powershell
git pull --ff-only
cargo build --locked --manifest-path .\desktop\bitchat-terminal\Cargo.toml --bin chatt3r
$client = '.\desktop\bitchat-terminal\target\debug\chatt3r.exe'
& $client --host --write-limit 128 --name windows-pc --debug
```

look for `[host] chatt3r desktop service=...` and then
`[host] windows gatt host started ... waiting for one subscriber`.
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
./scripts/chatt3r --desktop-peer --scan-only --scan-seconds 30
./scripts/chatt3r --desktop-peer --debug --scan-seconds 90 --name laptop
```

`--desktop-peer` bypasses this laptop's **local iphone shortcut for just
that command** and filters on the desktop service; it does not erase the
saved phone address or its pairing. the scan-only command is read-only and
exits after 30 seconds. if it finds a **live chatt3r desktop candidate**, run
the final command while the Windows host is still advertising. on windows,
look for `one central subscribed` (this alone does **not** prove which device
connected). the host now waits for a **signed incoming announcement** before
its first notification;
`--debug` then shows the session ATT MTU and capped frame limit. on linux,
look for `connected and subscribed`, peer announcements and text on **both**
terminals. send a short `hi from laptop` and `hi from pc` and record which
arrived. `--debug` may log public message contents and peer ids.

if Windows still reports `subscriber conflict: count=2`, the desktop UUID
alone did not isolate all subscribers: don't guess identities or unpair.
collect simultaneous logs from both devices. the previous stock-service
host needed phone Bluetooth off for a successful two-way text test; the phone
wasn't proven to be the second subscriber. eventually the host still needs
safe multi-subscriber selection, even on the desktop service.
`count=1` with a changed ID is a different failure. if Linux finds multiple
unnamed candidates, correlate its selected peer's signed announcement with
the Windows terminal, not just the scan name.

an earlier Linux attempt failed with ATT `0x11` on an unknown candidate.
Windows later rejected writes with `0x03` during conflicting subscriptions;
neither error alone diagnoses MTU. in the successful stock-service
one-subscriber run, `128`-byte frames and two-way text worked. the host logs
queued/rejected frames without remote addresses or text.

if the **new** desktop service is not visible on this Windows adapter, the
**proven phone-off** link is still available, but both ends must explicitly
select the old service. leave phone Bluetooth off or out of range for this
fallback; don't use it to test phone-on isolation:

```powershell
& $client --host --stock-host --write-limit 128 --name windows-pc --debug
```

```bash
CHATT3R_LE_PEER= ./scripts/chatt3r --debug --scan-seconds 90 --name laptop
```

`--stock-host` has no role in the new desktop-only discovery mode.

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

## pc ↔ linux: proven one link, not robust peer selection

windows `--host` uses native Windows GATT, while Linux scans as a central;
both roles feed the shared signed packet/chat code. two-way public text is
user-confirmed on real BLE hardware **using the earlier stock service with
phone Bluetooth off**. the desktop-only service still needs a hardware test
with the phone on. the one-subscriber host still fails closed if another
central subscribes. Linux-only advertising is still blocked on this laptop's
Realtek controller. see [laptop-to-laptop.md](laptop-to-laptop.md) for evidence
and next steps.
