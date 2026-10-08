# chatt3r progress and plans

updated: 2026-10-08. checkboxes mean the stated evidence exists; software
tests and physical delivery are tracked separately. update this file after
each checkpoint, with a link to the evidence and any remaining limits.

## what we have

- [x] linux ↔ iphone public text confirmed by the user over physical BLE,
  including an offline direct-LE run.
- [x] windows → iphone public text confirmed on native Windows. the reverse
  direction has not been separately confirmed.
- [x] linux ↔ windows public text confirmed both ways: stock service with
  iphone Bluetooth off, and desktop-only service with it on throughout.
  no simultaneous phone-on console transcript was retained.
- [x] opt-in iphone → linux public image receive: four JPEG saves fully
  decode locally (3,485 bytes twice, 39,907 and 44,478 bytes). the user
  visually confirmed the first image. original-byte comparison is pending.
- [x] public text up to 1,024 UTF-8 bytes passes software tests for signing,
  compression, fragmentation, reassembly and both chat receivers. this is
  not yet longer-text radio evidence.
- [x] short-text wire fixtures and file receiver tests pass. text-only MTU
  remains 185; file-opted direct LE requests 517. transport roles are unchanged.
- [x] protocol candidate rebuilt and pushed as `b24f9a6`: 57 Linux unit
  tests, strict Linux/Windows checks, launcher smoke and four terminal PTY
  cases passed. Windows checks establish types, not PC radio delivery.

## current milestone: next five steps

stop at this hardware-validation checkpoint. the detailed commands and
sample generator are in [long-text-checkpoint.md](docs/long-text-checkpoint.md).

1. [ ] start the existing linux ↔ iphone link and confirm short numbered
   text arrives on both screens.
2. [ ] send 100-byte text in both directions; compare the full received
   text. record direction and whether it arrived.
3. [ ] repeat with 256-byte text in both directions.
4. [ ] repeat with 1,024-byte text in both directions; optionally check
   256 copies of `🙂` (also 1,024 UTF-8 bytes).
5. [ ] repeat the short/100/256/1,024-byte checks on native Windows ↔ Linux
   using the existing host/client commands, then update the evidence and
   handoff. do not mark longer-text compatibility complete until observed.

the first run uses the existing saved-phone setting and text-only mode:

```bash
cd /home/paprika/dev/offline-link
./scripts/chatt3r --build
./scripts/chatt3r --name laptop --debug
```

keep the iphone unlocked with BitChat open in its Bluetooth public chat.
after a signed peer announcement, exchange a short message first. generate
the larger samples in a separate terminal using the linked guide.

if a size or direction fails, stop and record that result before continuing.
the main interoperability question is whether Apple's recompression yields
the same signature input as our outgoing compressed text. retain only
sanitized size/type/error metadata in git; no addresses, peer IDs, raw
packets, images or private messages. a GATT write is not a delivery receipt.

## deferred: separate tasks after this checkpoint

these are plans, not approved implementation work in the current milestone.

- [ ] audit zero-argument connectivity separately: eventually, launching
  `chatt3r` with no arguments should discover and connect to nearby
  compatible chatt3r instances without prior OS Bluetooth pairing or manual
  role selection. do not investigate or refactor startup, discovery or BLE
  roles during the current milestone.
- [ ] collect simultaneous sanitized console evidence for the already
  user-confirmed desktop-service exchange with iphone Bluetooth on.
- [ ] explicitly test peer shutdown, out-of-range behavior and restart.
  established-session reconnect and application delivery receipts are not
  implemented; never automatically replay an ambiguously delivered message.
- [ ] collect specific file fragment/compression and encoded-size evidence,
  including the earlier 46-part case, with an agreed test image. compare
  original bytes when available; do not ask for the unavailable first original.
- [ ] establish iphone → windows text separately and Linux ↔ Linux radio
  support. this laptop's adapter rejected the earlier advertising attempt.
- [ ] scope future file sending, broader media compatibility, private
  messaging and multi-device links before implementing them. none is shipped.

## references

- [current engineering handoff](docs/codex-handoff.md)
- [longer-text device checkpoint and protocol data flow](docs/long-text-checkpoint.md)
- [native Windows commands](docs/windows.md)
- [PC transport evidence](docs/laptop-to-laptop.md)
- [long-term goal](context.md)
