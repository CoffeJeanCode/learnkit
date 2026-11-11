// three.js scene of the `topology_flow` engine: an isometric board of icons
// joined by links, and packets that travel the links one message at a time.
// React (`TopologyFlowVisual.tsx`) owns WHICH step is showing; this file owns
// how it is drawn and animated.

import * as THREE from "three";
import { drawableRoute, layoutTopology, placeLabels, type LabelSide, type TopoLinkLike, type TopoStepLike } from "../../../../lib/scene3d/topologyLayout";
import { toneColor } from "../../../../lib/scene3d/palette";
import { buildIcon, type IconHandle } from "./iconMeshes";
import { GOLD, INK_DIM, PALETTE, createStage, makeLabel, pickedId } from "./stage";

export interface TopologyData {
  nodes: Array<{ id: string; label: string; icon: string; subtext?: string | null; explanation?: string | null }>;
  links: TopoLinkLike[];
  steps: TopoStepLike[];
}

export interface TopologyCallbacks {
  onPickNode(id: string): void;
  /** Every message of the step has arrived. */
  onStepFinished(index: number): void;
}

export interface TopologyController {
  /** Show step `index`; `animate` false draws its final state at once. */
  playStep(index: number, animate: boolean): void;
  setPaused(paused: boolean): void;
  setSpeed(multiplier: number): void;
  /** Spotlight one node (a guided-walkthrough step); null clears it. */
  setHighlight(id: string | null): void;
  dispose(): void;
}

const PACKET_SPEED = 3.4; // scene units per second at 1×
const PACKET_Y = 0.55;
const GAP_BETWEEN_MESSAGES = 0.25; // seconds

/** Per label side: CSS anchor (x, y) and world offset from the icon's base.
 *  Offsets are the world-space equivalents of the screen offsets `placeLabels`
 *  assumed (ground moves diagonally on screen; height moves straight up). */
const LABEL_ANCHOR: Record<LabelSide, (iconHeight: number) => [number, number, number, number, number]> = {
  below: () => [0.5, 0, 0.64, 0, 0.64],
  above: (h) => [0.5, 1, 0, h + 0.12, 0],
  right: () => [0, 0.5, 0.71, 0.35, -0.71],
  left: () => [1, 0.5, -0.71, 0.35, 0.71],
};

const linkKey = (a: string, b: string) => (a < b ? `${a}|${b}` : `${b}|${a}`);

