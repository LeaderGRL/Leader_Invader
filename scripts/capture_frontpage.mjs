import { chromium } from "playwright";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const input = path.resolve(process.env.LEADER_SVG ?? "generated/Leader.svg");
const outputDir = path.resolve(process.env.LEADER_FRONT_PAGE_CAPTURE_OUTPUT ?? "generated/frontpage-captures");
const svg = await readFile(input, "utf8");
await mkdir(outputDir, { recursive: true });

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1200, height: 675 }, deviceScaleFactor: 1 });

try {
  await page.setContent(`<!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;width:1200px;height:675px;overflow:hidden;background:#04080d}svg{display:block;width:1200px;height:675px}</style></head><body>${svg}</body></html>`, { waitUntil: "load" });

  const root = page.locator("svg").first();
  const supportsTimeline = await page.evaluate(() => {
    const svgRoot = document.querySelector("svg");
    return Boolean(svgRoot && typeof svgRoot.setCurrentTime === "function" && typeof svgRoot.pauseAnimations === "function");
  });
  if (!supportsTimeline) throw new Error("Browser SVG timeline seeking is unavailable");
  await page.evaluate(() => document.querySelector("svg").pauseAnimations());

  const staticContract = await page.evaluate(() => {
    const viewport = document.querySelector("#v2-machine-viewport");
    const machine = document.querySelector("#v2-machine");
    const clipRect = document.querySelector("#v2-machine-clip rect");
    const bitFabric = document.querySelector("#v2-memory-bitcell-fabric");
    const microFabric = document.querySelector("#v2-microcode-bitcell-fabric");
    const camera = document.querySelector("#v2-camera-contract");
    const finalFocus = document.querySelector("#v2-final-crt-focus");
    const scene = (name) => Number(camera?.getAttribute(`data-scene-${name}`));
    return {
      nodes: document.querySelectorAll("#v2-logic-nodes > g").length,
      nodeLabels: document.querySelectorAll("#v3-node-labels > g").length,
      staticWires: document.querySelectorAll("#v2-static-wires > use").length,
      memoryPages: document.querySelectorAll("#v2-memory-byte-fabric [data-memory-page]").length,
      bitcellPages: document.querySelectorAll("#v2-memory-bitcell-fabric [data-bitcell-page]").length,
      particles: document.querySelectorAll("animateMotion").length,
      rootViewBoxAnimations: document.querySelectorAll('animate[attributeName="viewBox"]').length,
      cameraTranslate: Boolean(document.querySelector("#v2-camera-translate")),
      cameraScale: Boolean(document.querySelector("#v2-camera-scale")),
      finalCrtFocus: Boolean(finalFocus),
      finalCrtSource: finalFocus?.getAttribute("data-final-focus") ?? null,
      finalCrtRaster: document.querySelector("#v2-final-crt-focus [data-final-native-raster]")?.getAttribute("data-final-native-raster") ?? null,
      viewportClip: viewport?.getAttribute("clip-path") ?? null,
      machineClip: machine?.getAttribute("clip-path") ?? null,
      clip: clipRect ? {
        x: Number(clipRect.getAttribute("x")), y: Number(clipRect.getAttribute("y")),
        width: Number(clipRect.getAttribute("width")), height: Number(clipRect.getAttribute("height")),
      } : null,
      memoryBitCells: bitFabric?.getAttribute("aria-label") ?? null,
      microcodeCells: microFabric?.getAttribute("data-microcode-cells") ?? null,
      scenes: {
        fetch: scene("fetch"), micro: scene("micro"), alu: scene("alu"), rom: scene("rom"),
        ram: scene("ram"), aluLate: scene("alu-late"), vram: scene("vram"), gpu: scene("gpu"),
        lateMemory: scene("late-memory"),
      },
    };
  });

  if (staticContract.nodes < 498 || staticContract.nodeLabels !== staticContract.nodes) throw new Error(`Every physical node must exist and have a camera-readable label: ${JSON.stringify(staticContract)}`);
  if (staticContract.staticWires < 1000) throw new Error(`Physical wiring is incomplete: ${JSON.stringify(staticContract)}`);
  if (staticContract.memoryPages !== 136 || staticContract.bitcellPages !== 136) throw new Error(`Expected all 136 physical memory pages and bit fabrics: ${JSON.stringify(staticContract)}`);
  if (staticContract.memoryBitCells !== "278528 physical memory bit-cell sites" || staticContract.microcodeCells !== "6144") throw new Error(`Low-level fabrics are incomplete: ${JSON.stringify(staticContract)}`);
  if (staticContract.particles !== 0 || staticContract.rootViewBoxAnimations !== 0) throw new Error(`Particles or root viewBox camera motion are forbidden: ${JSON.stringify(staticContract)}`);
  if (!staticContract.cameraTranslate || !staticContract.cameraScale || staticContract.viewportClip !== "url(#v2-machine-clip)") throw new Error(`Technical camera rig is incomplete: ${JSON.stringify(staticContract)}`);
  if (!staticContract.finalCrtFocus || staticContract.finalCrtSource !== "native-vram" || staticContract.finalCrtRaster !== "128x96") throw new Error(`Terminal CRT must reuse the native 128x96 VRAM raster: ${JSON.stringify(staticContract)}`);
  if (staticContract.machineClip !== null) throw new Error(`Raw topology must never carry a transformed clipPath: ${staticContract.machineClip}`);
  if (!staticContract.clip || staticContract.clip.x !== 24 || staticContract.clip.width !== 900 || staticContract.clip.x + staticContract.clip.width >= 934) throw new Error(`Hardware viewport must reserve a non-overlapping CRT sidebar: ${JSON.stringify(staticContract.clip)}`);
  for (const [name, value] of Object.entries(staticContract.scenes)) {
    if (!Number.isFinite(value) || value <= 0 || value >= 59) throw new Error(`Camera scene ${name} is not bound to a valid native event: ${value}`);
  }

  const checkpoints = [
    { name: "01-overview", time: 0.25, focus: "full die" },
    { name: "02-fetch-decode", time: staticContract.scenes.fetch, selector: "#v2-native-bus-propagation .v2-active-wire", focus: "PC + native fetch/decode" },
    { name: "03-microcode", time: staticContract.scenes.micro, selector: "#v2-microcode-bitcell-fabric > g", focus: "native 256x24 control word" },
    { name: "04-alu", time: staticContract.scenes.alu, selector: "#v2-native-alu-propagation .v2-active-wire", focus: "native ripple ALU" },
    { name: "05-rom", time: staticContract.scenes.rom, selector: "#v2-exact-memory-cell-activity > g[data-memory-owner=\"rom\"]", focus: "native ROM page/byte" },
    { name: "06-ram", time: staticContract.scenes.ram, selector: "#v2-exact-memory-cell-activity > g[data-memory-owner=\"ram\"]", focus: "native RAM page/byte" },
    { name: "07-alu-late", time: staticContract.scenes.aluLate, selector: "#v2-native-alu-propagation .v2-active-wire", focus: "late native ripple ALU" },
    { name: "08-vram", time: staticContract.scenes.vram, selector: "#v2-exact-memory-cell-activity > g[data-memory-owner=\"vram\"]", focus: "native VRAM page/byte" },
    { name: "09-gpu", time: staticContract.scenes.gpu, selector: "#v2-native-bus-propagation .v2-active-wire[data-stage=\"dma_data_latch\"]", focus: "native DMA latch" },
    { name: "10-late-memory", time: staticContract.scenes.lateMemory, selector: "#v2-exact-memory-cell-activity > g", focus: "late native memory access" },
    { name: "11-final-crt", time: 58.0, selector: "#v2-final-crt-focus", focus: "terminal native Space Invaders CRT" },
  ];

  const manifest = [];
  for (const checkpoint of checkpoints) {
    await seek(page, checkpoint.time);
    await page.waitForTimeout(80);
    if (checkpoint.selector) {
      const activeCount = await visibleCount(page, checkpoint.selector);
      if (activeCount <= 0) throw new Error(`Camera ${checkpoint.name} is not synchronized with ${checkpoint.selector} at ${checkpoint.time}s`);
      checkpoint.activityScore = activeCount;
    }

    const state = await inspectFrame(page);
    if (state.labelFailures.length > 0) throw new Error(`Node labels escape their physical components at ${checkpoint.time}s: ${JSON.stringify(state.labelFailures.slice(0, 8))}`);
    if (state.textOverlaps.length > 0) throw new Error(`Readable node labels overlap at ${checkpoint.time}s: ${JSON.stringify(state.textOverlaps)}`);
    if (state.rasterTransform !== "translate(950.000 127.000) scale(1.5000000 1.5000000)" || state.rasterClip !== null) throw new Error(`CRT raster must be exact 4:3, uniformly scaled and unclipped: ${JSON.stringify(state)}`);

    const isFinalCrt = checkpoint.name === "11-final-crt";
    if (!isFinalCrt && checkpoint.time >= 2.5 && state.visibleCrtFrames.length !== 1) throw new Error(`Exactly one sidebar native VRAM framebuffer must be visible at ${checkpoint.time}s: ${JSON.stringify(state.visibleCrtFrames)}`);
    if (isFinalCrt && state.visibleCrtFrames.length > 1) throw new Error(`At most one held sidebar framebuffer may remain beneath the terminal CRT: ${JSON.stringify(state.visibleCrtFrames)}`);
    for (const frame of state.visibleCrtFrames) if (frame.box.width > 192.5 || frame.box.height > 144.5) throw new Error(`Native framebuffer escapes the 192x144 CRT raster at ${checkpoint.time}s: ${JSON.stringify(frame)}`);
    if (isFinalCrt && (!state.finalFocus.visible || state.finalFocus.raster.width < 755 || state.finalFocus.raster.height < 565)) throw new Error(`Terminal CRT is not full readable focus at ${checkpoint.time}s: ${JSON.stringify(state.finalFocus)}`);

    const file = `${checkpoint.name}.png`;
    await root.screenshot({ path: path.join(outputDir, file), animations: "allow" });
    manifest.push({ ...checkpoint, file, state });
    console.log(`captured ${checkpoint.time.toFixed(4)}s ${checkpoint.focus} activity=${checkpoint.activityScore ?? "n/a"} -> ${file}`);
  }

  await writeFile(path.join(outputDir, "manifest.json"), `${JSON.stringify({ source: path.basename(input), staticContract, checkpoints: manifest }, null, 2)}\n`, "utf8");
} finally {
  await browser.close();
}

