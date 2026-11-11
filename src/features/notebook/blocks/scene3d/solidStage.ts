// three.js scene of the `solid_model` engine: a CAD part the student orbits,
// rebuilds operation by operation, looks at from the standard views and —
// with the camera inset on — watches from outside: the viewing camera moves
// around the part while the part and its axes stay put.
// React (`SolidModelVisual.tsx`) owns the state; this file owns the drawing.

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { partColor } from "../../../../lib/scene3d/palette";
import { cadToThree, featureBox, isAdditive, partBounds, planeAxes, profileOutline, viewDirection } from "../../../../lib/scene3d/solidBuild";
import type { SolidFeature } from "../../../../lib/schemas";
import { buildSolid, creaseEdges, featureOfHit, planePoint, surfaceAlong } from "./solidGeometry";
import { GOLD, INK, createStage, makeLabel } from "./stage";

export interface SolidData {
  features: SolidFeature[];
  startView: string;
}

export interface SolidCallbacks {
  /** A feature was tapped (null = empty space). */
  onPick(id: string | null): void;
}

export interface SolidController {
  /** Show the part as it is after the first `n` operations. */
  setStep(n: number): void;
  select(id: string | null): void;
  setView(view: string): void;
  setOrthographic(ortho: boolean): void;
  setCameraInset(on: boolean): void;
  setLabels(on: boolean): void;
  dispose(): void;
}

/** Fraction of the stage the camera inset covers (CSS mirrors it: `.solid-inset`). */
export const INSET_FRACTION = 0.34;
export const INSET_MARGIN = 8;

const AXIS_COLORS = { X: "#f87171", Y: "#4ade80", Z: "#38bdf8" };

