import init, { Explorer, Playback } from "./pkg/leader_explorer.js";

await init();

const explorer = new Explorer();
const playback = new Playback();
const svg = document.querySelector("#hardware-canvas");
const breadcrumb = document.querySelector("#breadcrumb");
const childViews = document.querySelector("#child-views");
const viewInfo = document.querySelector("#view-info");
const inspector = document.querySelector("#node-inspector");
const playbackState = document.querySelector("#playback-state");
const progressFill = document.querySelector("#progress-fill");
const timelineScrubber = document.querySelector("#timeline-scrubber");
const timelineLabel = document.querySelector("#timeline-label");
const NS = "http://www.w3.org/2000/svg";

let dragging = false;
let dragDistance = 0;
let lastPointer = null;
let rafHandle = 0;
let highlightedNode = null;

function parseJson(value) {
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
}

function center(bounds) {
  return {
    x: bounds.x + bounds.w * 0.5,
    y: bounds.y + bounds.h * 0.5,
  };
}

function hex(value, width) {
  if (value === null || value === undefined) return "—";
  return `0x${Number(value).toString(16).padStart(width, "0")}`;
}

function worldPoint(event) {
  const camera = parseJson(explorer.camera_json());
  const rect = svg.getBoundingClientRect();
  return {
    x: camera.x + ((event.clientX - rect.left) / rect.width) * camera.w,
    y: camera.y + ((event.clientY - rect.top) / rect.height) * camera.h,
  };
}

function makeSvg(tag, attributes = {}) {
  const element = document.createElementNS(NS, tag);
  for (const [name, value] of Object.entries(attributes)) {
    element.setAttribute(name, String(value));
  }
  return element;
}

function setHighlightedNode(nodeId) {
  if (highlightedNode === nodeId) return;
  if (highlightedNode) {
    svg.querySelector(`[data-node-id="${CSS.escape(highlightedNode)}"]`)?.classList.remove("is-highlighted");
  }
  highlightedNode = nodeId;
  if (highlightedNode) {
    svg.querySelector(`[data-node-id="${CSS.escape(highlightedNode)}"]`)?.classList.add("is-highlighted");
  }
}

function renderGraph() {
  const graph = parseJson(explorer.current_view_graph_json());
  const camera = parseJson(explorer.camera_json());
  const currentView = parseJson(explorer.current_view_json());
  const crumbs = parseJson(explorer.breadcrumb_json()) ?? [];
  const children = parseJson(explorer.child_views_json()) ?? [];
  if (!graph || !camera || !currentView) return;

  svg.replaceChildren();
  svg.setAttribute("viewBox", `${camera.x} ${camera.y} ${camera.w} ${camera.h}`);

  const nodeMap = new Map(graph.nodes.map((node) => [node.id, node]));
  const linkLayer = makeSvg("g", { class: "link-layer" });
  const nodeLayer = makeSvg("g", { class: "node-layer" });

  for (const link of graph.links) {
    const from = nodeMap.get(link.from);
    const to = nodeMap.get(link.to);
    if (!from || !to) continue;
    const a = center(from.bounds);
    const b = center(to.bounds);
    linkLayer.append(makeSvg("line", {
      x1: a.x,
      y1: a.y,
      x2: b.x,
      y2: b.y,
      class: `hardware-link signal-${link.signal}`,
      "data-link-id": link.id,
    }));
  }

  const cameraScale = camera.w / Math.max(svg.clientWidth, 1);
  const showLabels = cameraScale < 4.5;
  for (const node of graph.nodes) {
    const group = makeSvg("g", {
      class: `hardware-node${highlightedNode === node.id ? " is-highlighted" : ""}`,
      "data-node-id": node.id,
    });
    group.append(makeSvg("rect", {
      x: node.bounds.x,
      y: node.bounds.y,
      width: node.bounds.w,
      height: node.bounds.h,
      rx: Math.min(10, node.bounds.h * 0.12),
    }));
    if (showLabels) {
      const label = makeSvg("text", {
        x: node.bounds.x + node.bounds.w * 0.5,
        y: node.bounds.y + node.bounds.h * 0.52,
        "text-anchor": "middle",
        "dominant-baseline": "middle",
      });
      label.textContent = node.title;
      group.append(label);
    }
    nodeLayer.append(group);
  }

  svg.append(linkLayer, nodeLayer);
  renderNavigation(currentView, crumbs, children);
}

