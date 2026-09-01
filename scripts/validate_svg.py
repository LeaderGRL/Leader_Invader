#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path
import xml.etree.ElementTree as ET

# Artifact size is telemetry only. Native semantic completeness, physical
# inspectability and GitHub-safe SVG structure are validity conditions; a byte
# ceiling is not. Optimize duplicated presentation later without deleting real
# machine state merely to satisfy an arbitrary file-size target.
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
    'id="v2-native-bus-analyzer"',
    'data-source="native-bus-transactions"',
    'id="v2-crt"',
    'id="v2-final-crt-focus"',
    'data-final-focus="native-vram"',
    'data-final-native-raster="128x96"',
    'data-final-native-pixels="true"',
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

    size_mib = len(data) / (1024 * 1024)
    print(
        f"ok: {path} ({len(data)} bytes / {size_mib:.2f} MiB telemetry, physical die, "
        f"{memory_pages} pages / 34816 visible byte cells, native final CRT)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
