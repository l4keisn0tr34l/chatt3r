#!/usr/bin/env python3
"""Linux PTY smoke tests: no Bluetooth connection or phone required."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import termios
import time

crate = Path(__file__).resolve().parents[1]
build = subprocess.run(
    ["cargo", "test", "--offline", "--locked", "--manifest-path", str(crate / "Cargo.toml"),
     "--bin", "chatt3r", "--no-run", "--message-format=json"],
    check=True, capture_output=True, text=True,
)
records = [json.loads(line) for line in build.stdout.splitlines() if line.startswith("{")]
binary = next(r["executable"] for r in records
              if r.get("reason") == "compiler-artifact" and r.get("executable"))


def run_case(test, *, no_color=False, exit_key=None, module="ui::tests", recovery=False, room=False):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    before = termios.tcgetattr(slave)
    env = dict(os.environ, TERM="xterm-256color")
    env.pop("NO_COLOR", None)
    if no_color:
        env["NO_COLOR"] = "1"
    proc = subprocess.Popen(
        [binary, "--exact", f"{module}::{test}", "--ignored", "--nocapture", "--test-threads=1"],
        stdin=slave, stdout=slave, stderr=slave, env=env,
    )
    data = b""
    typed = completed = False
    stage = 0
    steps = [
        (b"test: initial waiting", b"/peers\r"),
        (b"0 nearby peers", b"/announce\r"),
        (b"nothing announced", b"offline text\r"),
        (b"message was not sent or queued", b"/quit\r"),
        (b"iphone is nearby", b"persistent dra"),
        (b"test: link lost; same room waiting", b"ft\r"),
        (b"Waiting for a phone link; message was not sent or queued", b"/peers\r"),
        (b"0 nearby peers", b"/announce\r"),
        (b"nothing announced", b"/quit\r"),
        (b"iphone is nearby", b"/peers\r"),
        (b"1 nearby peers", b"fresh text\r"),
        (b"[you] fresh text", b"/quit\r"),
        (b"test: blocked write", b"/peers\r"),
        (b"1 nearby peers", b"/announce\r"),
        (b"Will announce after", b"busy text\r"),
        (b"BLE write in progress; message was not sent or queued", exit_key),
    ]
    offset = 0
    deadline = time.monotonic() + 10
    try:
        while time.monotonic() < deadline:
            readable, _, _ = select.select([master], [], [], 0.1)
            if readable:
                data += os.read(master, 65536)
            if room:
                if stage < len(steps):
                    marker, command = steps[stage]
                    found = data.find(marker, offset)
                    if found >= 0:
                        os.write(master, command)
                        offset = found + len(marker)
                        stage += 1
                if proc.poll() is not None:
                    break
                continue
            prompt_data = data
            if recovery:
                marker = b"stream closed; starting fresh session"
                prompt_data = data.split(marker, 1)[1] if marker in data else b""
            if b"you> " in prompt_data and not typed:
                os.write(master, exit_key if exit_key is not None else b"dra")
                typed = True
            if exit_key is None and b"incoming during draft" in data and typed and not completed:
                os.write(master, b"ft text\r")
                completed = True
            if proc.poll() is not None:
                break
        if proc.poll() is None:
            raise RuntimeError(f"PTY test timed out: {data!r}")
        assert proc.returncode == 0, data
        assert termios.tcgetattr(slave) == before, "terminal mode not restored"
        if room:
            assert stage == len(steps), data
            assert b"persistent draft" in data, data
            assert b"test: persistent room complete" in data, data
            return
        if recovery:
            assert b"stream closed; starting fresh session" in data, data
            assert typed, "fresh session did not receive the exit key"
        if exit_key is None:
            assert b"incoming during draft" in data, data
            assert completed, "test did not send the full draft"
            assert (b"\x1b[1;32miphone\x1b[0m" in data) == (not no_color), data
        if no_color:
            assert b"\x1b[1;36m" not in data, data
    finally:
        if proc.poll() is None:
            proc.kill()
        proc.wait()
        os.close(master)
        os.close(slave)


run_case("incoming_output_preserves_draft")
run_case("incoming_output_preserves_draft", no_color=True)
run_case("exit_event_returns_none", exit_key=b"\x03")
run_case("exit_event_returns_none", exit_key=b"\x04")
run_case("stream_closure_restores_terminal_then_fresh_session_exits",
         module="tests", recovery=True, exit_key=b"\x03")
run_case("stream_closure_restores_terminal_then_fresh_session_exits",
         module="tests", recovery=True, exit_key=b"\x04")
for key in (b"/quit\r", b"\x03", b"\x04"):
    run_case("persistent_room_wait_loss_return_and_write_cancellation",
             module="tests", room=True, no_color=True, exit_key=key)
print("9 PTY cases passed: draft redraw, NO_COLOR, exits, persistent room commands, loss/return, no replay and write cancellation; terminal mode restored.")
