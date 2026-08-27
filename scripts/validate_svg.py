#!/usr/bin/env python3
from __future__ import annotations
import sys
from pathlib import Path
import xml.etree.ElementTree as ET

MAX_BYTES = 8 * 1024 * 1024
FORBIDDEN = ("<script", "javascript:", "onload=", "onclick=", "onerror=")


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: validate_svg.py PATH", file=sys.stderr)
        return 2

    path = Path(sys.argv[1])
    data = path.read_bytes()
    if len(data) > MAX_BYTES:
        raise SystemExit(f"SVG too large: {len(data)} bytes > {MAX_BYTES}")

    text = data.decode("utf-8")
    low = text.lower()
    for token in FORBIDDEN:
        if token in low:
            raise SystemExit(f"forbidden SVG token: {token}")

    root = ET.fromstring(text)
    if not root.tag.endswith("svg"):
        raise SystemExit("root element is not svg")
    if "animate" not in low and "@keyframes" not in low:
        raise SystemExit("expected declarative animation")
    if "@keyframes leadercamera" not in low:
        raise SystemExit("expected transform-based cinematic camera")
    if 'id="camera-world"' not in low:
        raise SystemExit("expected moving camera world")
    if "transform:matrix(" not in low:
        raise SystemExit("expected camera matrix keyframes")
    if 'attributename="viewbox"' in low:
        raise SystemExit("legacy animated viewBox camera must not remain")
    if "game clear" not in low:
        raise SystemExit("expected game-clear terminal state")

    print(f"ok: {path} ({len(data)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
