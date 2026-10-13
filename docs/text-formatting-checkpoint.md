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
