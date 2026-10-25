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

## reverse success and paragraph return failure

the next user test on 2026-10-26 established authenticated iPhone → Linux
delivery of **16,383 UTF-8 bytes** of prose. `[rx-text]` reports **1,153 wire
payload bytes**, `compressed=true` and `local_reencode_matches=true`.
the receiver verifies before displaying this line. local recompression matched
the actual phone encoding for this message, including its compression state;
that finding is specific to this payload/version and does not establish a
canonical match for every input. the user-provided prose is kept out of git.

the user then pasted the same paragraph into the laptop and explicitly reported
no phone receipt after **14/14 frames in 1,329 ms**. the outgoing size line was
not retained, so byte identity of the copy is not independently established.
a further exact 16,384-byte repetitive sample submitted **2/2 frames in 179 ms**
and was also reported absent. the observed link retained ATT MTU **185**,
value limit **182**, configured writes **128**. phone → Linux at exactly
16,384 bytes, large Linux → phone acceptance, and the old rejected reverse
packet's size remain unverified. no compression/signature/size policy is changed.

## next single frame check

the repetitive control has **70 wire payload bytes** and a **156-byte signed
packet** (22-byte v1 header/sender plus 64-byte signature). current configured
limit 128 forces two fragments even though the observed negotiated link can
carry 182 value bytes. the existing encoder returns the complete packet
unchanged when it fits; at configured limit 182 this control uses one write.
the direct-LE backend still clamps the operator limit to the negotiated value.
the synthetic fixture and source establish the expected shape, not receipt.

stop the old client and run this one-command override:

```bash
cd ~/dev/offline-link
CHATT3R_LE_PEER= ./scripts/chatt3r --name laptop --debug --scan-seconds 10 --write-limit 182
```

after a fresh signed announcement and short-text check, send the exact
repetitive control once. expect `[tx]` to show `payload_bytes=16384`,
`wire_payload_bytes=70`, `frames=1` and `configured_limit=182` on a link with
the previously observed MTU. explicitly check for the full ending marker on
the phone and retain only sanitized metadata. this is an existing CLI override;
launcher defaults, requested MTU, discovery/roles and saved settings are unchanged.
it removes fragmentation from this control and is not a demonstrated fix.
stop for physical receipt evidence before changing protocol behavior.

## two-sample physical result

on 2026-10-26, the user tested the diagnostic build and explicitly reported
neither 16 KiB sample received on iPhone:

| decoded bytes | compressed payload bytes | submitted frames | elapsed | phone receipt |
| ---: | ---: | ---: | ---: | --- |
| 16,384 repetitive | 70 | 2/2 | 268 ms | user reports absent |
| 16,384 numbered | 6,430 | 71/71 | 6,598 ms | user reports absent |

the user confirmed a three-byte short message arrived in that same session.
announcements continued; no link error is shown. a separate terminal input
during the 71-frame write was rejected without cancelling that write. neither
completion line is a receipt. a long fragment train alone cannot explain the
two-frame failure, but the observations do not identify a unique cause.

the reverse check reached a final fragment labelled `6/7`, original type
`0x02`, then dropped the assembled text with `invalid compressed text bounds`.
this rejection occurs before outer-message signature verification. the old
diagnostic described the last fragment rather than the rejected inner packet,
so it does not establish the inner size declaration or which bound failed.
an over-limit copy and an encoding problem remain distinguishable possibilities.

the version was recovered from the earlier notes: the user previously identified
app store **v1.7.1**, recorded in `upstream-analysis.md`. the user expects that
same version, but no fresh app-version inspection was made. local release tag
`v1.7.1` resolves to `9edb7c26ef7bdcf3bb29e7907b38997f8d5cd0fa`.
its `BinaryProtocol` uses the framed-file ceiling for decoded bounds,
`BLEFragmentAssemblyBuffer` permits up to 10,000 parts, and its text composer
guard is 60,000 characters. no apparent 16 KiB cap was found in these inspected
paths. the newer pinned source's 128 KiB per-type receive cap is not the
v1.7.1 implementation. compression and signing helpers match the inspected
reference, which does not prove byte-for-byte Apple compression compatibility.

## next reverse size check

the follow-up diagnostic retains a bounded metadata string when final
`Reassembler::accept` decoding fails. `[rx-assembled-shape]` reports the
rejected inner header, including untrusted `declared_text_bytes`, `text_cap`
and `compressed_bytes`. `original_size` now distinguishes empty declarations,
declared decoded sizes above 16,384, and impossible expansion ratios. acceptance
conditions, allocations and limits are unchanged; no raw packet data is saved.

75 Linux unit tests cover the bounds distinctions and v1/v2 reassembled error
metadata without content/identifiers, including consuming/clearing that metadata.
fourteen PTY cases, launcher smoke and strict Linux/Windows target checks pass;
the client is rebuilt. native Windows radio tests remain deferred.

restart the laptop client using the command below, then repeat the same
iPhone → laptop paste once. share only `[rx-assembled-shape]`, `[drop]`, or
`[rx-text]` metadata. keep any copied sender/header text separate from the
message, and do not increase the 16 KiB budget to bypass the rejection.
if the declared size is above the cap, retry a smaller exact copy; if it is
within the cap, use the specific error/comparison to choose the next experiment.
stop for this hardware evidence before changing compression or BLE behavior.

```bash
cd ~/dev/offline-link
CHATT3R_LE_PEER= ./scripts/chatt3r --name laptop --debug --scan-seconds 10
```

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