export function mountTopology(host: HTMLElement, data: TopologyData, cb: TopologyCallbacks): TopologyController | null {
  const stage = createStage(host);
  if (!stage) return null;
  const { scene } = stage;

  // Layout columns run along -Z and rows along +X, so with the camera on the
  // (+X, +Y, +Z) diagonal the story flows from lower-left to upper-right.
  const layout = layoutTopology(data.nodes, data.links);
  const ground = new Map<string, THREE.Vector3>();
  for (const n of data.nodes) {
    const p = layout.positions[n.id];
    ground.set(n.id, new THREE.Vector3(p.z, 0, -p.x));
  }

  // --- camera: fixed isometric, framed on the occupied area -----------------
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 200);
  // Looking at the world origin, so screen units map 1:1 to `toScreen()`.
  camera.position.copy(new THREE.Vector3(1, 0.82, 1).normalize().multiplyScalar(60));
  camera.lookAt(0, 0, 0);
  const { sides, bounds } = placeLabels(data.nodes, layout);
  const centerR = (bounds.minR + bounds.maxR) / 2;
  const centerU = (bounds.minU + bounds.maxU) / 2;
  const halfW = (bounds.maxR - bounds.minR) / 2 + 0.35;
  const halfH = (bounds.maxU - bounds.minU) / 2 + 0.35;
  stage.onResize((w, h) => {
    const aspect = w / h;
    const fit = Math.max(halfH, halfW / aspect);
    camera.left = centerR - fit * aspect;
    camera.right = centerR + fit * aspect;
    camera.top = centerU + fit;
    camera.bottom = centerU - fit;
    camera.updateProjectionMatrix();
  });

  // --- board ------------------------------------------------------------------
  const grid = new THREE.GridHelper(80, 80, "#2b3545", "#222a36");
  const gridMaterial = grid.material as THREE.Material;
  gridMaterial.transparent = true;
  gridMaterial.opacity = 0.55;
  scene.add(grid);

  const linkMaterials = new Map<string, THREE.MeshBasicMaterial>();
  for (const link of data.links) {
    const a = ground.get(link.from);
    const b = ground.get(link.to);
    const key = linkKey(link.from, link.to);
    if (!a || !b || link.from === link.to || linkMaterials.has(key)) continue;
    const material = new THREE.MeshBasicMaterial({ color: INK_DIM });
    const mesh = new THREE.Mesh(new THREE.BoxGeometry(a.distanceTo(b), 0.03, 0.07), material);
    mesh.position.copy(a).add(b).multiplyScalar(0.5).setY(0.02);
    mesh.rotation.y = -Math.atan2(b.z - a.z, b.x - a.x);
    scene.add(mesh);
    linkMaterials.set(key, material);
  }

  const icons = new Map<string, IconHandle>();
  const labelEls = new Map<string, HTMLElement>();
  const nodeGroups: THREE.Object3D[] = [];
  for (const n of data.nodes) {
    const icon = buildIcon(n.icon);
    const at = ground.get(n.id)!;
    icon.group.position.copy(at);
    icon.group.userData.nodeId = n.id;
    scene.add(icon.group);
    icons.set(n.id, icon);
    nodeGroups.push(icon.group);

    // A real <button>: reachable by keyboard and screen readers, unlike the mesh.
    const el = document.createElement("button");
    el.type = "button";
    el.className = "scene3d-node-label";
    const name = document.createElement("strong");
    name.textContent = n.label;
    el.appendChild(name);
    if (n.subtext) {
      const sub = document.createElement("span");
      sub.textContent = n.subtext;
      el.appendChild(sub);
    }
    if (n.explanation?.trim()) {
      el.classList.add("is-tappable");
      el.setAttribute("aria-label", `${n.label}: ver explicación`);
      el.addEventListener("click", () => cb.onPickNode(n.id));
    } else {
      el.tabIndex = -1;
    }
    // Each label sits on the side where it covers least (see `placeLabels`).
    const side = sides[n.id] ?? "below";
    const label = makeLabel(el);
    const [centerX, centerY, dx, dy, dz] = LABEL_ANCHOR[side](icon.height);
    label.center.set(centerX, centerY);
    label.position.copy(at).add(new THREE.Vector3(dx, dy, dz));
    el.classList.add(`is-${side}`);
    scene.add(label);
    labelEls.set(n.id, el);
  }
  stage.onPick(
    () => nodeGroups,
    (hit) => {
      const id = pickedId(hit, "nodeId");
      if (id) cb.onPickNode(id);
    },
  );

  // --- packet -----------------------------------------------------------------
  const packetMaterial = new THREE.MeshBasicMaterial({ color: PALETTE.sky });
  const packetGeometry = new THREE.BoxGeometry(0.3, 0.3, 0.3);
  const packet = new THREE.Group();
  packet.add(new THREE.Mesh(packetGeometry, packetMaterial));
  packet.add(new THREE.LineSegments(new THREE.EdgesGeometry(packetGeometry), new THREE.LineBasicMaterial({ color: "#0b1220" })));
  const packetLabelEl = document.createElement("div");
  packetLabelEl.className = "scene3d-packet-label";
  const packetLabel = makeLabel(packetLabelEl, 1);
  packetLabel.position.set(0, 0.42, 0);
  packet.add(packetLabel);
  packet.visible = false;
  scene.add(packet);

  // --- state ------------------------------------------------------------------
  let highlight: string | null = null;
  let activeEnds: string[] = [];
  let activeTone = PALETTE.sky;
  let paused = false;
  let speed = 1;
  let step = -1;
  let message = 0;
  let travelled = 0;
  let gap = 0;
  let running = false;
  let path: THREE.Vector3[] = [];
  let pathIds: string[] = [];

  const refreshNodes = () => {
    for (const [id, icon] of icons) {
      const el = labelEls.get(id)!;
      const spot = highlight === id;
      const dim = highlight !== null && !spot;
      const active = !dim && activeEnds.includes(id);
      icon.setTone(spot ? GOLD : active ? activeTone : null, dim ? 0.45 : 1);
      el.classList.toggle("is-dim", dim);
      el.classList.toggle("is-active", spot || active);
    }
  };

  const paintRoute = (ids: string[], color: string) => {
    for (let i = 0; i + 1 < ids.length; i++) linkMaterials.get(linkKey(ids[i], ids[i + 1]))?.color.set(color);
  };

  const startMessage = (index: number) => {
    const m = data.steps[step]?.messages[index];
    if (!m) return false;
    message = index;
    pathIds = drawableRoute(data.nodes, data.links, m.from, m.to).filter((id) => ground.has(id));
    path = pathIds.map((id) => ground.get(id)!.clone().setY(PACKET_Y));
    if (path.length < 2) return startMessage(index + 1);
    travelled = 0;
    activeEnds = [m.from, m.to];
    activeTone = toneColor(m.tone);
    packetMaterial.color.set(activeTone);
    packetLabelEl.textContent = m.label;
    packet.position.copy(path[0]);
    packet.visible = true;
    refreshNodes();
    return true;
  };

  const finishStep = () => {
    running = false;
    packet.visible = false;
    const finished = step;
    // Out of the frame callback: the listener sets React state.
    setTimeout(() => cb.onStepFinished(finished), 0);
  };

  stage.onFrame((dt) => {
    if (!running || paused) return;
    if (gap > 0) {
      gap -= dt;
      if (gap <= 0 && !startMessage(message + 1)) finishStep();
      return;
    }
    travelled += dt * PACKET_SPEED * speed;
    let remaining = travelled;
    for (let i = 0; i + 1 < path.length; i++) {
      const length = path[i].distanceTo(path[i + 1]);
      if (remaining <= length) {
        packet.position.lerpVectors(path[i], path[i + 1], remaining / length);
        packet.position.y = PACKET_Y + Math.sin((remaining / length) * Math.PI) * 0.18; // a small hop per link
        return;
      }
      remaining -= length;
    }
    // Arrived: the route it took stays lit in its tone.
    packet.position.copy(path[path.length - 1]);
    paintRoute(pathIds, activeTone);
    gap = GAP_BETWEEN_MESSAGES;
  });

  stage.setCamera(camera);

  return {
    playStep(index, animate) {
      step = index;
      linkMaterials.forEach((m) => m.color.set(INK_DIM));
      packet.visible = false;
      gap = 0;
      const messages = data.steps[index]?.messages ?? [];
      if (animate && !stage.reducedMotion && startMessage(0)) {
        running = true;
        return;
      }
      // Static: every route of the step lit, the last message's ends marked.
      running = false;
      for (const m of messages) paintRoute(drawableRoute(data.nodes, data.links, m.from, m.to), toneColor(m.tone));
      const last = messages[messages.length - 1];
      activeEnds = last ? [last.from, last.to] : [];
      activeTone = toneColor(last?.tone);
      refreshNodes();
      if (animate) finishStep();
    },
    setPaused(value) {
      paused = value;
    },
    setSpeed(multiplier) {
      speed = multiplier;
    },
    setHighlight(id) {
      highlight = id && icons.has(id) ? id : null;
      refreshNodes();
    },
    dispose() {
      running = false;
      stage.dispose();
    },
  };
}
