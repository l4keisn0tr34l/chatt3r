# persistent ordinary discovery on linux

## current checkpoint

the Linux central/client candidate now keeps its room open when a scan window
ends without a peer and returns to discovery after recoverable setup or link
failure. this covers stock BitChat discovery and explicit `--desktop-peer`;
one link is managed at a time. **software-tested, not yet radio-confirmed**.
the separately proven known-phone direct-LE path is preserved.

latest physical comparison (2026-10-13): the saved-phone command established
direct ATT/GATT, received a signed announcement and short text; the user said
the first command works. the bypass scan still reported `observed=1`,
`matching_advertisements=0`, `cached_uuid_matches=0`, `eligible=0` in successive
windows. fresh device events reach the client, but they do not identify the
BitChat service. the observed device is not proven to be the phone.

the current candidate adds **bounded direct-LE service verification** for
fresh addresses lacking the UUID on the ordinary Linux stock-service chat
path. it uses addresses and public/random address types from the current
scan, not a saved-phone setting, device name or existing bond. the scan's
service filter is retained. each probe has a five-second total deadline and
must verify the actual service and notify/write characteristic before
subscribing or sending chat frames. unrelated/incompatible peers are closed;
one is probed at a time and a 512-entry ordering history rotates failed
candidates across windows. cached-only devices are never probed.

the existing `DirectAtt::connect` wrapper still uses the proven public-address
path; `connect_discovered` additionally accepts random LE addresses.
`ClientLink` hands a verified direct link to the same `Room`/`Receiver` as
other transports. direct probes own and close their ATT socket and do not call
BlueZ's profile connect/disconnect on that candidate. socket/configuration
failures remain fatal; remote service rejection, connection failure and probe
timeout return to discovery. `--scan-only`, advertised-UUID candidates,
desktop-service and Windows routes retain their existing connection choices.
the probe fix is software-tested; automatic phone discovery remains unverified.

first physical check (2026-10-13): the user's stock-service scan repeated
ten-second windows and `/peers` returned zero, but no phone candidate appeared.
room persistence is observed; automatic phone discovery and text on this path
are **not confirmed**. this resembles the previously recorded BlueZ hidden-UUID
issue, but the excerpt cannot establish whether any advertisement was forwarded.

the earlier `1dfcf3e` correction retained a matching UUID from a fresh
`ServicesAdvertisement` event when cached peripheral properties omit it.
that evidence is bounded to the current window and the existing 512-address
limit; rejected candidates and cached-only sightings stay ineligible. GATT
properties and a fresh signed peer announcement are still required afterward.
this correction is software-tested, not hardware-confirmed; it cannot restore
advertisement data that BlueZ never forwards.

`--scan-seconds` is the length of each discovery window in Linux chat. a new
window starts when an empty one ends. `--scan-only` still performs one finite
scan and never connects. native Windows client/host lifecycle, service UUID
selection, BLE roles and launcher defaults retain their existing behavior.
no role negotiation, concurrent scanning while connected, or additional-peer
management is added.

## data flow and cleanup

in `desktop/bitchat-terminal/src/bin/chatt3r.rs`, `main` owns one `Room` and
process-level `Receiver`. `Room::operation` polls commands around
`scan_ready_peer`, connection setup and the five-second recovery delay.
setup diagnostics use `ui::Output` so a draft is redrawn rather than overwritten.

each `scan_ready_peer` window obtains a new adapter event stream and
`FreshScan` evidence set. cached UUIDs and synthetic discoveries remain
insufficient; a service match and fresh radio evidence are required. a fresh
matching service event also supplies the service match; its UUID is no longer
discarded in favor of possibly incomplete cached properties. a failed
candidate is rejected for that window. a later window can consider it again
only after fresh evidence. the live evidence bound remains 512 addresses.

`discover` returns a `DiscoveryCandidate`. visible-UUID candidates use the
existing `connect_with_retry`/`prepare_link` path. fresh UUID-less candidates
with an address type use `DirectAtt::connect_discovered` to verify the actual
service and characteristic over LE. `ClientLink` distinguishes the verified
transport; `ScanSession` retains cleanup state and probe ordering. scanning
stops before connection. ready notifications feed the existing
`chat_session`, `LinkWriter`, signed packet codec and `Receiver`.
the peer must provide a fresh valid announcement before text can be submitted.

