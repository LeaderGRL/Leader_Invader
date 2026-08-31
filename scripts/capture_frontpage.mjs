import { chromium } from "playwright";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const input = path.resolve(process.env.LEADER_SVG ?? "generated/Leader.svg");
const outputDir = path.resolve(process.env.LEADER_FRONT_PAGE_CAPTURE_OUTPUT ?? "generated/frontpage-captures");
const svg = await readFile(input, "utf8");

const checkpoints = [
  { name: "01-assembled-machine", time: 44.0 },
  { name: "02-microcode-control-rom", time: 58.0 },
  { name: "03-live-ripple-alu", time: 76.0 },
  { name: "04-live-work-ram", time: 95.0 },
  { name: "05-video-scanout-game", time: 126.0 },
];

await mkdir(outputDir, { recursive: true });

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({
  viewport: { width: 1200, height: 675 },
  deviceScaleFactor: 1,
});

try {
  await page.setContent(`<!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;width:1200px;height:675px;overflow:hidden;background:#070b11}svg{display:block;width:1200px;height:675px}</style></head><body>${svg}</body></html>`, {
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

  const manifest = [];
  for (const checkpoint of checkpoints) {
    await page.evaluate((time) => {
      const svgRoot = document.querySelector("svg");
      svgRoot.pauseAnimations();
      svgRoot.setCurrentTime(time);
    }, checkpoint.time);
    await page.waitForTimeout(60);

    const file = `${checkpoint.name}.png`;
    await root.screenshot({
      path: path.join(outputDir, file),
      animations: "allow",
    });
    manifest.push({ ...checkpoint, file });
    console.log(`captured ${checkpoint.time.toFixed(1)}s -> ${file}`);
  }

  await writeFile(
    path.join(outputDir, "manifest.json"),
    `${JSON.stringify({ source: path.basename(input), checkpoints: manifest }, null, 2)}\n`,
    "utf8",
  );
} finally {
  await browser.close();
}
