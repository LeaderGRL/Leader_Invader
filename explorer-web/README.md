# Live WASM explorer

This directory is the first browser shell for M4. The browser is deliberately a presentation/controller layer: the physical topology, hierarchy, hit testing, navigation targets and execution state continue to come from Rust.

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
- node hit testing against real topology bounds;
- click-to-enter deepest canonical node view;
- breadcrumb, parent/back/home navigation;
- drag-pan and wheel zoom backed by Rust viewport state;
- node inspection from the same native metadata;
- deterministic trace generation from a user seed;
- play/pause, native microcycle step and instruction-boundary step;
- frame seek plus native PC/MAR/MDR/IR display;
- follow PC, next bus transaction, next DMA and next VBlank;
- responsive browser layout.

## Authority rule

JavaScript must not implement CPU, device, memory-map, hierarchy, hit-test or gameplay semantics. It may transform browser pointer coordinates, render the data returned by WASM, and invoke explicit Rust controller operations. New explorer features should extend the Rust APIs first when they require machine knowledge.
