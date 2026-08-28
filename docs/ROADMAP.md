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

## M2 — bit-accurate datapath / F3 ✅

The visible CPU/control path is production-native and physically authoritative.

### Control unit

- native T0/T1/T2 CPU microcycles;
- real µPC with fetch, sequential, dispatch, routine-call and routine-return transitions;
- `256 × 24` physical control-ROM representation;
- all 24 control bits persisted into the native trace;
- 24 distinct visible control outputs with a stable bit → node → label contract;
- full µROM scan proving all 24 physical bits are exercised;
- internal control outputs wired to real physical consumers;
- visible address, condition, PC-select and register-select state latches;
- exact first-class values for those visible latches.

### Datapath authority

- physically gated MAR/MDR/IR fetch latches and ripple PC increment;
- shared operand/read/write micro-routines;
- ripple-carry 8-bit ALU and 16-bit PC/SP increment/decrement networks;
- authoritative operand A/B and ALU-op latches;
- authoritative flag mutation and exact Z/C/L native events;
- authoritative register selection and write-back destination;
- authoritative low/high target-address latches;
- authoritative branch-condition latch and PC input selection;
- LD/ST, jumps/branches, CALL and RET routed through those latches;
- compact LDI/ADDI routed through the same physical A/B/op and commit discipline;
- PC increments explicitly validated against `PC_INC`;
- PC loads explicitly validated against `PCLD + ARCH_COMMIT`;
- SP mutations explicitly validated against `STACK`.

### CALL / RET

- dedicated visible `RETURN BYTE` high/low PC mux;
- combinational PC → stack-data CALL path instead of an unnecessary hidden return-address latch;
- RET data path from stack → data bus → low/high address latches → PC;
- direct CPU-native `SpEvent` PUSH/POP stream;
- full-match contract proving CALL pushes exactly `instruction_pc + 3`, RET restores those bytes in LIFO order and actual PC return targets match the reconstructed stack stream;
- corruption of authoritative SP return bytes is detected;
- corruption of an older bus fallback cannot override the first-class SP stream.

### Native trace and replay

- native bus ownership transactions;
- native exact ALU events and carry chains;
- native exact flag events;
- native exact control-latch value events;
- native register-write events;
- native exact PC increment/load events;
- native exact SP PUSH/POP events including ripple chain, address and data;
- native decoder visualization from IR `DecodeLatch` microcycles;
- exact PC before/after/source/carry metadata in SVG;
- exact SP before/after/ripple/address/data metadata in SVG;
- exact ALU operands/result/carry-chain metadata in SVG;
- exact Z/C/L flag metadata in SVG;
- exact visible control-latch kind/value/validity metadata in SVG;
- exact register before/after write metadata in SVG;
- exact bus owner/address/data/control metadata in SVG;
- exact T-state PC/MAR/MDR/IR snapshots in SVG;
- complete native-only F3 overlay pipeline independent of `micro_samples`.

### Production contracts

- production base rendering suppresses the legacy `MicroSample` activity layer;
- production CLI receives every required first-class stream directly from `Machine::run_match()`;
- production no longer calls SP materialization/fallback reconstruction;
- native control authority validator ties architectural mutations back to physical µROM rows;
- traced µROM words are compared exactly against `control_word_at(µADDR, opcode).bits24()`;
- CPU fetch/read/write transactions are tied to their shared physical micro-routines;
- SP events are cross-checked against exact native stack bus transactions and local ordering;
- final injected topology validator checks unique/closed nodes and links, group containment and required F3 hardware;
- final SVG contract requires all native overlay/metadata families and rejects legacy coarse activity;
- GitHub-safe declarative output validation;
- hard production SVG budget of 5,000,000 bytes;
- current artifact is roughly 3.3 MB;
- CI cancels obsolete branch/PR runs and ignores generated-only updates.

### F3 acceptance — satisfied

F3 required every visible critical CPU datapath/control node to be driven from native same-tick state or a physically justified combinational path, production rendering to contain no semantic fallback activity, and all trace/topology/SVG contracts to remain green.

Those conditions are now enforced in code and CI. Historical reconstruction helpers remain only for backward compatibility with old trace shapes; they are not production dependencies.

### Post-F3 maintenance

These are cleanup/optimization tasks, not F3 blockers:

- decide when old `MicroSample`/bus reconstruction helper APIs can be deprecated or removed;
- continue reducing duplicate presentation work where multiple native overlays cover the same physical nodes, while preserving inspectable metadata;
- keep the generated artifact comfortably below the 5 MB budget;
- close any future visible wiring regression discovered by the topology/SVG contracts.

## M3 — richer arcade hardware

With F3 closed, move more game-specific peripheral behavior into explicit hardware:

- multiple enemy shots;
- shields;
- formation cadence tied to remaining invaders;
- original-arcade-inspired 16-bit shift-register peripheral;
- optional 8080-flavoured memory map;
- no proprietary arcade ROM assets.

The first M3 slices should follow the same rule established by F3: a visible peripheral path is backed by real state/control and emits a native event stream before it gains cinematic presentation.

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
