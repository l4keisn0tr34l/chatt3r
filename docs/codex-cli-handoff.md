# handoff to codex-cli: repeat the first saved iphone image

start in the repository root. give codex-cli this prompt:

> read `AGENTS.md`, `docs/codex-cli-handoff.md` and `docs/codex-handoff.md` first. continue the opt-in, receive-only stock iphone → linux public-file test. preserve the working text and pc paths, use only sanitized phone observations, and never treat a gatt write or a software test as proof of file delivery. do not reset bluetooth or remove bonds. do not edit or stage my untracked `codexguide.md`.

## where this stands

- user-confirmed linux ↔ iphone and linux ↔ windows public text must remain intact. text-only direct le requests mtu 185; `--receive-files <existing-dir>` alone requests 517 and fails if the negotiated att value limit is below 504.
- on the latest **physical** image attempt (2026-10-07), after rebuilding, the user reported one saved **3,485-byte iPhone JPEG** following a signed announcement. local checks verified the size, SHA-256 matching the receiver output, mode **0600**, and full **252×448 RGB JPEG** decode. the user has no original image saved for comparison. this excerpt contains 458/328/80/168/504/431-byte `0x20` values but no negotiated MTU, flags or fragment counts; no local path, peer ID or image bytes belong in git.
- the earlier attempt negotiated mtu **517 / value limit 514** and received intact **504-byte** `0x22`-marked fragments, but its **46-part** stream hit the old **16 kib** collector limit around part 36. two other complete `0x20` frames had compression flag `0x04`; their expanded original types are **unknown**. no file was saved on that earlier run. at mtu 185, 504-byte fragments had arrived truncated to 182 bytes.
- `baseline/file_fragments.rs` now shares the `baseline/file_wire.rs` signed-outer-frame ceiling (about 65 kib), retaining at most 8 pending assemblies, 256 fragments, and 30-second expiry. opt-in `decode_file_candidate` handles bounded raw-deflate version-1 compressed fragment packets, recognizing only an expanded file marker `0x22`. **the small JPEG receive is confirmed on radio; >16 kib assembly and compressed fragments still have only software evidence**. `baseline/file_wire.rs` requires a verified announced key and valid signed public v2 outer file *before* its payload is decoded or written. `baseline/file_store.rs` enforces 64 kib content, mime/magic checks, random `.bin` filename, existing directory and per-run quota. `src/bin/chatt3r.rs:Receiver::receive` routes only the opted-in file candidates; default text handling is unchanged. no file sender, private files, generic media support or application delivery receipts have been added.
- the file TLV, outer v2 signature layout and `0x20` fragmentation are stock BitChat-compatible references (see `docs/file-transfer-audit.md` and source pins in `docs/upstream-reuse-audit.md`). the bounded receive-only collector and linux direct-att client are chatt3r work. do not copy GPL/AGPL reference code without a license decision.

## one physical checkpoint for the user

the first small image is already saved and locally verified; do not restart that claim from zero or ask for the unavailable original again. the next checkpoint is a controlled repeat for repeatability and known sender-byte comparison when available. ask the user to rebuild first (the launcher otherwise reuses an older binary) and send **one harmless small public image** only with explicit consent. the user, not codex, controls the phone, bluetooth state and chosen test directory:

```bash
mkdir -m 700 -p "$HOME/chatt3r-file-test"
./scripts/chatt3r --build
./scripts/chatt3r --name laptop --debug --receive-files "$HOME/chatt3r-file-test"
```

wait for a signed iphone announcement, then send the image from the phone's **public** room; never request an image in a private channel. ask for the negotiated mtu/value limit, whether the `.bin` was saved, its byte count and sha-256, a comparison with known sender bytes when available, and at most a few **redacted**, metadata-only `[rx-shape]` / `[drop]` lines if there was a failure. do **not** ask for a full console transcript, an address, peer id, raw media or packet bytes. mtu alone, a write, or a gatt subscription is not delivery. if the value limit is below 504, stop file mode and use the unchanged text mode; `--write-limit` affects outbound frames only. no adapter reset, bond removal, peer-setting changes, or automatic replay of possibly delivered text.

if a repeat fails, isolate the first failing layer: att value length → fragment flags/shape and reassembly → complete signed v2 outer file → announced-key verification → bounded inflate/tlv → mime/magic and store. preserve size/signature rejection rather than accepting incomplete data. update `docs/codex-handoff.md`, `docs/file-transfer-audit.md`, and `docs/linux-iphone-test.md` with **observed** results. distinguish a saved, locally verified file from equality with original sender bytes; the first result establishes the former only. retain specific >16 kib and compressed-fragment radio checks as separate work.

## checked locally before handoff

linux `cargo test --offline --locked --manifest-path desktop/bitchat-terminal/Cargo.toml --bin chatt3r --lib` and strict clippy passed. tests include synthetic 46-part collection, a 47-part signed public file saved and byte-compared, and bounded compressed-fragment decode; these are **not** radio tests. the `x86_64-pc-windows-gnu` cargo check and clippy passed (type checks only). `python3 tests/launcher-smoke.py` and `python3 desktop/bitchat-terminal/tests/ui-smoke.py` passed. build/test the **chatt3r** binary, not the preserved legacy `bitchat` binary. stage only intended paths, inspect `git diff --cached`, keep sensitive logs and `codexguide.md` untracked, and push clearly labeled tested changes to `main` if repo instructions still call for it.