function renderNavigation(currentView, crumbs, children) {
  viewInfo.textContent = `${currentView.label}\n${currentView.id}\n${currentView.density}`;
  breadcrumb.replaceChildren();
  crumbs.forEach((view, index) => {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = view.label;
    button.disabled = index === crumbs.length - 1;
    button.addEventListener("click", () => {
      if (index === 0) {
        explorer.home();
      } else if (index === crumbs.length - 2) {
        explorer.parent();
      }
      renderGraph();
    });
    breadcrumb.append(button);
  });

  childViews.replaceChildren();
  for (const view of children) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = view.label;
    button.addEventListener("click", () => {
      explorer.enter_view(view.id);
      renderGraph();
    });
    childViews.append(button);
  }
}

function renderInspector(event) {
  const point = worldPoint(event);
  const node = parseJson(explorer.node_at_json(point.x, point.y));
  if (!node) {
    setHighlightedNode(null);
    inspector.textContent = "Move the pointer over a physical node.";
    return;
  }
  setHighlightedNode(node.id);
  inspector.textContent = `${node.title}\n${node.kind}\n${node.id}\n${node.subsystem}\n→ ${node.targetView ?? "subsystem"}`;
}

function renderPlayback() {
  const summary = parseJson(playback.summary_json());
  if (!summary) {
    playbackState.textContent = "No trace loaded.";
    progressFill.style.width = "0%";
    timelineScrubber.disabled = true;
    timelineScrubber.max = "0";
    timelineScrubber.value = "0";
    timelineLabel.value = "—";
    return;
  }

  const micro = parseJson(playback.current_microcycle_json());
  const bus = parseJson(playback.current_bus_json());
  const frame = parseJson(playback.current_frame_json());
  const lastCursor = Math.max(0, summary.microcycles - 1);
  timelineScrubber.disabled = false;
  timelineScrubber.max = String(lastCursor);
  timelineScrubber.value = String(summary.cursor);
  timelineLabel.value = `${summary.cursor} / ${lastCursor}`;

  const lines = [
    `seed      ${summary.seed}`,
    `cursor    ${summary.cursor}/${lastCursor}`,
    `frame     ${micro?.frame ?? "—"}`,
    `phase     ${micro?.phase ?? "—"}`,
    `µkind     ${micro?.kind ?? "—"}`,
    `PC        ${micro ? hex(micro.pc, 4) : "—"}`,
    `MAR       ${micro ? hex(micro.mar, 4) : "—"}`,
    `MDR / IR  ${micro ? `${hex(micro.mdr, 2)} / ${hex(micro.ir, 2)}` : "—"}`,
    `bus kind  ${bus?.kind ?? "—"}`,
    `bus addr  ${bus ? hex(bus.address, 4) : "—"}`,
    `bus data  ${bus ? hex(bus.data, 2) : "—"}`,
    `addr src  ${bus?.addressSource ?? "—"}`,
    `data src  ${bus?.dataSource ?? "—"}`,
    `score     ${frame?.score ?? "—"}`,
    `lives     ${frame?.lives ?? "—"}`,
  ];
  playbackState.textContent = lines.join("\n");
  progressFill.style.width = `${Math.max(0, Math.min(100, playback.progress() * 100))}%`;
}

function followPayload(payload) {
  const target = parseJson(payload);
  if (!target?.primaryNode) return;
  explorer.focus_node(target.primaryNode);
  highlightedNode = target.primaryNode;
  renderGraph();
}

