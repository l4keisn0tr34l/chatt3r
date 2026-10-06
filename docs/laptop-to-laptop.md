# laptop ↔ laptop: remove the phone dependency

status: **linux ↔ windows pc public text user-confirmed over BLE with phone
Bluetooth off.** Linux received a signed `windows-pc` announcement (two
fragments) and the Windows replies `yo` and `ok got it`; Linux sent `hi` and
`yoooooooo`, and the user confirmed the exchange. there was **no iphone
radio link** in this run. Wi-Fi/cellular switch states were not recorded;
the client used BLE GATT, not an Internet service. this is one physical
one-subscriber session, not proof of robust multi-peer selection or reconnect.

prior Windows runs with phone Bluetooth on rejected inbound writes when the
host saw a transient empty subscriber list and later **count=2** subscribers.
turning off phone Bluetooth allowed this later text exchange; this does not
prove the phone was the second subscriber, only that it changed the test
environment. phone-off is a **temporary isolation workaround**, not the
intended product design. the new **desktop-only BLE service UUID**
`88d5ec18-2621-4233-ad22-82702a601c97` is intended to prevent stock
BitChat's filtered scan from subscribing to the Windows desktop host, while
Linux explicitly opts in with `--desktop-peer`. it is **compile-tested, not
radio-tested** with phone Bluetooth on. Windows `--host --stock-host` keeps
the original stock-service phone-off link available for regression testing.
windows → stock iphone text is also user-confirmed on a Windows central client;
iphone → windows is not separately confirmed.
linux ↔ iphone text is user-confirmed bidirectional and offline. no files,
private chat, relaying or application delivery receipts are shipped.

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
- **observed:** another Windows PC reports peripheral-role support and
  advertisement `started`, and Linux received its signed announcement and
  Windows public text in the phone-off run. the user confirmed two-way text.
  **still missing:** Windows-side transcript for the successful run, a
  repeatable simultaneous test with phone Bluetooth on, a verified
  disconnect/reconnect sequence, and Linux ↔ Linux hosting.

## architecture checkpoint — first phone-free signed text

```text
windows pc advertises a BLE service, hosts notify/write GATT characteristic
  stock BitChat service: proven phone-off; desktop service: current default, untested on radio
       ↓ discover
linux laptop scans and connects as central
       ↓ write/notify over one BLE link
existing bitchat packet validation + peer state + terminal UI on both sides
```

`desktop/bitchat-terminal/src/baseline/windows_gatt.rs` uses native Windows
`GattServiceProvider` to advertise, a `GattLocalCharacteristic` for central
writes and notifications, and an ordered worker to feed the shared receiver.
`desktop/bitchat-terminal/src/bin/chatt3r.rs` routes both the btleplug/Linux
central and Windows-host frames through `LinkWriter`, `send`, `Receiver::receive`
and `chat`. the proven phone-off session used the stock service; the new
`--desktop-peer` mode changes **only discovery/service UUID**, not the
characteristic or packet format. stock BitChat's iOS central scans the stock
service UUID rather than the desktop one; if it still subscribes in a test,
collect logs before further changes. `desktop/bitchat-terminal/src/baseline/protocol.rs`
implements the BitChat-compatible signed packets and fragmentation; the
Windows host is new chatt3r transport code, not an upstream iPhone feature.
Linux's working phone `linux_att.rs` direct-LE backend is unchanged.

**important limitation:** host subscription selection is currently a
one-central policy. a brief empty subscription snapshot gets a short grace
period; a different or additional subscriber triggers a fail-closed exit.
we have **not** authenticated a durable device identity for multi-peer
selection. the current signing identity is ephemeral and does not provide
private/encrypted chat. no possibly delivered user message is automatically
replayed on disconnect. Windows caps notifications at its reported session
ATT MTU minus three bytes; the operator's 128-byte limit is not a measured
Linux MTU. Bluetooth pairings were not reset for these tests.

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

1. **hardware-test the new desktop service:** Windows `--host` advertises
   `88d5ec18-2621-4233-ad22-82702a601c97` by default (the earlier
   stock-service host remains selectable with `--host --stock-host`); Linux
   `--desktop-peer --scan-only` should show it while ignoring the phone's
   stock BitChat service. leave phone Bluetooth **on**, test short text both
   ways, and collect simultaneous Windows/Linux logs. this is a BLE
   discovery split, **not** a security or multi-client guarantee. record
   Wi-Fi switch states separately if a fully isolated offline claim matters.
2. if Windows still sees `count=2`, count/identify WinRT subscribed sessions
   without committing device addresses. eventually select a validated GATT
   session, isolate fragments/notifications by session, and reject other
   clients without tearing down the chosen link. do not automatically
   switch links or replay a message. `count=2` from previous runs does
   **not** prove the iphone was the other client.
3. test Windows `/quit`/Ctrl-C: Linux saw signed `LEAVE`, then a later periodic
   announce hit a BlueZ `WriteValue` method missing after the Windows GATT
   object was removed. likely normal peer shutdown, not an MTU failure; the
   client now reports the disappeared characteristic clearly. live-session
   reconnect and delivery receipts still need a design.
4. Linux ↔ Linux still requires a working advertising controller and a Linux
   GATT server. dual-role symmetric links and identity tie-break above remain
   **planned**, not part of the proven Windows-host/Linux-central pairing.

this milestone is a real phone-free **Linux ↔ Windows** text link. it is not
yet a general multi-device peer network, a file-transfer tool, or a tested
Linux ↔ Linux link.