export function mountSolid(host: HTMLElement, data: SolidData, cb: SolidCallbacks): SolidController | null {
  const stage = createStage(host);
  if (!stage) return null;
  const { scene, renderer, size: viewport } = stage;
  const { features } = data;

  scene.add(new THREE.HemisphereLight("#e6eeff", "#1a1f29", 1.1));
  const sun = new THREE.DirectionalLight("#ffffff", 1.7);
  sun.position.set(5, 8, 6);
  scene.add(sun);
  const fill = new THREE.DirectionalLight("#9fb6ff", 0.5);
  fill.position.set(-6, 2, -4);
  scene.add(fill);

  // Everything in CAD coordinates lives in `cad` (Z up -> three's Y up).
  const cad = new THREE.Group();
  cad.rotation.x = -Math.PI / 2;
  scene.add(cad);

  const bounds = partBounds(features) ?? { min: [-10, -10, 0], max: [10, 10, 10] };
  const centreCad = [0, 1, 2].map((k) => (bounds.min[k] + bounds.max[k]) / 2) as [number, number, number];
  const span = Math.max(...[0, 1, 2].map((k) => bounds.max[k] - bounds.min[k]), 1);
  const target = new THREE.Vector3(...cadToThree(centreCad));
  const distance = span * 2.7;

  // --- floor grid and axes, both fixed: what stays still while the camera moves ---
  const floorZ = Math.min(0, bounds.min[2]);
  const gridSize = Math.ceil((span * 2.4) / 10) * 10;
  const grid = new THREE.GridHelper(gridSize, 12, "#3d4c63", "#232b38");
  grid.position.set(target.x, floorZ, target.z);
  scene.add(grid);
  const axisLength = span * 0.55;
  for (const [name, dir] of [
    ["X", [1, 0, 0]],
    ["Y", [0, 1, 0]],
    ["Z", [0, 0, 1]],
  ] as const) {
    const color = AXIS_COLORS[name];
    const end = new THREE.Vector3(...dir).multiplyScalar(axisLength);
    const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), end]), new THREE.LineBasicMaterial({ color, depthTest: false }));
    line.renderOrder = 5;
    cad.add(line);
    const tip = new THREE.Mesh(new THREE.ConeGeometry(span * 0.018, span * 0.06, 12), new THREE.MeshBasicMaterial({ color, depthTest: false }));
    tip.renderOrder = 5;
    tip.position.copy(end);
    // Cones point along +Y; turn each towards its axis.
    tip.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), end.clone().normalize());
    cad.add(tip);
    const el = document.createElement("span");
    el.className = "solid-axis-label";
    el.style.color = color;
    el.textContent = name;
    const label = makeLabel(el, 0.5);
    label.position.copy(end.clone().multiplyScalar(1.12));
    cad.add(label);
  }
  const origin = new THREE.Mesh(new THREE.SphereGeometry(span * 0.015, 12, 8), new THREE.MeshBasicMaterial({ color: "#c084fc", depthTest: false }));
  origin.renderOrder = 6;
  cad.add(origin);

  // --- the part --------------------------------------------------------------------
  const materials = new Map<string, THREE.MeshStandardMaterial>();
  const baseColors = new Map<string, THREE.Color>();
  features.forEach((f, i) => {
    const color = new THREE.Color(isAdditive(f.op) ? partColor(null, i) : partColor("rose", i));
    // Material made by the part reads as one object: additive features share
    // a calm metal tone tinted by their colour; removals keep their colour.
    if (isAdditive(f.op)) color.lerp(new THREE.Color("#b8c4d4"), 0.55);
    const material = new THREE.MeshStandardMaterial({ color, roughness: 0.5, metalness: 0.15, flatShading: true, side: THREE.DoubleSide, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 });
    materials.set(f.id, material);
    baseColors.set(f.id, color.clone());
  });

  let solid: THREE.Mesh | null = null;
  let edges: THREE.LineSegments | null = null;
  const edgeMaterial = new THREE.LineBasicMaterial({ color: "#0d1117" });
  const solids = new Map<number, THREE.Mesh | null>();
  const solidAt = (n: number) => {
    if (!solids.has(n)) solids.set(n, buildSolid(features, n, (f) => materials.get(f.id)!));
    return solids.get(n)!;
  };

  /** Where an operation's sketch sits along its normal: its own plane, or —
   *  for a through cut — the face of the material it enters. */
  const sketchLevel = (f: SolidFeature) => {
    if (isAdditive(f.op) || f.depth != null) return f.offset;
    const before = features.slice(0, features.indexOf(f));
    return surfaceAlong(before, f.plane, f.at[0], f.at[1]) ?? Math.max(bounds.max[planeAxes(f.plane)[2]], f.offset);
  };

  // Sketch of the selected (or newest) operation, drawn on its own plane.
  let sketch: THREE.Group | null = null;
  const drawSketch = (f: SolidFeature | undefined) => {
    if (sketch) {
      cad.remove(sketch);
      sketch.traverse((o) => (o as THREE.Line).geometry?.dispose?.());
    }
    sketch = null;
    if (!f) return;
    const outline = profileOutline(f, 48);
    if (outline.length < 3) return;
    sketch = new THREE.Group();
    const n = sketchLevel(f);
    const pts = [...outline, outline[0]].map(([u, v]) => planePoint(f.plane, u, v, n));
    const material = new THREE.LineBasicMaterial({ color: GOLD, depthTest: false, transparent: true, opacity: 0.95 });
    const loop = new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), material);
    loop.renderOrder = 7;
    sketch.add(loop);
    if (f.op === "revolve") {
      // The axis it spins around: the plane's v axis.
      const e = outline.map((p) => p[1]);
      const [v0, v1] = [Math.min(...e) - span * 0.15, Math.max(...e) + span * 0.15];
      const axis = new THREE.Line(
        new THREE.BufferGeometry().setFromPoints([planePoint(f.plane, 0, v0, f.offset), planePoint(f.plane, 0, v1, f.offset)]),
        new THREE.LineDashedMaterial({ color: GOLD, dashSize: span * 0.04, gapSize: span * 0.025, depthTest: false }),
      );
      axis.computeLineDistances();
      axis.renderOrder = 7;
      sketch.add(axis);
    }
    cad.add(sketch);
  };

  // One label per operation, above its own box.
  const labelEls = new Map<string, HTMLElement>();
  const labels = new Map<string, THREE.Object3D>();
  features.forEach((f) => {
    const box = featureBox(f, Math.max(span, 1));
    if (!box) return;
    const el = document.createElement("button");
    el.type = "button";
    el.className = "scene3d-part-label";
    el.textContent = f.label;
    el.addEventListener("click", () => cb.onPick(f.id));
    // Material labels hang below their point, removal labels sit above theirs:
    // in a flat standard view the two would otherwise land on each other.
    const label = makeLabel(el, isAdditive(f.op) ? 0 : 1);
    if (isAdditive(f.op)) {
      label.position.set((box.min[0] + box.max[0]) / 2, (box.min[1] + box.max[1]) / 2, box.max[2] + span * 0.04);
    } else {
      // A removal is labelled where it bites: the centre of its sketch, on the face it enters.
      label.position.copy(planePoint(f.plane, f.at[0], f.at[1], sketchLevel(f))).add(new THREE.Vector3(0, 0, span * 0.04));
    }
    cad.add(label);
    labelEls.set(f.id, el);
    labels.set(f.id, label);
  });

  // --- state -----------------------------------------------------------------------
  let step = features.length;
  let selected: string | null = null;
  let labelsOn = true;
  let inset = false;

  const refresh = () => {
    const mesh = solidAt(step);
    if (mesh !== solid) {
      if (solid) cad.remove(solid);
      if (edges) {
        cad.remove(edges);
        edges.geometry.dispose();
      }
      solid = mesh;
      edges = null;
      if (solid) {
        cad.add(solid);
        edges = new THREE.LineSegments(creaseEdges(solid.geometry, 25), edgeMaterial);
        cad.add(edges);
      }
    }
    features.forEach((f, i) => {
      const material = materials.get(f.id)!;
      const isSelected = f.id === selected;
      const dim = selected !== null && !isSelected;
      material.color.copy(baseColors.get(f.id)!).multiplyScalar(dim ? 0.55 : 1);
      material.emissive.set(isSelected ? GOLD : "#000000");
      material.emissiveIntensity = isSelected ? 0.28 : 0;
      const el = labelEls.get(f.id);
      if (el) {
        el.style.display = labelsOn && i < step ? "" : "none";
        el.classList.toggle("is-active", isSelected);
        el.classList.toggle("is-dim", dim);
      }
    });
    const focus = features.find((f) => f.id === selected) ?? (step < features.length ? features[step - 1] : undefined);
    drawSketch(focus && features.indexOf(focus) < step ? focus : undefined);
  };

  // --- cameras ---------------------------------------------------------------------
  const perspective = new THREE.PerspectiveCamera(35, 1, span * 0.01, span * 100);
  const ortho = new THREE.OrthographicCamera(-1, 1, 1, -1, span * 0.01, span * 100);
  let camera: THREE.PerspectiveCamera | THREE.OrthographicCamera = perspective;
  const fitOrtho = () => {
    const aspect = viewport.width / Math.max(1, viewport.height);
    const half = span * 0.95;
    ortho.left = -half * aspect;
    ortho.right = half * aspect;
    ortho.top = half;
    ortho.bottom = -half;
    ortho.updateProjectionMatrix();
  };
  stage.onResize((w, h) => {
    perspective.aspect = w / h;
    perspective.fov = w / h < 1 ? 35 / Math.max(0.55, w / h) : 35;
    perspective.updateProjectionMatrix();
    fitOrtho();
  });
  /** Unit vector (three coords) from the part towards where `view` is seen from. */
  const viewVector = (view: string) => {
    const { dir } = viewDirection(view);
    // Straight down is a pole for orbit controls: nudge the top view a hair
    // towards the front, so +Y still points up on screen.
    const d = view === "top" ? [dir[0], -0.002, dir[2]] : dir;
    return new THREE.Vector3(...cadToThree(d as [number, number, number])).normalize();
  };
  perspective.position.copy(target).addScaledVector(viewVector(data.startView), distance);
  ortho.position.copy(perspective.position);

  const controls = new OrbitControls(camera as THREE.Camera, renderer.domElement);
  controls.target.copy(target);
  controls.enablePan = false;
  controls.enableDamping = true;
  controls.minDistance = span * 1.2;
  controls.maxDistance = span * 8;
  controls.update();

  // Animated move to a standard view (instant with reduced motion).
  // Interpolated in spherical coordinates (azimuth + elevation, like a CAD view
  // cube), so every frame on the way is a sensible view — a straight lerp
  // between directions only fixes the azimuth at the very last frame.
  let move: { from: THREE.Spherical; to: THREE.Spherical; t: number } | null = null;
  const goTo = (view: string) => {
    const to = new THREE.Spherical().setFromVector3(viewVector(view).multiplyScalar(distance));
    const from = new THREE.Spherical().setFromVector3(camera.position.clone().sub(target));
    // Shortest way round.
    while (to.theta - from.theta > Math.PI) to.theta -= Math.PI * 2;
    while (from.theta - to.theta > Math.PI) to.theta += Math.PI * 2;
    if (stage.reducedMotion) {
      camera.position.copy(target).add(new THREE.Vector3().setFromSpherical(to));
      controls.update();
      return;
    }
    move = { from, to, t: 0 };
  };

  // --- the camera seen from outside (inset) -------------------------------------------
  const observer = new THREE.PerspectiveCamera(32, 4 / 3, span * 0.01, span * 200);
  observer.position.copy(target).add(new THREE.Vector3(-1.1, 1.25, 1.6).normalize().multiplyScalar(distance * 1.9));
  observer.lookAt(target);
  const rig = new THREE.Group();
  const body = new THREE.Mesh(new THREE.BoxGeometry(span * 0.16, span * 0.11, span * 0.13), new THREE.MeshStandardMaterial({ color: GOLD, roughness: 0.4 }));
  const lens = new THREE.Mesh(new THREE.ConeGeometry(span * 0.07, span * 0.12, 16, 1, true), new THREE.MeshStandardMaterial({ color: "#e8eef7", side: THREE.DoubleSide }));
  // The cone opens along -Z, where a camera looks.
  lens.rotation.x = Math.PI / 2;
  lens.position.z = -span * 0.1;
  rig.add(body, lens);
  const sight = new THREE.Line(new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), new THREE.Vector3()]), new THREE.LineDashedMaterial({ color: GOLD, dashSize: span * 0.06, gapSize: span * 0.04 }));
  const ring = new THREE.Line(
    new THREE.BufferGeometry().setFromPoints(Array.from({ length: 97 }, (_, i) => new THREE.Vector3(Math.cos((i / 96) * Math.PI * 2), 0, Math.sin((i / 96) * Math.PI * 2)))),
    new THREE.LineBasicMaterial({ color: INK, transparent: true, opacity: 0.6 }),
  );
  const outside = new THREE.Group();
  outside.add(rig, sight, ring);
  outside.visible = false;
  scene.add(outside);
  stage.onAfterRender(() => {
    if (!inset) return;
    rig.position.copy(camera.position);
    rig.quaternion.copy(camera.quaternion);
    sight.geometry.setFromPoints([camera.position, target]);
    sight.computeLineDistances();
    // The orbit: a circle at the camera's height, around the vertical through the target.
    const offset = camera.position.clone().sub(target);
    const radius = Math.hypot(offset.x, offset.z);
    ring.position.set(target.x, camera.position.y, target.z);
    ring.scale.setScalar(Math.max(radius, 1e-3));
    const w = Math.round(viewport.width * INSET_FRACTION);
    const h = Math.round((w * 3) / 4);
    const x = viewport.width - w - INSET_MARGIN;
    const y = INSET_MARGIN;
    observer.aspect = w / h;
    observer.updateProjectionMatrix();
    outside.visible = true;
    renderer.setScissorTest(true);
    renderer.setViewport(x, y, w, h);
    renderer.setScissor(x, y, w, h);
    renderer.setClearColor(0x0b0e13, 1);
    renderer.render(scene, observer);
    renderer.setClearColor(0x000000, 0);
    renderer.setScissorTest(false);
    renderer.setViewport(0, 0, viewport.width, viewport.height);
    outside.visible = false;
  });

  // --- picking ------------------------------------------------------------------------
  let lastHit: THREE.Intersection | null = null;
  stage.onPick(
    () => (solid ? [solid] : []),
    (hit) => cb.onPick(hit && lastHit ? featureOfHit(lastHit) : null),
    (h) => {
      lastHit = h;
      return true;
    },
  );

  stage.onFrame((dt) => {
    if (move) {
      move.t = Math.min(1, move.t + dt / 0.7);
      const e = move.t < 0.5 ? 2 * move.t * move.t : 1 - (-2 * move.t + 2) ** 2 / 2;
      const { from, to } = move;
      const at = new THREE.Spherical(from.radius + (to.radius - from.radius) * e, from.phi + (to.phi - from.phi) * e, from.theta + (to.theta - from.theta) * e);
      camera.position.copy(target).add(new THREE.Vector3().setFromSpherical(at));
      if (move.t >= 1) move = null;
    }
    controls.update();
  });
  fitOrtho();
  refresh();
  stage.setCamera(camera);

  return {
    setStep(n) {
      step = Math.max(1, Math.min(features.length, n));
      refresh();
    },
    select(id) {
      selected = id && materials.has(id) ? id : null;
      refresh();
    },
    setView(view) {
      goTo(view);
    },
    setOrthographic(on) {
      const next = on ? ortho : perspective;
      if (next === camera) return;
      next.position.copy(camera.position);
      next.quaternion.copy(camera.quaternion);
      camera = next;
      controls.object = camera;
      controls.update();
      stage.setCamera(camera);
    },
    setCameraInset(on) {
      inset = on;
    },
    setLabels(on) {
      labelsOn = on;
      refresh();
    },
    dispose() {
      controls.dispose();
      for (const mesh of solids.values()) mesh?.geometry.dispose();
      stage.dispose();
    },
  };
}
