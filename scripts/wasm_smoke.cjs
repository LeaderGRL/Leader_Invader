const assert = require('node:assert/strict');
const path = require('node:path');

const generatedModule = process.argv[2];
if (!generatedModule) {
  throw new Error('usage: node scripts/wasm_smoke.cjs <generated-module>');
}

const wasm = require(path.resolve(generatedModule));
assert.equal(typeof wasm.Explorer, 'function', 'Explorer export missing');
assert.equal(typeof wasm.Playback, 'function', 'Playback export missing');
assert.equal(typeof wasm.ActivityResolver, 'function', 'ActivityResolver export missing');

const explorer = new wasm.Explorer();
const topology = JSON.parse(explorer.topology_json());
assert.ok(Array.isArray(topology.nodes) && topology.nodes.length > 400, 'canonical topology missing');
assert.ok(Array.isArray(topology.links) && topology.links.length > 800, 'canonical links missing');

const playback = new wasm.Playback();
assert.equal(playback.load_match('ci-wasm-smoke', 24), true, 'native match failed to load');
assert.equal(playback.is_loaded(), true, 'playback did not retain native trace');

const vram = JSON.parse(playback.current_vram_json());
assert.equal(vram.width, 128);
assert.equal(vram.height, 96);
assert.equal(vram.format, '1bpp-msb-first-row-major');
assert.equal(vram.bytes.length, 1536);
assert.equal(typeof vram.checksum, 'number');

const activity = JSON.parse(playback.current_activity_json());
assert.ok(activity && typeof activity === 'object', 'current physical activity missing');

const aluLinks = JSON.parse(playback.current_alu_links_json());
assert.ok(Array.isArray(aluLinks), 'ALU propagation API is not JSON array shaped');

const resolver = new wasm.ActivityResolver();
const busLinks = JSON.parse(resolver.bus_links_json(0x83fe, 0x5a, 'dma', 'vram', 'dma'));
assert.ok(Array.isArray(busLinks) && busLinks.length > 0, 'bus propagation API returned no physical links');
assert.ok(busLinks.some((link) => link.stage === 'page_select' && link.rank === 4), 'VRAM page selection missing');
assert.ok(busLinks.some((link) => link.stage === 'dma_data_latch' && link.rank === 6), 'DMA data latch propagation missing');

console.log(`WASM smoke OK: ${topology.nodes.length} nodes, ${topology.links.length} links, VRAM ${vram.width}x${vram.height}, ${busLinks.length} bus stages`);