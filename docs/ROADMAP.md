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
- complete native-only F3 overlay pipeline independent of `micro_samples`.

### Production contracts

- production base rendering suppresses the legacy `MicroSample` activity layer;
- production CLI receives every required first-class stream directly from `Machine::run_match()`;
- production no longer calls SP materialization/fallback reconstruction;
- native control authority validator ties architectural mutations back to physical µROM rows;
- SP events are cross-checked against exact native stack bus transactions and local ordering;
- final injected topology validator checks unique/closed nodes and links, group containment and required hardware;
- final SVG contract requires all native overlay/metadata families and rejects legacy coarse activity;
- GitHub-safe declarative output validation;
- hard production SVG budget of 5,000,000 bytes;
- CI cancels obsolete branch/PR runs and ignores generated-only updates.

### F3 acceptance — satisfied

F3 required every visible critical CPU datapath/control node to be driven from native same-tick state or a physically justified combinational path, production rendering to contain no semantic fallback activity, and all trace/topology/SVG contracts to remain green.

Those conditions are enforced in code and CI. Historical reconstruction helpers remain only for backward compatibility with old trace shapes; they are not production dependencies.

## M3 — richer arcade hardware 🚧

The F3 physical-authority rule is now being applied to game-specific hardware. Semantics remain complete and unsampled; only cinematic presentation is bounded.

### Original-arcade-inspired 16-bit shift register ✅

- explicit 16-bit device state with two-byte cascading load;
- 3-bit offset register and 8-bit shifted read window;
- memory-mapped `SHIFT_DATA`, `SHIFT_OFFSET`, `SHIFT_RESULT` ports;
- assembled ROM boot self-test proving `0x12`, `0x34`, offset `3` → `0xA0`;
- first-class shift-register events tied to same-tick CPU bus transactions;
- causal state replay contract with corruption tests;
- visible `SHIFT HI / LO / OFFSET / WINDOW / OUT` hardware path;
- native SVG metadata and mandatory production overlay.

### Hardware formation cadence ✅

- old high-level `frame % 3` movement gate removed;
- persistent hardware counter/divider is now the only fleet movement gate;
- divisor accelerates `3 → 2 → 1` as the formation thins;
- native clock stream includes alive count, divisor, counter before/after and tick;
- validator replays every clock and rejects fleet RAM mutation without `tick=true`;
- physical alive/divider/counter/tick nodes;
- bounded overlay preserves all three speed bands and both tick states.

### Three-slot enemy projectile bank ✅

- singleton enemy projectile removed from `GameState` and renderer;
- explicit `EnemyShotBank` owns three independent projectile slots;
- round-robin allocator + hardware cooldown state;
- each slot has authoritative RAM bytes for `X / Y / ACTIVE`;
- bot avoidance, collision, VRAM and gameplay replay consume all three slots;
- deterministic match must exercise all three slots and at least two simultaneous projectiles;
- RAM-authority contract replays every slot write across native snapshot intervals;
- invalid arm/clear ordering, missing writes and corrupted snapshots fail validation;
- physical allocator/cooldown plus three visible X/Y/ACTIVE banks;
- strict 84-frame maximum overlay sampling preserves slot use and concurrency;
- current generated replay after this slice: **3,767,395 bytes** (< 5 MB).

### Destructible shields ⏭️

Next slice. Target contract:

- explicit shield bitmap memory, not decorative sprites;
- projectile collision damages bitmap bits causally;
- both player and enemy projectiles interact with the same shield state;
- VRAM rasterization consumes shield memory;
- native mutation stream or exact RAM/bitmap replay contract;
- corruption tests for wrong shield/bit/address;
- physical shield RAM / address / damage-mask path;
- compact presentation to preserve the remaining ~1.23 MB SVG budget.

### Optional follow-ups

- 8080-flavoured memory-map cleanup;
- richer original-arcade timing/peripheral quirks where they improve inspectability;
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
