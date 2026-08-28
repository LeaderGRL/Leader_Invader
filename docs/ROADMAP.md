# Roadmap

## M0 — deterministic README pipeline ✅

- Rust workspace and CI.
- Stable 300+ node topology derived from the visual CPU prototype.
- Seeded autonomous Space Invaders-like match.
- Real RAM state + 128×96 one-bit VRAM generation.
- Trace-driven long-form SVG.
- GitHub Actions regeneration from commit SHA.
- Cinematic director with subsystem close-ups and final framebuffer zoom.

## M1 — byte-addressed ISA / F2 ✅

The game control loop lives in assembled ROM bytecode interpreted by the CPU.

- real instruction fetch / decode / execute through ROM bytes;
- eight 8-bit registers, flags, PC and stack pointer;
- CALL / RET and conditional control flow;
- memory-mapped input, game-device and video registers;
- repository-owned two-pass assembler with labels/fixups;
- game program emitted as an 8 KiB-bounded ROM image;
- WAIT_VBLANK and HALT are real instructions;
- corrupting the ROM breaks the match causally.

The complex collision/raster/DMA operations remain memory-mapped hardware devices. The ROM controls when and in what order those devices execute.

## M2 — bit-accurate datapath / F3 🚧

Most of the critical visible CPU path is now physically authoritative.

### Completed

- native T0/T1/T2 CPU microcycles;
- real µPC with fetch, sequential, dispatch, routine-call and routine-return transitions;
- `256 × 24` physical control-ROM representation;
- complete 24-bit control word persisted into the trace and consumed by the SVG overlay;
- physically gated MAR/MDR/IR fetch latches and ripple PC increment;
- shared operand/read/write micro-routines;
- ripple-carry 8-bit ALU and 16-bit PC/SP increment/decrement networks;
- authoritative operand A/B and ALU-op latches;
- authoritative flag latch;
- authoritative register selection and write-back destination;
- authoritative low/high address latches;
- authoritative branch-condition latch and PC input selection;
- LD/ST, jumps/branches, CALL and RET routed through those latches;
- compact LDI/ADDI routed through real A/B/op latches and commit control;
- native bus ownership transactions;
- native exact ALU, register-write and PC event streams;
- native decoder visualization from IR `DecodeLatch` microcycles;
- native-first bus, decoder, ALU, register, PC and stack render paths;
- CI invariants proving those render paths remain valid after deleting `micro_samples`.

### Remaining

- decide whether SP should gain its own first-class native mutation event stream instead of deriving visual SP transitions from already-native stack bus transactions;
- decide whether CALL should gain a dedicated return-address latch rather than sampling the current PC combinationally before pushing the two bytes;
- continue shrinking/removing historical semantic reconstruction fallbacks where they no longer provide useful compatibility;
- audit the remaining coarse base-SVG activity layer and either migrate or retire semantic-only illumination paths.

### Acceptance

Every visible critical datapath/control node must be computed from same-tick machine state, and breaking a critical control line must change or prevent execution causally.

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
