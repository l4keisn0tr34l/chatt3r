# direct le on linux: why it exists

## architecture checkpoint — dual-mode phone recovery

**what changed:** an explicit `--direct-le <phone-address>` transport bypasses
bluez's generic paired-device `connect()` for linux ↔ iphone. optional
`--wait-for-peer` retries *startup setup only* with a five-to-30-second
backoff so chat can be launched before the iphone app is foregrounded. normal bluez
scanning/chat remains the default. neither path unpairs, restarts bluetooth, or
replays a chat message. this is a direct link, **not** a live-session resume.

**why it exists:** on this laptop, an unlocked iphone running app store bitchat
v1.7.1 advertised the **mainnet** service uuid on a connectable le address.
a one-time privileged, read-only hci capture proved the bytes reached the
adapter. bluez mapped the resolvable private address to a known paired **public**
phone identity, but its device profile omitted the bitchat uuid. a targeted
`org.bluez.Device1.Connect()` attempt then failed with `br-connection-canceled`;
bluetoothd logged hands-free/audio profile failures. continuing the le scan or
holding a direct le socket open did not change that choice.

`gatttool -t public --primary` (diagnostic only) forced le to the same paired
identity and found the bitchat service. read-only characteristic discovery
found the notify/write uuid too. no bond removal was required. the hci trace
and device addresses were inspected locally; raw captures were deleted.

**data flow:**

```text
iphone advertises mainnet bitchat service
    -> linux direct le l2cap socket, att cid 4 (no bluez profile auto-connect)
    -> att mtu exchange + service/characteristic/cccd discovery
    -> subscribe; send/write with response
    -> existing signed bitchat frames and fragment reassembly
    -> existing peer validation and terminal ui
```

**important code:**

- `desktop/bitchat-terminal/src/baseline/linux_att.rs`: `DirectAtt::connect`
  opens the socket, negotiates mtu and discovers gatt handles; `request`
  serializes responses while the reader routes notifications separately.
  `incoming_att_request` rejects the iphone's unrelated writes to our
  nonexistent local gatt server instead of misclassifying them as our replies.
  `Drop` closes the le socket on exit/error.
- `desktop/bitchat-terminal/src/bin/chatt3r.rs`: `LinkWriter` isolates the
  backend's write/connected checks; both paths call the **same** `chat`,
  `Receiver::receive`, packet codec and `ui`. `--direct-le` requires an explicit
  known public address; the normal bluez path is untouched.
- `scripts/chatt3r`: supplies the operator's default 128-byte write limit.
  direct le additionally caps frames to negotiated att mtu minus three bytes.

**upstream versus chatt3r:** bitchat's service/characteristic identifiers,
packets and fragmentation are reused. linux l2cap/att recovery and the
backend boundary are chatt3r-specific. this does not implement a peripheral
or advertiser on the laptop; laptop ↔ laptop still needs that separate role.

**decision:** keep bluez/btleplug as default. alternatives considered: unpairing
(erases state and only masks the cache problem), connecting every nearby apple
device (unsafe selection), `ConnectProfile` (reported profile unavailable), or
using the deprecated `gatttool` subprocess as a production chat backend
(fragile process and notification handling). chosen: a small linux-native
att socket behind an explicit address option. tradeoff: this path currently
assumes a public paired identity address and must implement its own limited
gatt discovery/mtu/notification handling. it is not portable to windows.

**easy to misunderstand:** bluez's `connected: yes` or the terminal's `[you]`
are not delivery receipts. a signed peer announce proves a packet was received
and verified; the separate user observation of iphone-screen text is the
evidence for outgoing message delivery.
the user confirmed `android` is their iphone's chosen BitChat nickname. no
message is replayed after disconnect.

**evidence and next checkpoint:** offline unit tests and clippy pass; launcher
and pty smoke tests pass. a physical direct le probe negotiated mtu 185,
verified the bitchat gatt characteristic, subscribed, sent an announcement,
and received repeated signed `android` peer announcements; `/peers` listed it,
`/quit` exited cleanly, and bluez eventually showed disconnected while still
paired. the local launcher can use `CHATT3R_LE_PEER` for a one-word command;
that address is kept in the operator's shell config, not committed. a later
user terminal log shows `[you] yo`, a signed `[android] yo works right` reply,
and `[you] yea`. the user subsequently confirmed **both directions work,
including a fully offline iphone run**. this is user-confirmed physical
bidirectional text on the new backend, not merely a completed GATT write.
exact radio-switch settings and repetitions have not been recorded.
out-of-range reconnect still requires restarting the client; the next
milestone is a no-unpairing range/reconnect test. with the iphone app closed,
a physical `--wait-for-peer` probe reported missing BitChat GATT service and
kept retrying without sending chat packets; it did not unpair or restart the
adapter. the **closed app → reopened app → connected** transition still needs
a user-run check. the wait does not wake an ios app or guarantee its background
advertising; ctrl-c cancels the wait.

**what to understand before proceeding:** roles and pairing are separate. the
iphone can advertise and accept a temporary le/gatt link without an ios
settings pairing. a paired dual-mode identity can confuse bluez's generic
classic-versus-le connection choice; the direct backend requests le explicitly.
this does not make linux advertise. desktop ↔ desktop work still requires a
separate bluez advertising/gatt-server backend and role coordination, as
outlined in `codexguide.md`.
