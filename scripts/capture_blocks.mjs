import { chromium } from "playwright";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const baseUrl = process.env.LEADER_EXPLORER_URL ?? "http://127.0.0.1:8080/explorer-web/";
const outputDir = path.resolve(process.env.LEADER_CAPTURE_OUTPUT ?? "generated/screenshots");
const captureWidth = Number(process.env.LEADER_CAPTURE_WIDTH ?? 1800);

if (!Number.isFinite(captureWidth) || captureWidth < 800) {
  throw new Error(`Invalid LEADER_CAPTURE_WIDTH: ${process.env.LEADER_CAPTURE_WIDTH}`);
}

function slugify(value) {
  return value
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-zA-Z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .toLowerCase();
}

function parseViewInfo(value) {
  const [label = "", id = "", density = ""] = value.split("\n");
  return { label, id, density };
}

async function waitForExplorer(page) {
  await page.waitForSelector("#child-views button", { state: "visible" });
  await page.waitForFunction(() => {
    const viewInfo = document.querySelector("#view-info")?.textContent ?? "";
    const svg = document.querySelector("#hardware-canvas");
    return viewInfo.includes("view-") && svg?.querySelectorAll(".hardware-node").length > 0;
  });
}

async function installCaptureSurface(page) {
  await page.addStyleTag({
    content: `
      html, body, .app-shell, .workspace, .canvas-panel {
        width: 100% !important;
        height: 100% !important;
        min-width: 0 !important;
        min-height: 0 !important;
        margin: 0 !important;
        padding: 0 !important;
        overflow: hidden !important;
      }

      body {
        background: #070b10 !important;
      }

      .app-shell {
        display: block !important;
      }

      .topbar,
      .breadcrumb,
      .left-panel,
      .right-panel,
      .canvas-hint {
        display: none !important;
      }

      .workspace {
        display: block !important;
        background: #070b10 !important;
      }

      .canvas-panel {
        position: fixed !important;
        inset: 0 !important;
        display: block !important;
        border: 0 !important;
        background: #070b10 !important;
      }

      #hardware-canvas {
        display: block !important;
        width: 100vw !important;
        height: 100vh !important;
        min-width: 0 !important;
        min-height: 0 !important;
      }
    `,
  });
}

await mkdir(outputDir, { recursive: true });

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({
  viewport: { width: captureWidth, height: 1100 },
  deviceScaleFactor: 1,
});
await page.emulateMedia({ reducedMotion: "reduce" });

try {
  await page.goto(baseUrl, { waitUntil: "networkidle" });
  await waitForExplorer(page);

  // The machine view exposes the canonical subsystem views owned by Rust.
  const subsystemViews = await page.locator("#child-views button").evaluateAll((buttons) =>
    buttons.map((button, index) => ({
      index,
      label: button.textContent?.trim() ?? `subsystem-${index + 1}`,
    })),
  );

  if (subsystemViews.length === 0) {
    throw new Error("Explorer exposed no canonical subsystem views");
  }

  const manifest = [];

  for (const [captureIndex, subsystem] of subsystemViews.entries()) {
    await page.goto(baseUrl, { waitUntil: "networkidle" });
    await waitForExplorer(page);

    const button = page.locator("#child-views button").nth(subsystem.index);
    await button.click();
    await page.waitForFunction(
      (machineLabel) => {
        const info = document.querySelector("#view-info")?.textContent ?? "";
        return info.includes("view-") && !info.startsWith(`${machineLabel}\n`);
      },
      "LEADER MACHINE",
    );

    await page.locator("#fit-button").click();

    const view = await page.evaluate(() => {
      const svg = document.querySelector("#hardware-canvas");
      const info = document.querySelector("#view-info")?.textContent ?? "";
      if (!svg) throw new Error("Missing hardware canvas");
      const box = svg.viewBox.baseVal;
      return {
        info,
        bounds: { x: box.x, y: box.y, w: box.width, h: box.height },
        nodeCount: svg.querySelectorAll(".hardware-node").length,
        linkCount: svg.querySelectorAll(".hardware-link").length,
      };
    });

    if (!(view.bounds.w > 0) || !(view.bounds.h > 0)) {
      throw new Error(`Invalid canonical bounds for ${subsystem.label}`);
    }

    const captureHeight = Math.max(1, Math.round(captureWidth * (view.bounds.h / view.bounds.w)));
    await page.setViewportSize({ width: captureWidth, height: captureHeight });
    await installCaptureSurface(page);
    await page.waitForAnimationFrame?.();

    const metadata = parseViewInfo(view.info);
    const prefix = String(captureIndex + 1).padStart(2, "0");
    const filename = `${prefix}-${slugify(metadata.label || subsystem.label)}.png`;
    const outputPath = path.join(outputDir, filename);

    await page.locator("#hardware-canvas").screenshot({
      path: outputPath,
      animations: "disabled",
    });

    manifest.push({
      order: captureIndex + 1,
      id: metadata.id,
      label: metadata.label || subsystem.label,
      density: metadata.density,
      bounds: view.bounds,
      nodeCount: view.nodeCount,
      linkCount: view.linkCount,
      width: captureWidth,
      height: captureHeight,
      file: filename,
    });

    console.log(`captured ${metadata.id} -> ${filename} (${captureWidth}x${captureHeight})`);
  }

  await writeFile(
    path.join(outputDir, "manifest.json"),
    `${JSON.stringify({ generatedAt: new Date().toISOString(), views: manifest }, null, 2)}\n`,
    "utf8",
  );
} finally {
  await browser.close();
}
