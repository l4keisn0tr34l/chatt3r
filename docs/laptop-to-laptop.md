# laptop ↔ laptop: remove the phone dependency

status: **investigation/design, not implemented**. linux ↔ stock iphone public text
is user-confirmed bidirectional and offline. two laptops running the current
client cannot yet discover one another: both only scan/connect as BLE centrals.
bluetooth does **not** inherently require an iphone or manual settings pairing.

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
- **not measured yet:** stable advertisement visible to another linux laptop,
  inbound GATT write/notify, concurrent scanning, or Windows peripheral APIs.

## planned data flow

```text
laptop A advertises bitchat service, hosts notify/write GATT characteristic
       ↓ discover
laptop B scans and connects as central
       ↓ write/notify over one BLE link
existing bitchat packet validation + peer state + terminal UI on both sides
```

bluez will need a registered D-Bus `LEAdvertisement1` object with the bitchat
service uuid and a `GattService1`/`GattCharacteristic1` application with an
object manager. its characteristic must receive central `WriteValue` calls,
track `StartNotify`/`StopNotify`, and publish notifications back to subscribed
centrals. service registration and advertisement lifetimes need explicit
cleanup. whether to use a maintained D-Bus object-server crate or another
peripheral library is undecided until a small working advertisement + write
prototype is tested. no adapter daemon restart or bond removal is part of the
normal path.

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

1. first isolate this adapter's `Invalid Parameters (0x0d)` advertising
   failure (minimal advertising data, controller/driver support, another BLE
   dongle). once local advertising succeeds, register a linux advertisement +
   GATT service with the BitChat UUID using BlueZ D-Bus; prove cleanup.
2. on a **second** linux adapter, verify the actual advertisement and connect
   without an iphone, wifi, hotspot or cable for communication.
3. deliver one write into the server and one notification back. test service
   discovery, characteristic properties and subscription lifecycle.
4. route the existing signed text frames through that backend; test both
   directions, malformed frames and disconnect handling.
5. exercise simultaneous discovery/two-link races; implement the explicit
   tie-break above with physical evidence and tests. then investigate the
   Windows peripheral/server APIs separately.

neither the current iphone success nor this adapter's role list satisfies
step 2. don't describe laptop ↔ laptop as working until both devices confirm
real packets. keep the proof in this document and add an architecture checkpoint
when the peripheral subsystem is actually implemented.
