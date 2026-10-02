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
- connection drops during chat: quit/restart the client. startup retry isn't
  live-session recovery, and messages are never automatically replayed.
- rejected packet: `--debug` shows the reason. the current codec only supports
  short, uncompressed public text and short announcements.

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
- text is limited to **99 utf-8 bytes**. 500 characters / 500 bytes won't work
  yet. longer outgoing text is rejected; longer incoming text is dropped.
- public chat isn't encrypted. use test text, not secrets.
- phone photos/voice notes and arbitrary files aren't supported by chatt3r yet.
- the script does not need internet to start an already-built client. it builds
  offline if the binary is missing and keeps your terminal attached for editing.
