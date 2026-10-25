# 16 kib text and multiline paste checkpoint

status (2026-10-13): **software-tested; phone validation pending**. the user
requested 16 KiB per message and preservation of pasted whitespace. previous
Linux ↔ iPhone radio evidence confirms short text and 100/256/1,024-byte
numbered ASCII both ways; it does not establish the new upper limit.

user correction (2026-10-13): the user attributes the flattened formatting to
stock BitChat rather than chatt3r. record this as an observation about the
installed app, not a verified transport fault or a claim about every BitChat
version. the pinned reference at `5e9287fa` trims outer whitespace in some
paths (`ChatOutgoingCoordinator` local echo and `String+Ext.swift`); its text
formatter retains content slices. no blanket removal of internal newlines was
identified in those inspected paths. chatt3r's exact-whitespace software tests
remain valid; the new 16 KiB radio limit is still unverified.

first 16 KiB outbound check: the user reports no iPhone receipt. two earlier
submissions were rejected by the local 16,384-byte guard; their input sizes
were not recorded. a later numbered-ASCII attempt with the 16,384-byte end
marker was submitted as a 6,430-byte compressed payload over 71 frames at
configured limit 128. the submission completed and incoming announcements
continued. this establishes neither phone acceptance nor displayed receipt;
no link failure is shown. 2/8 KiB results and installed app version remain
unknown. inspect the phone's collapsed-message UI before diagnosing a codec
or assembly failure. the pinned stock public-text receive cap is 128 KiB,
with a 60,000-character sending guard; these do not establish the installed
version's limits or rule out another rejection path.

## diagnostic follow-up

on 2026-10-26 the user resumed this checkpoint without new receipt evidence.
the inspected stock `BLEFragmentHandler`, `BLEFragmentAssemblyBuffer`,
`BinaryProtocol` and `BitchatPacket.toBinaryDataForSigning` paths do not expose
a size/count limit that would explain the recorded 6,430-byte, 71-part message.
this is a source comparison against the pinned reference, not proof that the
installed phone decoded, reassembled or accepted the message. Swift re-encodes
decoded text for signature verification, so a different valid compressor can
still produce an incompatible signature preimage.

the follow-up changes diagnostics and synthetic tests only:

- `[tx]` includes decoded `payload_bytes` as well as wire payload bytes.
- `[tx-progress]` reports submitted frames every 32 parts; `[tx-complete]`
  reports the final count and elapsed milliseconds with `delivery=unconfirmed`.
  neither is an application ACK; elapsed time includes the existing pacing.
- over-limit rejection reports the actual UTF-8 byte count, exposing extra
  pasted characters without trimming or silently discarding them.
- authenticated `[rx-text]` includes wire size and `local_reencode_matches`.
  a false result means local re-encoding differs from that received text; it
  does not reject the incoming message or establish the outbound failure's
  cause. signature verification still authenticates the original wire bytes.

independent Python-zlib fixtures now cover the exact 2/8/16 KiB numbered
samples and a repetitive 16 KiB control. the latter compresses to 70 payload
bytes and two frames, compared with 6,430 bytes and 71 frames for the numbered
sample at configured limit 128. both carry 16,384 decoded bytes.
73 Linux application/library unit tests pass, including a simulated complete
71-frame send, rejection after 32 submitted frames without replay, and a valid
signed message with deliberately different DEFLATE encoding. fourteen PTY
cases, launcher smoke and strict Linux/Windows target checks pass. the binary
is rebuilt. there is no new hardware success or demonstrated root cause.

## next diagnostic check

restart the client with the command below, keeping BitChat foregrounded:

```bash
cd ~/dev/offline-link
CHATT3R_LE_PEER= ./scripts/chatt3r --name laptop --debug --scan-seconds 10
```

1. verify short text and a fresh signed phone announcement.
2. send the repetitive 16 KiB control, then the numbered 16 KiB sample, once
   each. check the iPhone for `END-REPEAT-16384` and `END-16384`, expanding any
   collapsed message. record receipt separately for each sample.
3. retain sanitized `[tx]`, `[tx-progress]` and `[tx-complete]` metadata. if
   either sample arrives, copy it back from the phone and retain `[rx-text]`.
   provide the installed BitChat version and earlier 2/8 KiB results if known.

