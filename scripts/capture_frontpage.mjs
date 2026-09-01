import { chromium } from "playwright";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const input = path.resolve(process.env.LEADER_SVG ?? "generated/Leader.svg");
const outputDir = path.resolve(process.env.LEADER_FRONT_PAGE_CAPTURE_OUTPUT ?? "generated/frontpage-captures");
const svgText = await readFile(input, "utf8");
await mkdir(outputDir, { recursive: true });

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1200, height: 675 }, deviceScaleFactor: 1 });

try {
  await page.setContent(`<!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;width:1200px;height:675px;overflow:hidden;background:#04080d}svg{display:block;width:1200px;height:675px}</style></head><body>${svgText}</body></html>`, { waitUntil: "load" });
  const root = page.locator("svg").first();
  const timelineAvailable = await page.evaluate(() => {
    const root = document.querySelector("svg");
    return Boolean(root && typeof root.setCurrentTime === "function" && typeof root.pauseAnimations === "function");
  });
  if (!timelineAvailable) throw new Error("Browser SVG timeline seeking is unavailable");
  await page.evaluate(() => document.querySelector("svg").pauseAnimations());

  const contract = await readContract(page);
  validateStaticContract(contract);

  const checkpoints = buildCheckpoints(contract)
    .sort((left, right) => left.time - right.time)
    .map((checkpoint, index) => ({
      ...checkpoint,
      name: `${String(index + 1).padStart(2, "0")}-${checkpoint.slug}`,
    }));

  const manifest = [];
  for (const checkpoint of checkpoints) {
    await seek(page, checkpoint.time);
    await page.waitForTimeout(80);

    if (checkpoint.selector) {
      const count = await visibleCount(page, checkpoint.selector);
      if (count <= 0) {
        throw new Error(`Native activity for ${checkpoint.slug} is not visible at ${checkpoint.time}s: ${checkpoint.selector}`);
      }
      checkpoint.activityScore = count;
    }

    const state = await inspectFrame(page);
    if (state.labelFailures.length) {
      throw new Error(`Node labels escape physical components at ${checkpoint.time}s: ${JSON.stringify(state.labelFailures.slice(0, 8))}`);
    }
    if (state.textOverlaps.length) {
      throw new Error(`Readable node labels overlap at ${checkpoint.time}s: ${JSON.stringify(state.textOverlaps)}`);
    }
    if (state.rasterTransform !== "translate(950.000 127.000) scale(1.5000000 1.5000000)" || state.rasterClip !== null) {
      throw new Error(`Sidebar CRT raster contract failed: ${JSON.stringify(state)}`);
    }

    if (checkpoint.pose) assertCameraPose(checkpoint, state.cameraMatrix);

    if (checkpoint.slug === "live-crt") {
      if (!state.finalFocus.visible || state.finalFocus.raster.width < 755 || state.finalFocus.raster.height < 565) {
        throw new Error(`Large native CRT is not fully readable: ${JSON.stringify(state.finalFocus)}`);
      }
      if (state.finalFocus.visibleNativeFrames !== 1) {
        throw new Error(`Large native CRT must display exactly one checkpoint at a time: ${JSON.stringify(state.finalFocus)}`);
      }
    } else if (checkpoint.time >= 2.5 && state.visibleCrtFrames.length !== 1) {
      throw new Error(`Sidebar CRT must expose exactly one native framebuffer at ${checkpoint.time}s: ${JSON.stringify(state.visibleCrtFrames)}`);
    }

    for (const frame of state.visibleCrtFrames) {
      if (frame.box.width > 192.5 || frame.box.height > 144.5) {
        throw new Error(`Sidebar native framebuffer escapes 192x144 raster: ${JSON.stringify(frame)}`);
      }
    }

    const file = `${checkpoint.name}.png`;
    await root.screenshot({ path: path.join(outputDir, file), animations: "allow" });
    manifest.push({ ...checkpoint, file, state });
    console.log(`captured ${checkpoint.time.toFixed(4)}s ${checkpoint.focus} -> ${file}`);
  }

  await writeFile(
    path.join(outputDir, "manifest.json"),
    `${JSON.stringify({ source: path.basename(input), contract, checkpoints: manifest }, null, 2)}\n`,
    "utf8",
  );
} finally {
  await browser.close();
}

