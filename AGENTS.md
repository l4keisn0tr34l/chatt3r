# chatt3r agent notes

read [docs/codex-handoff.md](docs/codex-handoff.md) first for the current
engineering state and next physical test. [README.md](README.md) has setup,
[docs/laptop-to-laptop.md](docs/laptop-to-laptop.md) has transport evidence,
and [context.md](context.md) describes the long-term goal, **not** necessarily
what has shipped. prefer observed results and current code over old roadmaps.

- known-phone direct LE with `--wait-for-peer` now returns to waiting after
  a write failure, disconnect or notification stream closure with one persistent
  terminal. `/peers`, `/announce` and `/quit` work during setup/backoff; drafts
  and history persist, while submitted text is never queued or replayed. this is
  software-tested; the first app-close hardware check reached recovery but
  exited on setup OS error 38. the candidate retries this errno only after
  a previously successful direct-LE setup and adds socket-stage diagnostics;
  fatal cold-start/configuration/unsupported-protocol guards remain.
  the next physical gate is commands with BitChat closed, app close/reopen and fresh
  short text both ways without restarting the laptop client. see
  `docs/reconnect.md`. preserve per-run file quota and duplicate history,
  require fresh announced keys, discard partial assemblies, and never replay
  user messages. general discovery, Windows recovery and role selection are
  separate follow-up work after this hardware gate. native Windows testing is
  deferred by the user until after this build.
- public text supports **1,024 UTF-8 bytes** in software tests. after the
  reference-zlib level-5 correction in `8699b17`, the user confirmed
  **100/256/1,024-byte numbered ASCII both ways on Linux ↔ iphone**, with
  supporting Linux metadata and explicit phone-side receipt. short text
  also worked. one missing-service startup retry followed by connection is
  observed; established-link reconnect and native Windows longer text remain
  unverified. short text below
  100 bytes retains its previous wire/signing format. announcements/leave
  keep the <=99-byte uncompressed subset, text MTU stays 185, and file bounds
  and opt-in mode stay separate. see `docs/long-text-checkpoint.md`; stop at
  completed phone checkpoint; remaining gates are known-phone reconnect and
  native Windows longer text. zero-argument connectivity was audited but
  remains deferred;
  do not change discovery or BLE roles during this recovery slice.
- preserve user-confirmed linux ↔ iphone direct-le text and windows → iphone
  central/client paths. linux ↔ windows pc **two-way public text** is also
  user-confirmed over BLE with iphone Bluetooth off on the stock service.
  the user also explicitly confirmed **two-way windows ↔ linux public text**
  over the desktop-only service with iphone Bluetooth on throughout. no
  simultaneous phone-on console transcript was retained. Windows
  `--host --stock-host` preserves the proven phone-off service path.
- opt-in `--receive-files <existing-dir>` on a Linux phone link has a
  **software-tested small public file receiver**. the first iphone image
  radio attempt has **no saved file reported**: 182-byte type-`0x20`
  notifications were rejected as truncated or unsupported. the repeat
  recorded stock file fragments of **504 declared vs 182 received bytes**
  (`0x20`, original `0x22`). only opt-in Linux direct-LE file mode now requests
  ATT MTU 517; the user then confirmed negotiated **517 / value 514**, with
  complete 504-byte file fragments on radio. 46 parts exceeded the old
  16 KiB file assembly budget; two other `0x20` frames had compression flag
  `0x04` (their original types were not captured). the opt-in file-only path
  now uses the bounded signed-v2 outer cap (~65 KiB) and can decode bounded
  raw-DEFLATE fragment frames before full outer-signature validation; this
  path then saved a **3,485-byte iPhone JPEG on radio**, verified locally by
  size, SHA-256 matching the receiver output, and full image decode (252×448).
  the user visually confirmed the saved image matches the image sent. no
  original file was available for byte/hash comparison; this run did not record
  fragment flags/count or its negotiated MTU. subsequent user image sends
  left four valid JPEG saves: 3,485 bytes twice (identical hashes), 39,907
  and 44,478 bytes. all fully decode; the two larger images are 336×448 RGB.
  repeated receive and larger decoded content are now observed. encoded
  outer-frame sizes were not retained. the earlier 46-part image and
  compressed-fragment behavior still need specific radio evidence. default text
  stays at 185. no file sending, private files,
  general media compatibility, or receipts.
  keep backend-specific BLE code separate from the shared signed packet/chat
  layer. do not replay a possibly delivered user message on reconnect. BLE
  write success is not a delivery receipt.
- never reset adapters, remove bonds, or alter the user's local known-phone
  setting as a routine fix. this laptop has `CHATT3R_LE_PEER` set locally; use
  `CHATT3R_LE_PEER= ./scripts/chatt3r ...` to bypass it **for one PC test**.
- treat github as a public-facing repo and judge publication automatically.
  publish intended, tested code, relevant tests and synthetic fixtures, and
  accurate setup/protocol/evidence docs useful to users or contributors.
  keep personal working plans, scratch notes, chat context, raw device logs,
  received media and local configuration out of commits. `TODO.md` is a
  local-only progress checklist; never stage or push it. inspect staged
  content for public suitability before every push; explicit user preferences
  take precedence. do not ask for approval file by file or rewrite published
  history without authorization.
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
