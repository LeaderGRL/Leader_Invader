# Roadmap

## M0 — deterministic README pipeline ✅

- Rust workspace and CI.
- Stable detailed hardware topology.
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

The critical visible CPU/control path is now overwhelmingly native and physically authoritative.

### Completed

#### Control unit

- native T0/T1/T2 CPU microcycles;
- real µPC with fetch, sequential, dispatch, routine-call and routine-return transitions;
- `256 × 24` physical control-ROM representation;
- all 24 control bits persisted into the native trace;
- 24 distinct visible control outputs with a stable bit → node → label contract;
- full µROM scan proving all 24 physical bits are exercised;
- internal control outputs wired to real physical consumers;
- visible address, condition, PC-select and register-select state latches.

#### Datapath authority

- physically gated MAR/MDR/IR fetch latches and ripple PC increment;
- shared operand/read/write micro-routines;
- ripple-carry 8-bit ALU and 16-bit PC/SP increment/decrement networks;
- authoritative operand A/B and ALU-op latches;
- authoritative flags;
- authoritative register selection and write-back destination;
- authoritative low/high target-address latches;
- authoritative branch-condition latch and PC input selection;
- LD/ST, jumps/branches, CALL and RET routed through those latches;
- compact LDI/ADDI routed through the same physical A/B/op and commit discipline.

#### CALL / RET

- dedicated visible `RETURN BYTE` high/low PC mux;
- combinational PC → stack-data CALL path instead of an unnecessary hidden return-address latch;
- RET data path from stack → data bus → low/high address latches → PC;
- full-match contract proving CALL pushes exactly `instruction_pc + 3`, RET restores those bytes in LIFO order and actual PC return targets match the reconstructed stack stream.

#### Native trace and replay

- native bus ownership transactions;
- native exact ALU, register-write and PC event streams;
- native decoder visualization from IR `DecodeLatch` microcycles;
- exact PC before/after/source/carry metadata in SVG;
- exact SP before/after/address/data metadata in SVG;
- exact ALU operands/result/carry-chain metadata in SVG;
- exact register before/after write metadata in SVG;
- exact bus owner/address/data/control metadata in SVG;
- exact T-state PC/MAR/MDR/IR snapshots in SVG;
- complete native-only F3 overlay pipeline independent of `micro_samples`.

#### Production contracts

- production base rendering clears `micro_samples`, so the legacy coarse activity layer cannot enter `generated/Leader.svg`;
- native control authority validator ties architectural mutations back to physical µROM rows;
- traced µROM words are compared exactly against `control_word_at(µADDR, opcode).bits24()`;
- CPU fetch/read/write transactions are tied to their shared physical micro-routines;
- final injected topology validator checks unique/closed nodes and links, group containment and required F3 hardware;
- final SVG contract requires all native overlay/metadata families and rejects legacy coarse activity;
- GitHub-safe declarative output validation;
- hard production SVG budget of 5,000,000 bytes;
- CI cancels obsolete branch/PR runs and ignores generated-only updates.

### Remaining F3 work

The remaining items are now refinements rather than missing causal CPU paths:

- optionally add a dedicated first-class `SpEvent` mutation stream; current SP execution and rendering are already causal through native stack transactions and exact ripple reconstruction;
- decide how long historical `MicroSample` reconstruction fallbacks should remain in public core helper APIs for old traces;
- continue reducing duplicate presentation work where multiple native overlays cover the same physical nodes, while preserving inspectable metadata and the 5 MB artifact budget;
- audit whether flag/control-state **values** should become dedicated native events in addition to their already-authoritative physical load enables;
- close any remaining visible wiring gaps discovered during review before declaring F3 complete.

### Acceptance

F3 is complete when every visible critical CPU datapath/control node is driven from native same-tick state or a physically justified combinational path, production rendering contains no semantic fallback activity, and all trace/topology/SVG contracts remain green.

## M3 — richer arcade hardware

Once F3 is closed, push more of the game-specific peripherals toward explicit hardware:

- multiple enemy shots;
- shields;
- formation cadence tied to remaining invaders;
- original-arcade-inspired 16-bit shift-register peripheral;
- optional 8080-flavoured memory map;
- no proprietary arcade ROM assets.

## M4 — live WASM explorer

The README remains a zero-JavaScript cinematic artifact, while the same core powers a live explorer with:

- drag-pan and wheel/pinch zoom;
- node inspection using the same native metadata contracts;
- pause / micro-step / instruction-step;
- follow PC / bus / DMA / VBlank;
- seed and bot-policy selection.

## M5 — generated match seasons

- seed from commit SHA on source changes;
- optional scheduled matches;
- archive winning traces;
- multiple deterministic bot strategies;
- match metadata embedded in the SVG footer.
