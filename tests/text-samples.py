#!/usr/bin/env python3
"""Export synthetic text fixtures for manual checks; never access Bluetooth."""
import argparse
import json
import os
from pathlib import Path
import sys


FIXTURES = Path(__file__).resolve().parents[1] / "desktop/bitchat-terminal/test-vectors/public-text-long.json"
SAMPLES = (
    ("numbered-100", "numbered-100.txt"),
    ("numbered-256", "numbered-256.txt"),
    ("numbered-1024", "numbered-1024.txt"),
    ("formatted-multiline", "multiline.txt"),
    ("hardware-numbered-2048", "numbered-2048.txt"),
    ("hardware-numbered-8192", "numbered-8192.txt"),
    ("hardware-numbered-16384", "numbered-16384.txt"),
    ("hardware-repeat-16384", "repeat-16384.txt"),
)


def write_limit(value):
    limit = int(value)
    if not 36 <= limit <= 512:
        raise argparse.ArgumentTypeError("write limit must be 36..512 value bytes")
    return limit


def check_existing(path, payload):
    if path.is_symlink() or (path.exists() and (not path.is_file() or path.read_bytes() != payload)):
        raise FileExistsError(f"refusing to replace existing content: {path}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output_dir", type=Path, help="directory for exact-byte text files")
    parser.add_argument("--write-limit", type=write_limit, default=128,
                        help="operator characteristic-value limit, not measured MTU (default: 128)")
    args = parser.parse_args()
    try:
        cases = {case["label"]: case for case in json.loads(FIXTURES.read_text(encoding="utf-8"))["cases"]}
        samples = []
        for label, filename in SAMPLES:
            case = cases[label]
            payload = bytes.fromhex(case["decoded_hex"])
            wire = bytes.fromhex(case["wire_hex"])
            header = 14 if wire[0] == 1 else 16
            wire_payload = int.from_bytes(wire[12:header], "big")
            # chatt3r uses a v1 fragment header even when the inner packet is v2.
            chunk = args.write_limit - (14 + 8 + (8 if wire[11] & 1 else 0) + 13)
            frames = 1 if len(wire) <= args.write_limit else (len(wire) + chunk - 1) // chunk
            expected = str(frames) if frames <= 1024 else f"rejected: {frames}>1024"
            path = args.output_dir / filename
            check_existing(path, payload)
            samples.append((path, payload, wire_payload, len(wire), expected))

        args.output_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
        for path, payload, *_ in samples:
            try:
                fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            except FileExistsError:
                check_existing(path, payload)
            else:
                with os.fdopen(fd, "wb") as stream:
                    stream.write(payload)  # No added newline or text normalization.
    except (OSError, ValueError, KeyError) as error:
        parser.error(str(error))

    print("file\tdecoded_bytes\twire_payload_bytes\tsigned_packet_bytes\texpected_frames")
    for path, payload, wire_payload, packet_bytes, frames in samples:
        print(f"{path}\t{len(payload)}\t{wire_payload}\t{packet_bytes}\t{frames}")
    print("Expected fixture sizes/frames only; no Bluetooth operation or delivery check. "
          "New files request mode 0600; identical existing files are left unchanged.", file=sys.stderr)


if __name__ == "__main__":
    main()
