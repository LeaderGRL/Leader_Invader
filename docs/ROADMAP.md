# Roadmap

## M0 — deterministic README pipeline ✅

- Rust workspace and CI.
- Stable 300+ node topology derived from the visual CPU prototype.
- Seeded autonomous Space Invaders-like match.
- Real RAM state + 128×96 one-bit VRAM generation.
- Trace-driven long-form SVG.
- GitHub Actions regeneration from commit SHA.
- cinematic director with subsystem close-ups and final framebuffer zoom.

## M1 — byte-addressed ISA ✅

The game control loop now lives in assembled ROM bytecode interpreted by the CPU.

- real instruction fetch / decode / execute through ROM bytes;
- eight 8-bit registers, flags, PC and stack pointer;
- CALL / RET and conditional control flow;
- memory-mapped input, game-device and video registers;
- repository-owned two-pass assembler with labels/fixups;
- game program emitted as an 8 KiB-bounded ROM image;
- WAIT_VBLANK and HALT are real instructions;
- trace records ROM fetches, decoded opcodes, stack traffic, memory traffic and ALU work.

Acceptance is covered by test: replacing the first ROM opcode with `HALT` prevents the match from advancing or killing an invader.

The complex collision/raster/DMA operations are intentionally still modeled as memory-mapped hardware devices. The ROM controls *when* and *in what order* those devices execute. Moving those devices down into gate-level datapath logic belongs to M2/M3 rather than hiding host callbacks behind fake CPU lights.

## M2 — bit-accurate datapath

Promote visible datapath nodes to first-class logic state.

- DFF edge semantics;
- PC/MAR/MDR/IR bits;
- carry propagation through every ALU slice;
- decoder select lines;
- tri-state bus ownership;
- explicit control-ROM outputs;
- derive visible activity directly from each tick's bit state instead of subsystem-level trace mapping.

Acceptance: every glowing datapath node is directly computed in the same tick, and breaking a visible critical node changes execution causally.

## M3 — richer arcade hardware

- multiple enemy shots;
- shields;
- formation cadence tied to remaining invaders;
- original-arcade-inspired 16-bit shift-register peripheral;
- optional 8080-flavoured memory map;
- no proprietary arcade ROM assets.

## M4 — live WASM explorer

The README remains a zero-JavaScript cinematic artifact, while the same core powers a live explorer with:

- drag-pan and wheel/pinch zoom;
- node inspection;
- pause / micro-step / instruction-step;
- follow PC / bus / DMA / VBlank;
- seed and bot-policy selection.

## M5 — generated match seasons

- seed from commit SHA on source changes;
- optional scheduled matches;
- archive winning traces;
- multiple deterministic bot strategies;
- match metadata embedded in the SVG footer.
