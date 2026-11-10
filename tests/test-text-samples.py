#!/usr/bin/env python3
"""CLI checks for exact-byte samples and non-destructive exports; no radio."""
import csv
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "tests/text-samples.py"
FIXTURES = ROOT / "desktop/bitchat-terminal/test-vectors/public-text-long.json"


class TextSampleTests(unittest.TestCase):
    def run_cli(self, directory, *args):
        return subprocess.run([sys.executable, str(SCRIPT), str(directory), *args],
                              cwd="/tmp", capture_output=True, text=True, timeout=10)

    def test_exact_fixtures_and_frame_boundary_without_rewriting(self):
        cases = {case["label"]: case for case in json.loads(FIXTURES.read_text())["cases"]}
        with tempfile.TemporaryDirectory(prefix="chatt3r text samples ") as temporary:
            output = Path(temporary) / "samples"
            first = self.run_cli(output)
            self.assertEqual(first.returncode, 0, first.stderr)
            rows = {Path(row["file"]).name: row for row in csv.DictReader(first.stdout.splitlines(), delimiter="\t")}
            self.assertEqual(len(rows), 8)
            for filename, label in (("numbered-1024.txt", "numbered-1024"),
                                    ("numbered-16384.txt", "hardware-numbered-16384"),
                                    ("repeat-16384.txt", "hardware-repeat-16384"),
                                    ("multiline.txt", "formatted-multiline")):
                path = output / filename
                self.assertEqual(path.read_bytes(), bytes.fromhex(cases[label]["decoded_hex"]))
                if os.name == "posix":
                    self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            repeat = output / "repeat-16384.txt"
            self.assertEqual(repeat.stat().st_size, 16384)
            self.assertTrue(repeat.read_bytes().endswith(b" END-REPEAT-16384"))
            self.assertEqual(rows[repeat.name]["expected_frames"], "2")
            self.assertEqual(rows[repeat.name]["wire_payload_bytes"], "70")
            self.assertEqual(rows[repeat.name]["signed_packet_bytes"], "156")
            before = {path: (path.read_bytes(), path.stat().st_mtime_ns) for path in output.iterdir()}
            second = self.run_cli(output, "--write-limit", "182")
            self.assertEqual(second.returncode, 0, second.stderr)
            rows = {Path(row["file"]).name: row for row in csv.DictReader(second.stdout.splitlines(), delimiter="\t")}
            self.assertEqual(rows[repeat.name]["expected_frames"], "1")
            self.assertIn("no Bluetooth operation or delivery check", second.stderr)
            for path, (data, mtime) in before.items():
                self.assertEqual(path.read_bytes(), data)
                self.assertEqual(path.stat().st_mtime_ns, mtime)

    def test_conflicting_file_and_symlink_are_not_replaced(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "samples"
            output.mkdir()
            conflict = output / "repeat-16384.txt"
            conflict.write_bytes(b"keep this unrelated content")
            before = conflict.stat().st_mtime_ns
            result = self.run_cli(output)
            self.assertEqual(result.returncode, 2)
            self.assertIn("refusing to replace", result.stderr)
            self.assertEqual(conflict.read_bytes(), b"keep this unrelated content")
            self.assertEqual(conflict.stat().st_mtime_ns, before)
            self.assertEqual(list(output.iterdir()), [conflict])
            if os.name == "posix":
                target = Path(temporary) / "unrelated.txt"
                target.write_bytes(b"keep target")
                conflict.unlink()
                conflict.symlink_to(target)
                result = self.run_cli(output)
                self.assertEqual(result.returncode, 2)
                self.assertTrue(conflict.is_symlink())
                self.assertEqual(target.read_bytes(), b"keep target")

    def test_invalid_limits_do_not_create_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "samples"
            for limit in ("35", "513", "not-a-number"):
                result = self.run_cli(output, "--write-limit", limit)
                self.assertEqual(result.returncode, 2)
                self.assertFalse(output.exists())

    def test_valid_small_limit_reports_sender_fragment_cap(self):
        with tempfile.TemporaryDirectory() as temporary:
            result = self.run_cli(Path(temporary) / "samples", "--write-limit", "36")
            self.assertEqual(result.returncode, 0, result.stderr)
            rows = {Path(row["file"]).name: row for row in csv.DictReader(result.stdout.splitlines(), delimiter="\t")}
            self.assertEqual(rows["numbered-16384.txt"]["expected_frames"], "rejected: 6516>1024")


if __name__ == "__main__":
    unittest.main()