async function readContract(page) {
  return page.evaluate(() => {
    const camera = document.querySelector("#v2-camera-contract");
    const finalFocus = document.querySelector("#v2-final-crt-focus");
    const viewport = document.querySelector("#v2-machine-viewport");
    const machine = document.querySelector("#v2-machine");
    const clipRect = document.querySelector("#v2-machine-clip rect");
    const bitFabric = document.querySelector("#v2-memory-bitcell-fabric");
    const microFabric = document.querySelector("#v2-microcode-bitcell-fabric");
    const scene = (name) => ({
      time: Number(camera?.getAttribute(`data-scene-${name}`)),
      pose: {
        tx: Number(camera?.getAttribute(`data-scene-${name}-tx`)),
        ty: Number(camera?.getAttribute(`data-scene-${name}-ty`)),
        scale: Number(camera?.getAttribute(`data-scene-${name}-scale`)),
      },
    });
    const focusStart = Number(finalFocus?.getAttribute("data-focus-start"));
    const focusEnd = Number(finalFocus?.getAttribute("data-focus-end"));
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
      viewportClip: viewport?.getAttribute("clip-path") ?? null,
      machineClip: machine?.getAttribute("clip-path") ?? null,
      clip: clipRect ? {
        x: Number(clipRect.getAttribute("x")),
        width: Number(clipRect.getAttribute("width")),
      } : null,
      memoryBitCells: bitFabric?.getAttribute("aria-label") ?? null,
      microcodeCells: microFabric?.getAttribute("data-microcode-cells") ?? null,
      finalCrtSource: finalFocus?.getAttribute("data-final-focus") ?? null,
      finalCrtLive: finalFocus?.getAttribute("data-showcase-live") ?? null,
      finalCrtAlive: Number(finalFocus?.getAttribute("data-showcase-alive")),
      finalCrtScore: Number(finalFocus?.getAttribute("data-showcase-score")),
      finalCrtFrameCount: document.querySelectorAll("#v2-final-crt-focus [data-showcase-vram-frame]").length,
      finalCrtRaster: document.querySelector("#v2-final-crt-focus [data-final-native-raster]")?.getAttribute("data-final-native-raster") ?? null,
      finalCrtFocusTime: Number.isFinite(focusStart) && Number.isFinite(focusEnd) ? (focusStart + focusEnd) * 0.5 : NaN,
      scenes: {
        fetch: scene("fetch"),
        micro: scene("micro"),
        rom: scene("rom"),
        alu: scene("alu"),
        ram: scene("ram"),
        vram: scene("vram"),
        gpu: scene("gpu"),
        lateMemory: scene("late-memory"),
      },
    };
  });
}

