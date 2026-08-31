#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path
import xml.etree.ElementTree as ET

# V2 intentionally spends more bytes on visible physical density. This is a
# review budget, not the final optimization target; structural symbol reuse can
# reduce it later without removing any visible byte cells.
MAX_BYTES = 3 * 1024 * 1024
FORBIDDEN = (
    "<script",
    "javascript:",
    "onload=",
    "onclick=",
    "onerror=",
    "<animatemotion",
    "leadercamera",
    'attributename="viewbox"',
    'id="camera-world"',
    'values="0;1;1;0"',
    "rgb332 screen",
)
REQUIRED = (
    'data-frontpage-version="physical-die-v2"',
    'id="v2-machine"',
    'id="v2-logic-nodes"',
    'id="v2-static-wires"',
    'id="v2-memory-byte-fabric"',
    'id="v2-native-bus-propagation"',
    'id="v2-native-alu-propagation"',
    'id="v2-exact-memory-cell-activity"',
    'id="v2-crt"',
    'data-memory-bytes="34816"',
    'data-memory-bit-cells="278528"',
    'data-byte-cells="256"',
    'data-memory-address="',
    'data-memory-bits="',
    'data-vram-frame="',
    'data-vram-pixels="',
    'values="0;1;0;0"',
    "1-bit crt",
)


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
            raise SystemExit(f"forbidden physical-die SVG token: {token}")

    root = ET.fromstring(text)
    if not root.tag.endswith("svg"):
        raise SystemExit("root element is not svg")

    if "<animate" not in low:
        raise SystemExit("expected declarative native-trace animation")

    for marker in REQUIRED:
        if marker.lower() not in low:
            raise SystemExit(f"missing physical-die marker: {marker}")

    memory_pages = low.count('data-byte-cells="256"')
    if memory_pages != 136:
        raise SystemExit(f"expected 136 visible memory pages, found {memory_pages}")

    print(
        f"ok: {path} ({len(data)} bytes, physical die, "
        f"{memory_pages} pages / 34816 visible byte cells)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