async function visibleCount(page, selector) {
  return page.evaluate((candidateSelector) => [...document.querySelectorAll(candidateSelector)].filter((element) => Number.parseFloat(getComputedStyle(element).opacity || "0") > 0.05).length, selector);
}

async function inspectFrame(page) {
  return page.evaluate(() => {
    const viewportRect = document.querySelector("#v2-machine-viewport")?.getBoundingClientRect();
    const labelFailures = [];
    const visibleTitleRects = [];
    for (const group of document.querySelectorAll("#v3-node-labels > g")) {
      const id = group.getAttribute("data-label-for-node");
      const node = document.querySelector(`#v2-node-${CSS.escape(id)}`);
      const title = group.querySelector(".v3-title");
      if (!node || !title || !viewportRect) continue;
      const nodeRect = node.getBoundingClientRect();
      const titleRect = title.getBoundingClientRect();
      if (!intersects(nodeRect, viewportRect, 0)) continue;
      const tolerance = Math.max(2, nodeRect.width * 0.035);
      if (titleRect.x < nodeRect.x - tolerance || titleRect.y < nodeRect.y - tolerance || titleRect.x + titleRect.width > nodeRect.x + nodeRect.width + tolerance || titleRect.y + titleRect.height > nodeRect.y + nodeRect.height + tolerance) labelFailures.push({ id, node: rect(nodeRect), label: rect(titleRect) });
      if (titleRect.width > 1 && titleRect.height > 1 && intersects(titleRect, viewportRect, 0)) visibleTitleRects.push({ id, rect: rect(titleRect) });
    }

    const textOverlaps = [];
    for (let i = 0; i < visibleTitleRects.length; i += 1) {
      for (let j = i + 1; j < visibleTitleRects.length; j += 1) {
        if (intersects(visibleTitleRects[i].rect, visibleTitleRects[j].rect, 1.5)) {
          textOverlaps.push([visibleTitleRects[i].id, visibleTitleRects[j].id]);
          if (textOverlaps.length >= 12) break;
        }
      }
      if (textOverlaps.length >= 12) break;
    }

    const visibleCrtFrames = [...document.querySelectorAll("#v2-crt .v2-crt-pixel")]
      .filter((element) => Number.parseFloat(getComputedStyle(element).opacity || "0") > 0.5)
      .map((element) => ({ frame: element.getAttribute("data-vram-frame"), pixels: element.getAttribute("data-vram-pixels"), checksum: element.getAttribute("data-vram-checksum"), box: rect(element.getBoundingClientRect()) }));
    const crtRasterGroup = [...document.querySelectorAll("#v2-crt g")].find((element) => element.getAttribute("transform")?.includes("scale(1.5000000 1.5000000)"));
    const finalFocus = document.querySelector("#v2-final-crt-focus");
    const finalRaster = finalFocus?.querySelector("[data-final-native-raster]");
    return {
      labelFailures, textOverlaps, visibleLabels: visibleTitleRects.length, visibleCrtFrames,
      rasterTransform: crtRasterGroup?.getAttribute("transform") ?? null,
      rasterClip: crtRasterGroup?.getAttribute("clip-path") ?? null,
      finalFocus: {
        visible: Boolean(finalFocus && Number.parseFloat(getComputedStyle(finalFocus).opacity || "0") > 0.5),
        raster: finalRaster ? rect(finalRaster.getBoundingClientRect()) : { x: 0, y: 0, width: 0, height: 0 },
      },
      activeWires: [...document.querySelectorAll("#v2-native-bus-propagation .v2-active-wire, #v2-native-alu-propagation .v2-active-wire")].filter((element) => Number.parseFloat(getComputedStyle(element).opacity || "0") > 0.05).length,
    };

    function rect(value) { return { x: value.x, y: value.y, width: value.width, height: value.height }; }
    function intersects(a, b, tolerance = 1.5) { return a.x + a.width - tolerance > b.x && b.x + b.width - tolerance > a.x && a.y + a.height - tolerance > b.y && b.y + b.height - tolerance > a.y; }
  });
}

async function seek(page, time) {
  await page.evaluate((target) => {
    const svgRoot = document.querySelector("svg");
    svgRoot.pauseAnimations();
    svgRoot.setCurrentTime(target);
  }, time);
}
