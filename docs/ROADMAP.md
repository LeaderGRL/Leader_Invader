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

## M2 — bit-accurate datapath / F3 ✅

The visible CPU/control path is production-native and physically authoritative.

### Control unit

- native T0/T1/T2 CPU microcycles;
- real µPC with fetch, sequential, dispatch, routine-call and routine-return transitions;
- `256 × 24` physical control-ROM representation;
- all 24 control bits persisted into the native trace;
- 24 distinct visible control outputs with a stable bit → node → label contract;
- full µROM scan proving all 24 physical bits are exercised;
- internal outputs wired to real physical consumers;
- visible address, condition, PC-select and register-select state with exact values.

### Datapath authority

- physically gated MAR/MDR/IR fetch latches and ripple PC increment;
- shared operand/read/write micro-routines;
- ripple-carry 8-bit ALU and 16-bit PC/SP increment/decrement networks;
- authoritative operand/op/flag/register/address/condition/PC-select state;
- CALL/RET stack path with direct CPU-native `SpEvent` mutations;
- native CPU bus/ALU/flag/control/register/PC/SP streams;
- production renderer independent of `MicroSample` reconstruction.

### Production contracts

- exact µROM/control validation;
- SP and CALL/RET contracts;
- final injected topology validation;
- native-only overlay pipeline;
- GitHub-safe declarative output validation;
- hard production SVG budget of 5,000,000 bytes.

### F3 acceptance — satisfied

Every visible critical CPU datapath/control node is backed by native same-tick state or a physically justified combinational path. Removing or corrupting authority is caught by tests/contracts.

## M3 — richer arcade hardware ✅

The F3 authority rule is now applied to the core game-specific hardware set. Simulation semantics remain complete and unsampled; only SVG presentation is bounded.

### Original-arcade-inspired 16-bit shift register ✅

- explicit 16-bit device state with two-byte cascading load;
- 3-bit offset register and 8-bit shifted read window;
- memory-mapped `SHIFT_DATA`, `SHIFT_OFFSET`, `SHIFT_RESULT` ports;
- assembled ROM boot self-test proving `0x12`, `0x34`, offset `3` → `0xA0`;
- first-class shift-register events tied to same-tick CPU bus transactions;
- causal replay/corruption tests;
- visible `SHIFT HI / LO / OFFSET / WINDOW / OUT` hardware path;
- mandatory native SVG metadata/overlay.

### Hardware formation cadence ✅

- persistent hardware counter/divider is the sole fleet movement gate;
- divisor accelerates `3 → 2 → 1` as formation population falls;
- native clocks include alive count, divisor, counter before/after and tick;
- fleet RAM mutation without `tick=true` is invalid;
- visible alive/divider/counter/tick path;
- bounded overlay preserves all speed bands and tick states.

### Three-slot enemy projectile bank ✅

- explicit `EnemyShotBank` owns three independent projectile slots;
- round-robin allocator + hardware cooldown state;
- authoritative per-slot `X / Y / ACTIVE` RAM bytes;
- bot avoidance, collision, VRAM and gameplay replay consume all three slots;
- complete matches exercise all slots and concurrent projectiles;
- exact RAM replay across native frame checkpoints;
- invalid arm/clear ordering, missing writes and snapshot corruption fail validation;
- physical allocator/cooldown plus three visible X/Y/ACTIVE banks;
- shield-caused slot clear requires immediately preceding same-frame/same-PC `SHIELD_DAMAGE_ENEMY` authority;
- strict bounded overlay sampling preserves concurrency and slot use.

### Bit-addressed destructible shields ✅

- four explicit `16 × 8` shield bitmaps = 64 bytes total;
- shield RAM begins at `RAM_BASE + 0x40`;
- initial bunker silhouette is actual bit state, not presentation geometry;
- world coordinate → shield → byte → one-hot bit-mask addressing;
- player and enemy projectile sweeps prevent tunneling at 3 px / 2 px steps;
- each impact clears exactly one existing bit and writes the resulting RAM byte;
- source-specific `SHIELD_DAMAGE_PLAYER` / `SHIELD_DAMAGE_ENEMY` controls;
- replay contract rejects bit creation, duplicate/multi-bit destruction and wrong provenance;
- physical `ADDR -> MASK -> WRITE -> RAM0..3 -> VIDEO` path;
- CRT shield pixels disappear at exact native RAM-write timestamps;
- cross-device enemy-shot/shield ordering is validated;
- shield-complete generated replay measured **3,856,346 bytes**, leaving roughly **1.14 MB** under budget.

### M3 acceptance — satisfied

The four core arcade systems are now authoritative, physically represented and production-validated:

```text
shift register
formation cadence
three-slot enemy projectile bank
bit-addressed destructible shields
```

No proprietary arcade ROM assets are used.

### Post-M3 hardware cleanup / optional fidelity

These are follow-ups, not M3 blockers:

- centralize and document an 8080-flavoured memory map while preserving current addresses;
- add explicit non-overlap/region ownership tests for ROM, RAM, shield/projectile subregions, VRAM and MMIO;
- explore richer original-arcade timing/peripheral quirks only where they improve inspectability;
- continue reducing presentation duplication while maintaining the 5 MB budget;
- eventually deprecate historical `MicroSample`/bus reconstruction helper APIs when compatibility is no longer needed.

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
