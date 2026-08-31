import init, { ActivityResolver, Explorer, Playback } from "./pkg/leader_explorer.js";

await init();

const explorer = new Explorer();
const playback = new Playback();
const activityResolver = new ActivityResolver();
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

const activePointers = new Map();
let tapCandidate = null;
let tapTravel = 0;
let pinchState = null;
let rafHandle = 0;
let highlightedNode = null;

function parseJson(value) {
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
}

function hex(value, width) {
  if (value === null || value === undefined) return "—";
  return `0x${Number(value).toString(16).padStart(width, "0")}`;
}

function routePath(route) {
  if (!Array.isArray(route) || route.length < 2) return null;
  return route
    .map((point, index) => `${index === 0 ? "M" : "L"}${point[0]} ${point[1]}`)
    .join(" ");
}

function clientToWorld(clientX, clientY) {
  const camera = parseJson(explorer.camera_json());
  const rect = svg.getBoundingClientRect();
  return {
    x: camera.x + ((clientX - rect.left) / rect.width) * camera.w,
    y: camera.y + ((clientY - rect.top) / rect.height) * camera.h,
  };
}

function worldPoint(event) {
  return clientToWorld(event.clientX, event.clientY);
}

function makeSvg(tag, attributes = {}) {
  const element = document.createElementNS(NS, tag);
  for (const [name, value] of Object.entries(attributes)) {
    element.setAttribute(name, String(value));
  }
  return element;
}

function pointerPair() {
  return [...activePointers.values()].slice(0, 2);
}

function midpoint(left, right) {
  return { x: (left.x + right.x) * 0.5, y: (left.y + right.y) * 0.5 };
}

function pointerDistance(left, right) {
  return Math.hypot(right.x - left.x, right.y - left.y);
}

function beginPinch() {
  if (activePointers.size < 2) {
    pinchState = null;
    return;
  }
  const [left, right] = pointerPair();
  pinchState = {
    distance: Math.max(pointerDistance(left, right), 1),
    midpoint: midpoint(left, right),
  };
  tapCandidate = null;
}