function playbackLoop() {
  rafHandle = 0;
  if (!playback.is_playing()) return;
  playback.tick(48);
  renderPlayback();
  if (playback.is_playing()) {
    rafHandle = requestAnimationFrame(playbackLoop);
  }
}

function ensurePlaybackLoop() {
  if (!rafHandle && playback.is_playing()) {
    rafHandle = requestAnimationFrame(playbackLoop);
  }
}

svg.addEventListener("pointerdown", (event) => {
  dragging = true;
  dragDistance = 0;
  lastPointer = { x: event.clientX, y: event.clientY };
  svg.setPointerCapture(event.pointerId);
});

svg.addEventListener("pointermove", (event) => {
  if (dragging && lastPointer) {
    const camera = parseJson(explorer.camera_json());
    const rect = svg.getBoundingClientRect();
    const dx = event.clientX - lastPointer.x;
    const dy = event.clientY - lastPointer.y;
    dragDistance += Math.abs(dx) + Math.abs(dy);
    explorer.pan_camera(-(dx / rect.width) * camera.w, -(dy / rect.height) * camera.h);
    lastPointer = { x: event.clientX, y: event.clientY };
    renderGraph();
    return;
  }
  renderInspector(event);
});

svg.addEventListener("pointerleave", () => {
  if (!dragging) {
    setHighlightedNode(null);
    inspector.textContent = "Move the pointer over a physical node.";
  }
});

svg.addEventListener("pointerup", (event) => {
  if (!dragging) return;
  dragging = false;
  lastPointer = null;
  if (dragDistance < 5) {
    const point = worldPoint(event);
    explorer.focus_at(point.x, point.y);
    renderGraph();
  }
});

svg.addEventListener("wheel", (event) => {
  event.preventDefault();
  const point = worldPoint(event);
  const factor = Math.exp(-event.deltaY * 0.0015);
  explorer.zoom_camera_at(point.x, point.y, factor);
  renderGraph();
}, { passive: false });

document.querySelector("#home-button").addEventListener("click", () => {
  explorer.home();
  renderGraph();
});
document.querySelector("#back-button").addEventListener("click", () => {
  explorer.back();
  renderGraph();
});
document.querySelector("#fit-button").addEventListener("click", () => {
  explorer.fit_current_view();
  renderGraph();
});

document.querySelector("#load-button").addEventListener("click", () => {
  const seed = document.querySelector("#seed-input").value.trim();
  const frames = Number(document.querySelector("#frames-input").value);
  playback.load_match(seed, Math.max(1, Math.trunc(frames)));
  renderPlayback();
});
document.querySelector("#play-button").addEventListener("click", () => {
  playback.play();
  ensurePlaybackLoop();
  renderPlayback();
});
document.querySelector("#pause-button").addEventListener("click", () => {
  playback.pause();
  renderPlayback();
});
document.querySelector("#micro-step-button").addEventListener("click", () => {
  playback.step_microcycle();
  renderPlayback();
});
document.querySelector("#instruction-step-button").addEventListener("click", () => {
  playback.step_instruction();
  renderPlayback();
});
timelineScrubber.addEventListener("input", () => {
  playback.seek_cursor(Number(timelineScrubber.value));
  renderPlayback();
});
document.querySelector("#follow-pc-button").addEventListener("click", () => {
  followPayload(playback.follow_pc_json());
});
document.querySelector("#next-bus-button").addEventListener("click", () => {
  if (playback.seek_next_bus()) followPayload(playback.follow_bus_json());
  renderPlayback();
});
document.querySelector("#next-dma-button").addEventListener("click", () => {
  if (playback.seek_next_dma()) followPayload(playback.follow_dma_json());
  renderPlayback();
});
document.querySelector("#next-vblank-button").addEventListener("click", () => {
  if (playback.seek_next_vblank()) followPayload(playback.follow_vblank_json());
  renderPlayback();
});

renderGraph();
renderPlayback();
