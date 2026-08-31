#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path
import xml.etree.ElementTree as ET

MAX_BYTES = 2 * 1024 * 1024
FORBIDDEN = ("<script", "javascript:", "onload=", "onclick=", "onerror=")
REQUIRED = (
    'data-frontpage-version="observatory-v1"',
    'id="frontpage-overview"',
    'id="frontpage-native-bus-pulses"',
    'id="frontpage-logic-microscope"',
    'id="frontpage-native-video-replay"',
    'id="frontpage-native-telemetry"',
    'data-bus-address="',
    'data-bus-data="',
    'data-detail-module="ramsys.pages"',
    'data-detail-module="alu.ripple"',
    'data-detail-module="decode.microcode"',
    'data-vram-frame="',
    'values="0;1;0;0"',
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
            raise SystemExit(f"forbidden SVG token: {token}")

    root = ET.fromstring(text)
    if not root.tag.endswith("svg"):
        raise SystemExit("root element is not svg")

    if "<animate" not in low and "<animatemotion" not in low:
        raise SystemExit("expected declarative native-trace animation")

    for marker in REQUIRED:
        if marker.lower() not in low:
            raise SystemExit(f"missing fixed-frontpage marker: {marker}")

    if "leadercamera" in low:
        raise SystemExit("cinematic camera must not remain in the GitHub front page")
    if 'attributename="viewbox"' in low:
        raise SystemExit("animated viewBox camera must not remain")
    if 'id="camera-world"' in low:
        raise SystemExit("moving camera world must not remain")
    if 'values="0;1;1;0"' in low:
        raise SystemExit("VRAM replay may not accumulate previous raster frames")
    if "game clear" not in low:
        raise SystemExit("expected game-clear terminal state")

    print(f"ok: {path} ({len(data)} bytes, fixed observatory)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