function validateStaticContract(contract) {
  if (contract.nodes < 498 || contract.nodeLabels !== contract.nodes) throw new Error(`Physical node labeling incomplete: ${JSON.stringify(contract)}`);
  if (contract.staticWires < 1000) throw new Error(`Physical wiring incomplete: ${JSON.stringify(contract)}`);
  if (contract.memoryPages !== 136 || contract.bitcellPages !== 136) throw new Error(`Memory fabric incomplete: ${JSON.stringify(contract)}`);
  if (contract.memoryBitCells !== "278528 physical memory bit-cell sites" || contract.microcodeCells !== "6144") throw new Error(`Bit-level fabrics incomplete: ${JSON.stringify(contract)}`);
  if (contract.particles !== 0 || contract.rootViewBoxAnimations !== 0) throw new Error(`Forbidden presentation animation found: ${JSON.stringify(contract)}`);
  if (!contract.cameraTranslate || !contract.cameraScale || contract.viewportClip !== "url(#v2-machine-clip)") throw new Error(`Technical camera rig incomplete: ${JSON.stringify(contract)}`);
  if (contract.machineClip !== null) throw new Error(`Raw transformed machine must not carry clipPath: ${contract.machineClip}`);
  if (!contract.clip || contract.clip.x !== 24 || contract.clip.width !== 900) throw new Error(`Hardware viewport contract failed: ${JSON.stringify(contract.clip)}`);
  if (contract.finalCrtSource !== "native-vram" || contract.finalCrtLive !== "true" || contract.finalCrtRaster !== "128x96") throw new Error(`Large CRT is not native VRAM: ${JSON.stringify(contract)}`);
  if (contract.finalCrtAlive < 8 || contract.finalCrtScore >= 320 || contract.finalCrtFrameCount < 8 || !Number.isFinite(contract.finalCrtFocusTime)) throw new Error(`Large CRT selected a completed/dead match state: ${JSON.stringify(contract)}`);
  for (const [name, scene] of Object.entries(contract.scenes)) {
    if (!Number.isFinite(scene.time) || scene.time <= 0 || scene.time >= 55) throw new Error(`Scene ${name} has invalid native time: ${JSON.stringify(scene)}`);
    if (![scene.pose.tx, scene.pose.ty, scene.pose.scale].every(Number.isFinite) || scene.pose.scale <= 0) throw new Error(`Scene ${name} has invalid expected camera pose: ${JSON.stringify(scene)}`);
  }
}

function buildCheckpoints(contract) {
  return [
    { slug: "overview", time: 0.25, focus: "full die" },
    { slug: "fetch-decode", ...contract.scenes.fetch, selector: "#v2-native-bus-propagation .v2-active-wire", focus: "PC + native fetch/decode" },
    { slug: "microcode", ...contract.scenes.micro, selector: "#v2-microcode-bitcell-fabric > g", focus: "single native control-ROM visit" },
    { slug: "rom", ...contract.scenes.rom, selector: "#v2-exact-memory-cell-activity > g[data-memory-owner=\"rom\"]", focus: "native ROM page/byte" },
    { slug: "alu", ...contract.scenes.alu, selector: "#v2-native-alu-propagation .v2-active-wire", focus: "native ripple ALU" },
    { slug: "ram", ...contract.scenes.ram, selector: "#v2-exact-memory-cell-activity > g[data-memory-owner=\"ram\"]", focus: "native RAM page/byte" },
    { slug: "vram", ...contract.scenes.vram, selector: "#v2-exact-memory-cell-activity > g[data-memory-owner=\"vram\"]", focus: "native VRAM page/byte" },
    { slug: "gpu", ...contract.scenes.gpu, selector: "#v2-native-bus-propagation .v2-active-wire[data-stage=\"dma_data_latch\"]", focus: "native DMA latch" },
    { slug: "late-memory", ...contract.scenes.lateMemory, selector: "#v2-exact-memory-cell-activity > g", focus: "late native memory access" },
    { slug: "live-crt", time: contract.finalCrtFocusTime, selector: "#v2-final-crt-focus", focus: "active native Space Invaders replay" },
  ];
}

function assertCameraPose(checkpoint, matrix) {
  if (!matrix) throw new Error(`Camera matrix missing at ${checkpoint.slug}`);
  const scaleTolerance = 0.004;
  const pixelTolerance = 2.0;
  if (
    Math.abs(matrix.a - checkpoint.pose.scale) > scaleTolerance
    || Math.abs(matrix.d - checkpoint.pose.scale) > scaleTolerance
    || Math.abs(matrix.e - checkpoint.pose.tx) > pixelTolerance
    || Math.abs(matrix.f - checkpoint.pose.ty) > pixelTolerance
  ) {
    throw new Error(`Camera ${checkpoint.slug} is looking at the wrong subsystem. expected=${JSON.stringify(checkpoint.pose)} actual=${JSON.stringify(matrix)}`);
  }
}

async function visibleCount(page, selector) {
  return page.evaluate((candidate) => [...document.querySelectorAll(candidate)]
    .filter((element) => Number.parseFloat(getComputedStyle(element).opacity || "0") > 0.05).length, selector);
}

