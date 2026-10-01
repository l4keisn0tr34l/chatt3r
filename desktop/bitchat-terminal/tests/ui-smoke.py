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


def run_case(test, *, no_color=False, exit_key=None):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    before = termios.tcgetattr(slave)
    env = dict(os.environ, TERM="xterm-256color")
    env.pop("NO_COLOR", None)
    if no_color:
        env["NO_COLOR"] = "1"
    proc = subprocess.Popen(
        [binary, "--exact", f"ui::tests::{test}", "--ignored", "--nocapture", "--test-threads=1"],
        stdin=slave, stdout=slave, stderr=slave, env=env,
    )
    data = b""
    typed = completed = False
    deadline = time.monotonic() + 10
    try:
        while time.monotonic() < deadline:
            readable, _, _ = select.select([master], [], [], 0.1)
            if readable:
                data += os.read(master, 65536)
            if b"you> " in data and not typed:
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
print("4 PTY cases passed: draft redraw, NO_COLOR, Ctrl-C, Ctrl-D; terminal mode restored.")
