# Live WASM explorer

This directory is the browser shell for M4. The browser is deliberately a presentation/controller layer: physical topology, hierarchy, hit testing, navigation targets, routed wires, trace playback and machine activity continue to come from Rust.

## Build

From the repository root:

```bash
cargo test --workspace
cargo check -p leader-explorer --target wasm32-unknown-unknown
wasm-pack build crates/leader-explorer --target web --release --out-dir ../../explorer-web/pkg
```

Then serve the repository with any static HTTP server, for example:

```bash
python -m http.server 8080
```

Open `http://localhost:8080/explorer-web/`.

## Current M4 surface

- canonical machine → subsystem → detail hierarchy;
- physical node/link graph exported by WASM;
- canonical orthogonal wire routes derived by Rust;
- node hit testing against real topology bounds;
- click-to-enter deepest canonical node view;
- breadcrumb, parent/back/home navigation;
- drag-pan, wheel zoom and two-pointer pinch/pan backed by Rust viewport state;
- node inspection from the same native metadata;
- deterministic trace generation from a user seed and frame budget;
- play/pause, native microcycle step and instruction-boundary step;
- exact microcycle scrubber plus frame seek;
- native PC/MAR/MDR/IR and bus address/data/source display in hex;
- exact event focus for bus, DMA and VBlank follows;
- follow PC, next bus transaction, next DMA and next VBlank;
- core-owned physical activity mapping with exact addressed ROM/RAM/VRAM page selection;
- live node illumination and routed-wire animation from native activity;
- responsive desktop/tablet/mobile browser layout;
- reduced-motion support for signal animation.

## Authority rule

JavaScript must not implement CPU, device, memory-map, topology, hierarchy, routing, hit-test, trace or gameplay semantics. It may transform browser pointer coordinates, render data returned by WASM, and invoke explicit Rust controller operations. New explorer features must extend Rust APIs first whenever they require machine knowledge.

The current wire glow is a conservative visualization of the core-owned activity subgraph. Exact per-link electrical stage timing, bus values directly on routes and live bit-flip propagation remain explicit M4 work in `docs/ROADMAP.md`.