async function inspectFrame(page) {
  return page.evaluate(() => {
    const viewport = document.querySelector("#v2-machine-viewport")?.getBoundingClientRect();
    const labelFailures = [];
    const visibleLabels = [];
    for (const group of document.querySelectorAll("#v3-node-labels > g")) {
      const id = group.getAttribute("data-label-for-node");
      const node = document.querySelector(`#v2-node-${CSS.escape(id)}`);
      const title = group.querySelector(".v3-title");
      if (!node || !title || !viewport) continue;
      const nodeRect = node.getBoundingClientRect();
      const titleRect = title.getBoundingClientRect();
      if (!intersects(nodeRect, viewport, 0)) continue;
      const tolerance = Math.max(2, nodeRect.width * 0.035);
      if (titleRect.x < nodeRect.x - tolerance || titleRect.y < nodeRect.y - tolerance || titleRect.right > nodeRect.right + tolerance || titleRect.bottom > nodeRect.bottom + tolerance) {
        labelFailures.push({ id, node: rect(nodeRect), label: rect(titleRect) });
      }
      if (titleRect.width > 1 && titleRect.height > 1 && intersects(titleRect, viewport, 0)) visibleLabels.push({ id, rect: rect(titleRect) });
    }

    const textOverlaps = [];
    for (let left = 0; left < visibleLabels.length; left += 1) {
      for (let right = left + 1; right < visibleLabels.length; right += 1) {
        if (intersects(visibleLabels[left].rect, visibleLabels[right].rect, 1.5)) {
          textOverlaps.push([visibleLabels[left].id, visibleLabels[right].id]);
          if (textOverlaps.length >= 12) break;
        }
      }
      if (textOverlaps.length >= 12) break;
    }

    const visibleCrtFrames = [...document.querySelectorAll("#v2-crt .v2-crt-pixel")]
      .filter((element) => Number.parseFloat(getComputedStyle(element).opacity || "0") > 0.5)
      .map((element) => ({
        frame: element.getAttribute("data-vram-frame"),
        checksum: element.getAttribute("data-vram-checksum"),
        box: rect(element.getBoundingClientRect()),
      }));
    const rasterGroup = [...document.querySelectorAll("#v2-crt g")]
      .find((element) => element.getAttribute("transform")?.includes("scale(1.5000000 1.5000000)"));
    const finalFocus = document.querySelector("#v2-final-crt-focus");
    const finalRaster = finalFocus?.querySelector("[data-final-native-raster]");
    const visibleNativeFrames = finalFocus
      ? [...finalFocus.querySelectorAll("[data-showcase-vram-frame]")]
        .filter((element) => Number.parseFloat(getComputedStyle(element).opacity || "0") > 0.5).length
      : 0;
    const ctm = document.querySelector("#v2-camera-scale")?.getCTM();

    return {
      labelFailures,
      textOverlaps,
      rasterTransform: rasterGroup?.getAttribute("transform") ?? null,
      rasterClip: rasterGroup?.getAttribute("clip-path") ?? null,
      visibleCrtFrames,
      cameraMatrix: ctm ? { a: ctm.a, d: ctm.d, e: ctm.e, f: ctm.f } : null,
      finalFocus: {
        visible: Boolean(finalFocus && Number.parseFloat(getComputedStyle(finalFocus).opacity || "0") > 0.5),
        visibleNativeFrames,
        raster: finalRaster ? rect(finalRaster.getBoundingClientRect()) : { x: 0, y: 0, width: 0, height: 0 },
      },
    };

    function rect(value) {
      return { x: value.x, y: value.y, width: value.width, height: value.height };
    }
    function intersects(a, b, tolerance = 1.5) {
      return a.x + a.width - tolerance > b.x
        && b.x + b.width - tolerance > a.x
        && a.y + a.height - tolerance > b.y
        && b.y + b.height - tolerance > a.y;
    }
  });
}

async function seek(page, time) {
  await page.evaluate((target) => {
    const root = document.querySelector("svg");
    root.pauseAnimations();
    root.setCurrentTime(target);
  }, time);
}
