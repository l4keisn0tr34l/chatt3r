# trying chatt3r on windows

**status: experimental, not windows-hardware-tested.** these commands build and
run the current **ble central/client** on native windows. it can *try* to find
an advertising stock bitchat iphone, but nobody has confirmed a windows ↔
iphone chat yet. it cannot advertise a gatt service, so this alone will **not**
connect your windows pc to the linux laptop. please share the exact stage/error
rather than unpairing or assuming a successful scan means chat works.

skip wsl for the first attempt: wsl2 normally doesn't expose the windows
bluetooth adapter as a linux `hci` device. use **powershell on windows itself**.

## one-time setup

1. use windows 10/11 with a bluetooth le adapter; enable bluetooth in windows
   settings. a listed bluetooth device doesn't prove peripheral/advertising
   support — that separate capability is still untested.
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

this windows build has **not** been run here. if it fails, save the first full
compiler error and your windows/bluetooth adapter details; don't call it a
working windows release. `scripts/chatt3r` is a **bash/linux launcher**, not
a powershell script. running the binary directly needs `--write-limit` for
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

## pc ↔ linux is the next, separate feature

right now **both desktops would only scan**. one must advertise the bitchat
service and host a gatt notify/write characteristic. this linux laptop's
realtek controller currently rejects even a temporary test advertisement;
windows may be the better first **peripheral/server** candidate, but support
has not been checked on your pc. a native windows backend would need windows
ble advertising + gatt-server APIs (not the current btleplug central path),
then use the existing shared signed packet/chat code. see
[laptop-to-laptop.md](laptop-to-laptop.md) for the role and duplicate-link plan.
do not call a two-laptop exchange working until it is observed on both machines
without a phone.