missing service, exhausted transient setup errors, a disappeared candidate,
and the known BlueZ `br-connection-canceled` setup failure can return to
discovery. on the BlueZ setup path, permissions, authentication failure and
unsupported properties remain fatal. direct probes reject incompatible remote
services/responses but fail on local socket/configuration errors. terminal
failures and adapter/event-stream failures remain fatal. an ended
discovery stream returns an error rather than polling EOF rapidly. automatic
adapter recovery is separate work.

on a lost chat link, keys and partial text/file assemblies are cleared before
disconnect cleanup. duplicate-message history and the per-process file quota
remain. submitted user text is never queued or replayed. `/announce` without
a link reports waiting; `/peers` shows zero. `/quit`, Ctrl-C and Ctrl-D cancel
setup/backoff; scan shutdown and selected-peer disconnect have five-second
cleanup deadlines. cleanup explicitly stops scanning and disconnects a
BlueZ-selected peer because dropping a D-Bus future does not cancel its work.
direct links instead close their own socket and abort their ATT reader on drop.
no adapter resets, bond removal or saved-phone configuration changes occur.

BitChat's packet format and the existing protocol/transport implementations
are reused. the persistent lifecycle, fresh-window orchestration and terminal
ownership are chatt3r work. compression, signing, wire limits and file codecs
are unchanged. a successful BLE write is still not a delivery receipt.

## software evidence and limits

70 Linux unit tests, strict Linux checks, launcher smoke and twelve PTY cases
pass. regressions cover recovery eligibility, permanent-error guards, ended
scan streams, fresh evidence for returning rejected candidates, repeated empty
waiting windows with an intact draft, and all three quit methods. the terminal
tests simulate waiting/transport; they do not exercise BlueZ or a radio.
Windows target check/Clippy validate types only; native Windows tests remain
deferred. no new dependencies are added.
additional regressions cover fresh-only probe eligibility, rejection/ranking,
rotation and bounds, public/random kernel ATT address encoding, and permanent
local socket errors versus remote probe failures.

## next physical gate

quit an old client first. use the saved-phone bypass for this run only, with
file mode off:

```bash
cd ~/dev/offline-link
CHATT3R_LE_PEER= ./scripts/chatt3r --name laptop --debug --scan-seconds 10
```

1. keep BitChat closed for several windows, ideally five minutes. chatt3r must
   keep discovering; `/peers` must report zero and `/announce` must report
   waiting. submitted harmless text must be explicitly rejected, not queued.
2. open BitChat in its Bluetooth public room. expect fresh discovery and setup.
   if connected, wait for its signed announcement and verify short text on
   both screens. a discovery/subscription or successful write is not enough.
3. close the app. if the link ends, expect zero peers and resumed discovery
   in the same process. reopen it and verify fresh text both ways again.
4. test `/quit` while scanning. on separate runs, test Ctrl-C/Ctrl-D during
   scanning or setup; the shell should retain normal editing afterward.
5. separately, `CHATT3R_LE_PEER= ./scripts/chatt3r --scan-only --scan-seconds 10`
   must exit after one window without connecting.

if no candidate appears, share one `[scan-state]` line and whether the usual
`./scripts/chatt3r --name laptop --debug` command still exchanges text with
the configured phone. `observed` counts devices with fresh qualifying events;
`matching_advertisements` counts devices with a matching service event;
`cached_uuid_matches` counts current peripheral property matches, including
cache; `eligible` counts candidates after the freshness/rejection checks,
including unverified addresses eligible only for service probing. it is not
`/peers`, which counts validated packet identities.
zero counts alone cannot distinguish a silent phone from BlueZ filtering it.
the summary contains no device addresses, IDs or packet content.

this laptop previously required direct LE because BlueZ selected the phone's
classic audio profile. ordinary discovery may still find that phone but fail
setup; this candidate keeps searching after the known cancellation error, it
does not solve that profile selection. if this happens, report only the
sanitized stage/error and whether scanning remained active. preserve the
working known-phone path; do not change bonds or infer two-way text success.
the new bounded live-GATT probe reuses direct LE for fresh UUID-less candidates;
there is no hidden fallback to the saved phone, name-based selection, pairing,
adapter reset or BLE role negotiation. if it succeeds, verify short text on
both screens, then app close/reopen and fresh text in the same process. if it
fails, share sanitized `[probe]`/`[scan-state]` lines and whether discovery
continues; automatic phone communication is not established by a probe alone.
stop at this gate before another lifecycle or role milestone.
