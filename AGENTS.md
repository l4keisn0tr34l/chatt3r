# chatt3r agent notes

read [docs/codex-handoff.md](docs/codex-handoff.md) first for the current
engineering state and next physical test. [README.md](README.md) has setup,
[docs/laptop-to-laptop.md](docs/laptop-to-laptop.md) has transport evidence,
and [context.md](context.md) describes the long-term goal, **not** necessarily
what has shipped. prefer observed results and current code over old roadmaps.

- preserve the user-confirmed linux ↔ iphone direct-le text path and the
  windows → iphone central/client path. windows `--host` is experimental; no
  phone-free laptop ↔ pc message has yet been confirmed.
- keep backend-specific BLE code separate from the shared signed packet/chat
  layer. do not replay a possibly delivered user message on reconnect. BLE
  write success is not a delivery receipt.
- never reset adapters, remove bonds, or alter the user's local known-phone
  setting as a routine fix. this laptop has `CHATT3R_LE_PEER` set locally; use
  `CHATT3R_LE_PEER= ./scripts/chatt3r ...` to bypass it **for one PC test**.
- keep real device addresses, exported Pi sessions, credentials, and raw
  sensitive logs out of git. the local untracked `codexguide.md` belongs to the
  user: do not edit, stage, or commit it. it contains older plans; check the
  handoff and actual code for updated status.
- for a coherent architectural milestone, explain the data flow, exact code
  paths, upstream vs new work, evidence and limitations. keep docs and commit
  messages conversational and lowercase. don't label compilation or a GATT
  subscription as working two-way text.
- build/test `--bin chatt3r` from
  `desktop/bitchat-terminal/Cargo.toml` (not the preserved legacy `bitchat`
  binary). run Linux unit tests, launcher smoke and terminal PTY smoke. On a
  Windows Rust target, `cargo check`/Clippy check types only; the PC must
  build/run and test the radio natively. stage only intended files, inspect
  `git diff --cached`, and push tested, clearly labeled changes to `main`.
