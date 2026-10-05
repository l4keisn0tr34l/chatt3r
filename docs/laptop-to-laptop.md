# laptop ↔ laptop: remove the phone dependency

status: **native windows GATT server backend implemented, Windows-target
compile-checked; hardware link untested.** `chatt3r --host` advertises the
BitChat service, receives characteristic writes and sends notifications.
Linux can scan/connect as a central. no Windows advertisement has yet been
seen by the laptop; no real two-desktop packet exchange is confirmed. windows →
stock iphone text is user-confirmed; iphone → windows is not separately
confirmed. linux ↔ iphone is user-confirmed bidirectional and offline. a
software-only test covers signed two-peer text, fragmentation, rejection and
deduplication, but it does not use BLE. bluetooth does **not** require an iphone
or manual settings pairing.

## hardware and library check on this linux laptop

- `bluetoothctl show` lists **central** and **peripheral** roles; the bluez
  advertising manager reports ten supported instances. this is a capability
  report, **not** proof that scan + advertise simultaneously work here.
- `/org/bluez/hci0` exports `org.bluez.LEAdvertisingManager1` with
  `RegisterAdvertisement`/`UnregisterAdvertisement` and
  `org.bluez.GattManager1` with `RegisterApplication`/`UnregisterApplication`.
- our btleplug 0.11.8 / bluez-async 0.8.2 path provides client scanning,
  connecting, GATT writes and notifications, but no linux advertising/GATT
  **server** abstraction used by chatt3r. keep the working central and direct
  LE clients; investigate a small BlueZ D-Bus peripheral backend separately.
- **local advertisement probe:** bluetoothctl 5.72 attempted a temporary
  unrelated 128-bit UUID (and then a plain/default advertisement), with no
  BitChat GATT server and no pairing changes. both registrations failed:
  `org.bluez.Error.Failed`; bluetoothd logged `Failed to add advertisement:
  Invalid Parameters (0x0d)`. `ActiveInstances` remained **0** and cleanup
  left it at 0. a second test disconnected **only the iphone's current link**
  (pairing intact) and still failed both plain and custom-uuid registration
  with the same error. a concurrent phone link does not explain the failure.
  this Realtek USB controller reports both roles but has **not** demonstrated
  working LE advertising. try a separately tested adapter or a minimal BlueZ
  D-Bus registration on this one before building the full server. do not
  restart bluetooth or delete bonds just to mask this error.
- **not measured yet:** stable advertisement from windows seen by this laptop,
  inbound GATT write/notify, concurrent scanning, or actual windows peripheral
  capability. native Windows `GattServiceProvider` APIs compile-check for a
  Windows target; that does not prove the PC adapter supports them.

## planned data flow

```text
windows pc advertises bitchat service, hosts notify/write GATT characteristic
       ↓ discover
linux laptop scans and connects as central
       ↓ write/notify over one BLE link
existing bitchat packet validation + peer state + terminal UI on both sides
```

the new windows `--host` path uses `GattServiceProvider` to advertise the
service, owns a notify/write `GattLocalCharacteristic`, and uses a single
ordered worker for incoming writes. it restricts the session to one subscribed
central and uses the existing signed packet/chat validation for text. no
automatic replay after disconnection; provider and event handler are cleaned
up when the host exits. bluetooth device pairing is **not** an app-layer trust
mechanism, and signed packets are not encrypted.

if Windows advertising cannot run on this PC, a linux host would require a
BlueZ D-Bus `LEAdvertisement1` and a `GattService1`/`GattCharacteristic1`
application with an object manager, plus a working advertising adapter. the
local Realtek registration failure remains. do not restart the adapter or
remove a bond as the first diagnostic step.

## duplicate links and role policy (design, not shipped)

both laptops may advertise **and** scan, so two physical links can form at
once. after each side receives a signed peer announcement, compare the full
identity public keys (or another authenticated stable ordering key; the current
8-byte peer id is only a truncated hash). for a pair, prefer the connection
where the lower identity is central and the higher identity is peripheral;
drop the opposite-role duplicate only after the surviving link is ready. a
collision or unresolved identity must fail closed rather than silently choosing
different links on each side. do not automatically replay a user message while
merging links: its first write may already have arrived.

for example: if a < b, A's outbound connection to B wins. B keeps that inbound
peripheral connection; B's outbound and A's corresponding inbound link lose.
when a preferred link fails later, rescan/re-advertise and negotiate again.
separately design peer identity persistence and application delivery receipts
before promising resumed chat or files.

## next proof, in order

1. on Windows, run `chatt3r.exe --host --write-limit 128 --debug`: record
   peripheral-role support, GATT creation and **Started** advertising status.
   stop on a capability or advertisement error; no pairing changes.
2. on linux, disable its local known-iphone shortcut for this command with
   `CHATT3R_LE_PEER= ./scripts/chatt3r --scan-only --scan-seconds 30`. confirm
   an actual BitChat service radio update from the Windows PC. see
   [windows.md](windows.md) for the complete two-machine run.
3. connect Linux as central. check service discovery, characteristic properties,
   notification subscription and a central write plus Windows notification.
   collect both debug logs and verify signed `hi` text on both terminals.
4. test disconnect behavior, more than one central, malformed frames and
   message size limits. the startup retry code must never resend an ambiguous
   message. Windows host should fail closed instead of switching central peers.
5. later: test dual-role adapters/two simultaneous links and implement the
   authenticated stable-identity tie-break above. if Windows cannot advertise,
   investigate an alternate BLE adapter or a Linux BlueZ server with a verified
   advertising controller.

neither the iphone success nor Windows-target compilation proves step 2/3.
do not describe laptop ↔ laptop as working until both machines confirm real
packets without a phone.
