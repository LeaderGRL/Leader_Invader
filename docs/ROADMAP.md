# Roadmap

## M0 — deterministic README pipeline

- Rust workspace and CI.
- Stable 300+ node topology derived from the visual CPU prototype.
- Seeded autonomous Space Invaders-like match.
- Real RAM state + 128×96 one-bit VRAM generation.
- Trace-driven long-form SVG with automatic camera choreography.
- GitHub Actions regeneration from commit SHA.

## M1 — byte-addressed ISA

Move the game loop from host-language semantic routines into ROM bytecode interpreted by the CPU.

- fetch / decode / execute through ROM bytes;
- stack and control flow;
- memory-mapped input/video registers;
- assembler owned by the repository;
- game program emitted as ROM.

Acceptance: corrupting one ROM instruction must change or break the match causally.

## M2 — bit-accurate datapath

Promote visible datapath nodes to first-class logic state.

- DFF edge semantics;
- PC/MAR/MDR/IR bits;
- carry propagation through every ALU slice;
- decoder select lines;
- tri-state bus ownership;
- explicit control-ROM outputs.

Acceptance: every glowing datapath node is directly computed in the same tick.

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
