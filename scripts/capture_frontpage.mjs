import { chromium } from "playwright";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const input = path.resolve(process.env.LEADER_SVG ?? "generated/Leader.svg");
const outputDir = path.resolve(process.env.LEADER_FRONT_PAGE_CAPTURE_OUTPUT ?? "generated/frontpage-captures");
const svg = await readFile(input, "utf8");

const checkpoints = [
  { name: "01-power-on-full-die", time: 0.0 },
  { name: "02-native-bus-propagation", time: 4.5 },
  { name: "03-ripple-alu-propagation", time: 18.0 },
  { name: "04-exact-memory-cell", time: 34.0 },
  { name: "05-native-crt-gameplay", time: 50.0 },
];

await mkdir(outputDir, { recursive: true });

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({
  viewport: { width: 1200, height: 675 },
  deviceScaleFactor: 1,
});

try {
  await page.setContent(`<!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;width:1200px;height:675px;overflow:hidden;background:#04080d}svg{display:block;width:1200px;height:675px}</style></head><body>${svg}</body></html>`, {
    waitUntil: "load",
  });

  const root = page.locator("svg").first();
  const supportsTimeline = await page.evaluate(() => {
    const svgRoot = document.querySelector("svg");
    return Boolean(svgRoot && typeof svgRoot.setCurrentTime === "function" && typeof svgRoot.pauseAnimations === "function");
  });
  if (!supportsTimeline) {
    throw new Error("Browser SVG timeline seeking is unavailable");
  }

  const staticContract = await page.evaluate(() => {
    const machine = document.querySelector("#v2-machine");
    const logic = document.querySelector("#v2-logic-nodes");
    const memory = document.querySelector("#v2-memory-byte-fabric");
    const machineRect = machine?.getBoundingClientRect();
    const logicRect = logic?.getBoundingClientRect();
    const memoryRect = memory?.getBoundingClientRect();
    return {
      nodes: document.querySelectorAll("#v2-logic-nodes > g").length,
      staticWires: document.querySelectorAll("#v2-static-wires > use").length,
      memoryPages: document.querySelectorAll("#v2-memory-byte-fabric [data-memory-page]").length,
      particles: document.querySelectorAll("animateMotion").length,
      cameraAnimations: document.querySelectorAll('animate[attributeName="viewBox"]').length,
      machineClipPath: machine?.getAttribute("clip-path") ?? null,
      machineRect: machineRect && { x: machineRect.x, y: machineRect.y, width: machineRect.width, height: machineRect.height },
      logicRect: logicRect && { x: logicRect.x, y: logicRect.y, width: logicRect.width, height: logicRect.height },
      memoryRect: memoryRect && { x: memoryRect.x, y: memoryRect.y, width: memoryRect.width, height: memoryRect.height },
    };
  });

  if (staticContract.nodes === 0 || staticContract.staticWires === 0) {
    throw new Error(`Physical die missing at t=0: ${JSON.stringify(staticContract)}`);
  }
  if (staticContract.memoryPages !== 136) {
    throw new Error(`Expected 136 physical memory pages, got ${staticContract.memoryPages}`);
  }
  if (staticContract.particles !== 0 || staticContract.cameraAnimations !== 0) {
    throw new Error(`V2 must not contain particle/camera motion: ${JSON.stringify(staticContract)}`);
  }
  if (staticContract.machineClipPath !== null) {
    throw new Error(`Physical die must not be clipped in transformed user space: ${staticContract.machineClipPath}`);
  }
  if (!staticContract.logicRect || staticContract.logicRect.width < 850 || staticContract.logicRect.height < 400) {
    throw new Error(`Physical logic die is cropped or undersized: ${JSON.stringify(staticContract.logicRect)}`);
  }
  if (!staticContract.memoryRect || staticContract.memoryRect.width < 350 || staticContract.memoryRect.height < 250) {
    throw new Error(`Memory fabric is cropped or undersized: ${JSON.stringify(staticContract.memoryRect)}`);
  }

  const manifest = [];
  for (const checkpoint of checkpoints) {
    await page.evaluate((time) => {
      const svgRoot = document.querySelector("svg");
      svgRoot.pauseAnimations();
      svgRoot.setCurrentTime(time);
    }, checkpoint.time);
    await page.waitForTimeout(60);

    const visible = await page.evaluate(() => {
      const root = document.querySelector("#v2-machine");
      const node = document.querySelector("#v2-logic-nodes > g");
      const wire = document.querySelector("#v2-static-wires > use");
      return {
        machine: root ? getComputedStyle(root).display !== "none" : false,
        node: node ? getComputedStyle(node).display !== "none" && getComputedStyle(node).opacity !== "0" : false,
        wire: wire ? getComputedStyle(wire).display !== "none" && getComputedStyle(wire).opacity !== "0" : false,
      };
    });
    if (!visible.machine || !visible.node || !visible.wire) {
      throw new Error(`Dangling/hidden physical die at ${checkpoint.time}s: ${JSON.stringify(visible)}`);
    }

    const file = `${checkpoint.name}.png`;
    await root.screenshot({
      path: path.join(outputDir, file),
      animations: "allow",
    });
    manifest.push({ ...checkpoint, file, visible });
    console.log(`captured ${checkpoint.time.toFixed(1)}s -> ${file}`);
  }

  await writeFile(
    path.join(outputDir, "manifest.json"),
    `${JSON.stringify({ source: path.basename(input), staticContract, checkpoints: manifest }, null, 2)}\n`,
    "utf8",
  );
} finally {
  await browser.close();
}