the exact sample generators live in
`desktop/bitchat-terminal/test-vectors/generate-public-text-long.py`; private
local copies may be recreated in `/tmp`. a two-frame success and 71-frame
failure would narrow the experiment, not uniquely identify compression versus
fragment handling. stop for those physical observations before changing the
working protocol or BLE behavior.

## changes and data flow

`baseline/text_payload.rs:MAX_TEXT_BYTES` now allows **16,384 UTF-8 bytes**,
not 16,384 characters. compression remains reference zlib at level 5 with the
same threshold, diversity gate and versioned size prefix. short messages and
their independent signing fixtures remain unchanged.

`baseline/protocol.rs` permits a text frame up to **16,739 bytes**, including
bounded header, recipient, compression prefix, signature and padding overhead.
text alone permits 1,024 fragments so full-size uncompressed text works at
the tested 64/128/182-byte write limits. control messages retain their prior
99-byte signing subset, frame cap and 256-part bound. assemblies remain capped
at 16 with 30-second expiry. compressed unsigned text fragments retain their
1,024-byte expansion cap; complete text is authenticated before decompression.
file codecs, quotas, MTU selection, discovery and reconnect are unchanged.

the existing Unlicensed `rustyline-async` 0.4.9 source is vendored with a focused
paste/rendering patch; runtime dependency versions are unchanged. bracketed
paste is enabled on terminal open and disabled on drop. `Event::Paste` inserts
the exact clipboard text in one draft; it does not submit any lines or commands.
Enter submits the whole draft through `Input::next_line` and `chat_session`.
large drafts render a screenful around the cursor while retaining all content.
history and incoming output retain the multiline draft. pasted terminal
controls are escaped when rendered, not executed.

`display_message` preserves newlines, tabs and spaces for received and sent
message output; CRLF is displayed as a line break. payload bytes are not
trimmed or normalized before signing/sending. nicknames and other metadata
still use the stricter existing display escaping. Markdown is sent as text.
multiline messages beginning with a slash remain text; exact single-line chat
commands retain their behavior.

## software evidence and limits

- 71 Linux application/library unit tests pass, including both receivers with
  file mode disabled/enabled, exact whitespace, maximum UTF-8 size, oversize
  rejection and independent Python-generated signing/compression fixtures.
- five local terminal-library tests pass; the paste regression checks complete
  content, safe control rendering and the bounded viewport.
- fourteen Linux PTY cases pass, including multiline and full-size paste,
  explicit Enter submission, complete history recall, incoming output during a
  draft, existing room/recovery behavior and terminal/paste-mode restoration.
- launcher smoke and strict Linux/Windows target checks pass; the binary is
  rebuilt. Windows checks validate types, not native terminal or radio behavior.

terminal paste needs bracketed-paste support. piped stdin retains one message
per input line. unusually small write limits can still exceed the bounded
fragment count; full-size delivery may also encounter the assembly deadline
on slow links. no ACK, message queue or automatic replay has been added.

## next physical check

restart the previous client, then run:

```bash
cd ~/dev/offline-link
CHATT3R_LE_PEER= ./scripts/chatt3r --name laptop --debug --scan-seconds 10
```

1. wait for the signed peer announcement and verify short text both ways.
2. paste a harmless multiline message with a blank line, indentation and
   trailing spaces. it must stay one draft until Enter and arrive as one
   received message on the iPhone. note any phone-side formatting changes
   separately; if it looks flattened, copy the received message into an editor
   to distinguish display formatting from stored text. send multiline text back
   and check the laptop display. neither successful writes nor local echo
   establish phone receipt, and flattened phone display alone is not a
   transport-failure diagnosis.
3. test numbered ASCII at 2,048, 8,192 and 16,384 bytes, both ways where the
   stock app permits composing it. check the final marker and one complete
   message. test 16,385 bytes on the laptop: it must reject before transmission.
4. close/reopen the phone link and send fresh short text again in the same
   laptop process. preserve the previously confirmed discovery/recovery path.

use synthetic text and share sizes plus sanitized `[tx]`, `[rx-text]` or `[drop]`
metadata if a case fails. keep device details, raw transcripts and personal
message content local. stop at this physical checkpoint; native Windows tests
remain deferred.
