#!/usr/bin/env python3
"""test launcher paths/arguments with fake tools; never touches bluetooth."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

source = Path(__file__).resolve().parents[1] / "scripts" / "chatt3r"
with tempfile.TemporaryDirectory(prefix="chatt3r launcher ") as directory:
    root = Path(directory)
    script = root / "scripts" / "chatt3r"
    script.parent.mkdir()
    shutil.copy2(source, script)
    script.chmod(0o755)
    binary = root / "desktop" / "bitchat-terminal" / "target" / "debug" / "chatt3r"
    binary.parent.mkdir(parents=True)
    tools = root / "fake tools"
    tools.mkdir()
    trace = root / "calls.jsonl"
    app = """#!/usr/bin/env python3
import json, os, sys
record = {'kind': 'app', 'args': sys.argv[1:], 'stdin': sys.stdin.read()}
with open(os.environ['TRACE'], 'a') as stream: stream.write(json.dumps(record)+'\\n')
print(json.dumps(record))
"""
    binary.write_text(app)
    binary.chmod(0o755)
    cargo = tools / "cargo"
    cargo.write_text("""#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
assert args[:4] == ['build', '--quiet', '--offline', '--locked'], args
assert args[args.index('--bin')+1] == 'chatt3r', args
assert pathlib.Path(args[args.index('--manifest-path')+1]).name == 'Cargo.toml', args
with open(os.environ['TRACE'], 'a') as stream:
    stream.write(json.dumps({'kind': 'cargo', 'args': args})+'\\n')
target = pathlib.Path(args[args.index('--target-dir')+1]) / 'debug' / 'chatt3r'
target.parent.mkdir(parents=True, exist_ok=True)
target.write_text(os.environ['FAKE_APP'])
target.chmod(0o755)
""")
    cargo.chmod(0o755)
    doctor = tools / "bluetoothctl"
    doctor.write_text("""#!/usr/bin/env python3
import json, os, sys
assert sys.argv[1:] in [['list'], ['show']], sys.argv
with open(os.environ['TRACE'], 'a') as stream:
    stream.write(json.dumps({'kind': 'bluetoothctl', 'args': sys.argv[1:]})+'\\n')
print('fake adapter state')
""")
    doctor.chmod(0o755)
    env = dict(os.environ, PATH=f"{tools}:/usr/bin:/bin", TRACE=str(trace), FAKE_APP=app,
               CARGO_TARGET_DIR=str(root / "wrong target"))
    env.pop("CHATT3R_LE_PEER", None)  # tests must not inherit the operator's phone

    def calls():
        return [json.loads(line) for line in trace.read_text().splitlines()] if trace.exists() else []

    def run(*args, status=0):
        # /tmp instead of the repo checks that the launcher is cwd-independent.
        result = subprocess.run([str(script), *args], cwd="/tmp", env=env,
                                input="draft\n", capture_output=True, text=True, timeout=10)
        assert result.returncode == status, (result.stdout, result.stderr)
        return result

    for args in [(), ("--debug",), ("--name", "my laptop"),
                 ("--write-limit", "64"), ("--scan-only", "--scan-seconds", "30"),
                 ("--direct-le", "AA:BB:CC:DD:EE:FF", "--debug")]:
        record = json.loads(run(*args).stdout)
        assert record["args"] == ["--write-limit", "128", *args], record
        assert record["stdin"] == "draft\n", record
    env["CHATT3R_LE_PEER"] = "AA:BB:CC:DD:EE:FF"
    for args, expected in [
        (("--debug",), ["--direct-le", "AA:BB:CC:DD:EE:FF", "--debug"]),
        (("--debug", "--scan-only"), ["--debug", "--scan-only"]),
        (("--direct-le", "11:22:33:44:55:66"), ["--direct-le", "11:22:33:44:55:66"]),
    ]:
        record = json.loads(run(*args).stdout)
        assert record["args"] == ["--write-limit", "128", *expected], record
    before = len(calls())
    assert "cheat sheet" in run("--help").stdout
    run("--doctor")
    run("--build")
    env.pop("CHATT3R_LE_PEER")
    before = len(calls())
    assert "cheat sheet" in run("--help").stdout
    assert len(calls()) == before, "help must not build or access bluetooth"
    run("--doctor")
    assert calls()[-2:] == [
        {"kind": "bluetoothctl", "args": ["list"]},
        {"kind": "bluetoothctl", "args": ["show"]},
    ]
    before = len(calls())
    run("--build", "--debug", status=2)
    run("--doctor", "--debug", status=2)
    assert len(calls()) == before, "invalid helper arguments must have no side effects"
    run("--build")
    assert calls()[-1]["kind"] == "cargo", "build-only must not start chat"
    assert str(binary.parent.parent) in calls()[-1]["args"], "use the explicit local target dir"
    binary.unlink()
    run("--debug")
    assert [record["kind"] for record in calls()[-2:]] == ["cargo", "app"]
    assert calls()[-1]["args"] == ["--write-limit", "128", "--debug"]

print("launcher tests passed: paths with spaces, argument forwarding, stdin, help, read-only doctor, offline builds.")
