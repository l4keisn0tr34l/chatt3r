# chatt3r

An offline data link over Bluetooth LE, starting with Linux ↔ iPhone text and
reusing BitChat's protocol. No accounts, cloud, Wi-Fi or Internet required during
local communication.

Project destination: https://github.com/l4keisn0tr34l/chatt3r

## Current status

- Upstream inspected and pinned: [analysis](docs/upstream-analysis.md).
- Legacy `bitchat-terminal` builds but is incompatible with the current iOS protocol.
- A separate `chatt3r` binary in the same source checkout implements a small
  signed-public-text test harness. Legacy client structure is preserved.
- User-confirmed bidirectional Linux ↔ stock iPhone BitChat text over BLE.
  Repeated exchanges with Wi-Fi/cellular disabled still need recorded testing.
- Quiet chat by default, peer-colored nicknames, editable input/history, and
  incoming messages that preserve your draft. `--debug` enables BLE diagnostics.
- Windows, laptop-to-laptop BLE and arbitrary files remain future milestones.

## Build and try

On Ubuntu, install Rust and the build/Bluetooth prerequisites if needed:
`build-essential pkg-config libdbus-1-dev bluez`. Build as a normal user.

```bash
git clone https://github.com/l4keisn0tr34l/chatt3r.git
cd chatt3r
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --locked --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r
cargo test --locked --manifest-path desktop/bitchat-terminal/Cargo.toml
./desktop/bitchat-terminal/target/debug/chatt3r --scan-only --scan-seconds 30
./desktop/bitchat-terminal/target/debug/chatt3r --write-limit 128
```

Keep stock BitChat open on iPhone in its Bluetooth public room. Start with a short
nickname such as `iphone` and short test messages. The write limit is configurable
and **not measured MTU**. Read the [physical test procedure](docs/linux-iphone-test.md)
for limitations and evidence to collect.

Normal interactive mode colors each peer's nickname consistently, uses cyan for
`you`, and supports arrow-key editing and session-only history. Set `NO_COLOR=1`
to disable colors. Piped/redirected output stays plain. Add `--debug` only when
troubleshooting; periodic announcements and sync packets stay quiet otherwise.
To pick up a rebuilt binary, `/quit` the old session and restart it.

This first harness supports public plaintext messages up to 99 UTF-8 bytes.
Signatures do not provide confidentiality or a verified Noise session. No DMs,
files, compression, routing or relaying yet. Do not use it for sensitive data.

## Can I send files from iPhone?

Not to chatt3r yet. Stock BitChat's checked iOS composer can send photos from its
camera/photo-library button and voice notes from its microphone control, but
chatt3r does not receive or save media/file packets. There is no `/send` command.

The upstream binary envelope supports filenames, MIME types, PDFs and generic
bytes, but the inspected iOS composer has no general Files/PDF picker. Installed
app versions may differ. We must implement and test desktop receiving before
phone attachments can become a laptop file-transfer feature.

## Repository layout

- `context.md`: project requirements and staged roadmap.
- `desktop/bitchat-terminal/`: included Rust source fork, not a submodule.
  New harness: `src/bin/chatt3r.rs`, `src/baseline/protocol.rs`, `src/baseline/ui.rs`.
  Its legacy `bitchat` binary remains preserved; run `--bin chatt3r` explicitly.
- `desktop/bitchat-terminal/test-vectors/`: language-neutral packet fixtures.
- `desktop/bitchat-terminal/tests/ui-smoke.py`: Linux PTY tests; run with Python 3
  after building/fetching dependencies, with `cargo` on PATH.
- `docs/`: source pins, findings, and reproducible physical-test instructions.

Builds do not require separate upstream checkouts. Optional references may be
cloned under ignored `upstream/`: official Swift BitChat and GPL-licensed Android
BitChat. No Android source is copied or shipped here.

## License and provenance

[Unlicense](LICENSE), consistent with the primary public-domain upstreams.
Based on [ShilohEye/bitchat-terminal](https://github.com/ShilohEye/bitchat-terminal)
and the protocol/BLE behavior of [permissionlesstech/bitchat](https://github.com/permissionlesstech/bitchat).
The desktop upstream history is retained. Exact analyzed commits and licensing
notes are in [docs/upstream-analysis.md](docs/upstream-analysis.md).