function updatePinch() {
  if (activePointers.size < 2 || !pinchState) return false;
  const [left, right] = pointerPair();
  const nextMidpoint = midpoint(left, right);
  const nextDistance = Math.max(pointerDistance(left, right), 1);
  const camera = parseJson(explorer.camera_json());
  const rect = svg.getBoundingClientRect();
  const dx = nextMidpoint.x - pinchState.midpoint.x;
  const dy = nextMidpoint.y - pinchState.midpoint.y;
  explorer.pan_camera(-(dx / rect.width) * camera.w, -(dy / rect.height) * camera.h);
  const anchor = clientToWorld(nextMidpoint.x, nextMidpoint.y);
  const factor = Math.max(0.5, Math.min(2, nextDistance / pinchState.distance));
  explorer.zoom_camera_at(anchor.x, anchor.y, factor);
  pinchState = { distance: nextDistance, midpoint: nextMidpoint };
  renderGraph();
  return true;
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

function signalValueText(link, cameraScale) {
  if (link.value === null || link.value === undefined || !link.width) return null;
  const value = Number(link.value);
  if (cameraScale < 0.9) return `0b${value.toString(2).padStart(link.width, "0")}`;
  return hex(value, Math.ceil(link.width / 4));
}

function activityLinks(activity) {
  if (!activity?.phase) return [];
  return parseJson(activityResolver.links_json(activity.phase, activity.address ?? -1, activity.data ?? -1)) ?? [];
}

function currentBitChanges() {
  return parseJson(playback.current_bit_changes_json()) ?? [];
}

function currentAluValues() {
  return parseJson(playback.current_alu_values_json()) ?? [];
}

function applyActivity(activity) {
  svg.querySelectorAll(".is-active").forEach((element) => element.classList.remove("is-active"));
  svg.querySelectorAll(".activity-value").forEach((element) => element.remove());
  if (!activity?.nodes) {
    delete svg.dataset.activityPhase;
    return;
  }
  svg.dataset.activityPhase = activity.phase;
  for (const nodeId of activity.nodes) {
    svg.querySelector(`[data-node-id="${CSS.escape(nodeId)}"]`)?.classList.add("is-active");
  }
  const camera = parseJson(explorer.camera_json());
  const cameraScale = camera ? camera.w / Math.max(svg.clientWidth, 1) : Number.POSITIVE_INFINITY;
  const linkLayer = svg.querySelector(".link-layer");
  for (const link of activityLinks(activity)) {
    const path = svg.querySelector(`[data-link-id="${CSS.escape(link.id)}"]`);
    if (!path) continue;
    path.classList.add("is-active");
    const valueText = signalValueText(link, cameraScale);
    if (!valueText || !linkLayer || cameraScale >= 2.8) continue;
    const length = path.getTotalLength();
    if (!Number.isFinite(length) || length <= 0) continue;
    const point = path.getPointAtLength(length * 0.5);
    const label = makeSvg("text", {
      x: point.x,
      y: point.y - Math.max(8, cameraScale * 3),
      class: `activity-value signal-value-${link.signal}`,
      "text-anchor": "middle",
    });
    label.textContent = valueText;
    linkLayer.append(label);
  }
}

function clearAluValues() {
  svg.querySelectorAll(".has-alu-value").forEach((element) => {
    element.classList.remove("has-alu-value", "alu-value-one", "alu-value-zero");
    delete element.dataset.aluBit;
    delete element.dataset.aluStage;
    delete element.dataset.aluValue;
  });
  svg.querySelectorAll(".alu-node-value").forEach((element) => element.remove());
}

function applyAluValues(values) {
  clearAluValues();
  if (!Array.isArray(values) || values.length === 0) return;
  const camera = parseJson(explorer.camera_json());
  const cameraScale = camera ? camera.w / Math.max(svg.clientWidth, 1) : Number.POSITIVE_INFINITY;
  for (const state of values) {
    const group = svg.querySelector(`[data-node-id="${CSS.escape(state.node)}"]`);
    if (!group) continue;
    group.classList.add("has-alu-value", state.value ? "alu-value-one" : "alu-value-zero");
    group.dataset.aluBit = String(state.bit);
    group.dataset.aluStage = state.stage;
    group.dataset.aluValue = state.value ? "1" : "0";
    if (cameraScale >= 1.35) continue;
    const rect = group.querySelector("rect");
    if (!rect) continue;
    const label = makeSvg("text", {
      x: Number(rect.getAttribute("x")) + Number(rect.getAttribute("width")) - 8,
      y: Number(rect.getAttribute("y")) + 14,
      class: "alu-node-value",
      "text-anchor": "end",
    });
    label.textContent = state.value ? "1" : "0";
    group.append(label);
  }
}

function clearBitChanges() {
  svg.querySelectorAll(".is-bit-change").forEach((element) => {
    element.classList.remove("is-bit-change", "bit-to-one", "bit-to-zero");
    delete element.dataset.bitTransition;
    delete element.dataset.bitSource;
  });
  svg.querySelectorAll(".bit-transition-value").forEach((element) => element.remove());
}

function applyBitChanges(changes) {
  clearBitChanges();
  if (!Array.isArray(changes) || changes.length === 0) return;
  const camera = parseJson(explorer.camera_json());
  const cameraScale = camera ? camera.w / Math.max(svg.clientWidth, 1) : Number.POSITIVE_INFINITY;
  for (const change of changes) {
    const group = svg.querySelector(`[data-node-id="${CSS.escape(change.node)}"]`);
    if (!group) continue;
    const before = change.before ? 1 : 0;
    const after = change.after ? 1 : 0;
    group.classList.add("is-bit-change", change.after ? "bit-to-one" : "bit-to-zero");
    group.dataset.bitTransition = `${before}→${after}`;
    group.dataset.bitSource = change.source ?? "native";
    if (cameraScale >= 1.8) continue;
    const rect = group.querySelector("rect");
    if (!rect) continue;
    const label = makeSvg("text", {
      x: Number(rect.getAttribute("x")) + Number(rect.getAttribute("width")) * 0.5,
      y: Number(rect.getAttribute("y")) + Number(rect.getAttribute("height")) + Math.max(11, cameraScale * 5),
      class: "bit-transition-value",
      "text-anchor": "middle",
    });
    label.textContent = `${before}→${after}`;
    group.append(label);
  }
}

function applyLiveState(activity, aluValues, changes) {
  applyActivity(activity);
  applyAluValues(aluValues);
  applyBitChanges(changes);
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
  const linkLayer = makeSvg("g", { class: "link-layer" });
  const nodeLayer = makeSvg("g", { class: "node-layer" });
  for (const link of graph.links) {
    const path = routePath(link.route);
    if (!path) continue;
    linkLayer.append(makeSvg("path", {
      d: path,
      class: `hardware-link signal-${link.signal}`,
      "data-link-id": link.id,
      "data-from-node": link.from,
      "data-to-node": link.to,
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
  applyLiveState(parseJson(playback.current_activity_json()), currentAluValues(), currentBitChanges());
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
      if (index === 0) explorer.home();
      else if (index === crumbs.length - 2) explorer.parent();
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
  const liveNode = svg.querySelector(`[data-node-id="${CSS.escape(node.id)}"]`);
  const transition = liveNode?.dataset.bitTransition;
  const source = liveNode?.dataset.bitSource;
  const aluStage = liveNode?.dataset.aluStage;
  const aluValue = liveNode?.dataset.aluValue;
  const mutation = transition ? `\nflip ${transition}\n${source}` : "";
  const alu = aluStage ? `\nALU bit ${liveNode.dataset.aluBit} · ${aluStage} = ${aluValue}` : "";
  inspector.textContent = `${node.title}\n${node.kind}\n${node.id}\n${node.subsystem}\n→ ${node.targetView ?? "subsystem"}${alu}${mutation}`;
}

function renderPlayback() {
  const summary = parseJson(playback.summary_json());
  const activity = parseJson(playback.current_activity_json());
  const aluValues = currentAluValues();
  const bitChanges = currentBitChanges();
  applyLiveState(activity, aluValues, bitChanges);
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
  const links = activityLinks(activity);
  const mutations = bitChanges.slice(0, 6)
    .map((change) => `${change.node}:${change.before ? 1 : 0}→${change.after ? 1 : 0}`)
    .join(" ");
  const aluOnes = aluValues.filter((state) => state.value).length;
  const lines = [
    `seed      ${summary.seed}`,
    `cursor    ${summary.cursor}/${lastCursor}`,
    `frame     ${micro?.frame ?? "—"}`,
    `phase     ${activity?.phase ?? "—"}`,
    `µphase    ${micro?.phase ?? "—"}`,
    `µkind     ${micro?.kind ?? "—"}`,
    `PC        ${micro ? hex(micro.pc, 4) : "—"}`,
    `MAR       ${micro ? hex(micro.mar, 4) : "—"}`,
    `MDR / IR  ${micro ? `${hex(micro.mdr, 2)} / ${hex(micro.ir, 2)}` : "—"}`,
    `bus kind  ${bus?.kind ?? "—"}`,
    `bus addr  ${bus ? hex(bus.address, 4) : "—"}`,
    `bus data  ${bus ? hex(bus.data, 2) : "—"}`,
    `addr src  ${bus?.addressSource ?? "—"}`,
    `data src  ${bus?.dataSource ?? "—"}`,
    `active    ${activity?.nodes?.length ?? 0} nodes / ${links.length} links`,
    `ALU gates ${aluValues.length ? `${aluOnes}/${aluValues.length} high` : "—"}`,
    `bit flips ${bitChanges.length}`,
    `mutations ${mutations || "—"}`,
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
  if (playback.is_playing()) rafHandle = requestAnimationFrame(playbackLoop);
}

function ensurePlaybackLoop() {
  if (!rafHandle && playback.is_playing()) rafHandle = requestAnimationFrame(playbackLoop);
}

svg.addEventListener("pointerdown", (event) => {
  activePointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  svg.setPointerCapture(event.pointerId);
  if (activePointers.size === 1) {
    tapCandidate = event.pointerId;
    tapTravel = 0;
    pinchState = null;
  } else beginPinch();
});

svg.addEventListener("pointermove", (event) => {
  const previous = activePointers.get(event.pointerId);
  if (!previous) {
    if (activePointers.size === 0) renderInspector(event);
    return;
  }
  activePointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  if (activePointers.size >= 2) {
    updatePinch();
    return;
  }
  const camera = parseJson(explorer.camera_json());
  const rect = svg.getBoundingClientRect();
  const dx = event.clientX - previous.x;
  const dy = event.clientY - previous.y;
  tapTravel += Math.abs(dx) + Math.abs(dy);
  if (tapTravel >= 5) tapCandidate = null;
  explorer.pan_camera(-(dx / rect.width) * camera.w, -(dy / rect.height) * camera.h);
  renderGraph();
});

svg.addEventListener("pointerleave", () => {
  if (activePointers.size === 0) {
    setHighlightedNode(null);
    inspector.textContent = "Move the pointer over a physical node.";
  }
});

function finishPointer(event, allowTap) {
  const isTap = allowTap && tapCandidate === event.pointerId && activePointers.size === 1;
  activePointers.delete(event.pointerId);
  if (isTap) {
    const point = worldPoint(event);
    explorer.focus_at(point.x, point.y);
    renderGraph();
  }
  if (activePointers.size >= 2) beginPinch();
  else {
    pinchState = null;
    tapCandidate = null;
    tapTravel = 0;
  }
}

svg.addEventListener("pointerup", (event) => finishPointer(event, true));
svg.addEventListener("pointercancel", (event) => finishPointer(event, false));
svg.addEventListener("wheel", (event) => {
  event.preventDefault();
  const point = worldPoint(event);
  explorer.zoom_camera_at(point.x, point.y, Math.exp(-event.deltaY * 0.0015));
  renderGraph();
}, { passive: false });

document.querySelector("#home-button").addEventListener("click", () => { explorer.home(); renderGraph(); });
document.querySelector("#back-button").addEventListener("click", () => { explorer.back(); renderGraph(); });
document.querySelector("#fit-button").addEventListener("click", () => { explorer.fit_current_view(); renderGraph(); });
document.querySelector("#load-button").addEventListener("click", () => {
  const seed = document.querySelector("#seed-input").value.trim();
  const frames = Number(document.querySelector("#frames-input").value);
  playback.load_match(seed, Math.max(1, Math.trunc(frames)));
  renderPlayback();
});
document.querySelector("#play-button").addEventListener("click", () => { playback.play(); ensurePlaybackLoop(); renderPlayback(); });
document.querySelector("#pause-button").addEventListener("click", () => { playback.pause(); renderPlayback(); });
document.querySelector("#micro-step-button").addEventListener("click", () => { playback.step_microcycle(); renderPlayback(); });
document.querySelector("#instruction-step-button").addEventListener("click", () => { playback.step_instruction(); renderPlayback(); });
timelineScrubber.addEventListener("input", () => { playback.seek_cursor(Number(timelineScrubber.value)); renderPlayback(); });
document.querySelector("#follow-pc-button").addEventListener("click", () => followPayload(playback.follow_pc_json()));
document.querySelector("#next-bus-button").addEventListener("click", () => { if (playback.seek_next_bus()) followPayload(playback.follow_bus_json()); renderPlayback(); });
document.querySelector("#next-dma-button").addEventListener("click", () => { if (playback.seek_next_dma()) followPayload(playback.follow_dma_json()); renderPlayback(); });
document.querySelector("#next-vblank-button").addEventListener("click", () => { if (playback.seek_next_vblank()) followPayload(playback.follow_vblank_json()); renderPlayback(); });

renderGraph();
renderPlayback();
