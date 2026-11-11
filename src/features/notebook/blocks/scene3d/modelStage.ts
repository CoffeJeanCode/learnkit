// three.js scene of the `explorable_model` engine: a schematic object built
// from catalogue shapes that the student orbits, cuts open and taps.
// React (`ExplorableModelVisual.tsx`) owns the selection and the toggles; this
// file owns the drawing.

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { hashString } from "../../../../lib/rng";
import { partColor } from "../../../../lib/scene3d/palette";
import { CUT_NORMAL, CUT_OFFSET, SHELL_RADII, isLayer, layerFractions, placeParts } from "../../../../lib/scene3d/partPlacement";
import { SHELL_TARGET } from "../../../../lib/schemas";
import { buildPart, buildShellGeometry } from "./partMeshes";
import { createStage, makeLabel, pickedId } from "./stage";

export interface ModelData {
  shell?: { label: string; shape: string; cutaway: boolean } | null;
  parts: Array<{ id: string; label: string; shape: string; size?: string | null; count: number; zone?: string | null; color?: string | null }>;
}

export interface ModelCallbacks {
  /** A part was tapped (null = empty space). */
  onPick(id: string | null): void;
}

export interface ModelController {
  select(id: string | null): void;
  setIsolated(isolated: boolean): void;
  setCut(cut: boolean): void;
  setLabels(visible: boolean): void;
  resetView(): void;
  dispose(): void;
}

export function mountModel(host: HTMLElement, data: ModelData, cb: ModelCallbacks): ModelController | null {
  const stage = createStage(host);
  if (!stage) return null;
  const { scene, renderer } = stage;
  renderer.localClippingEnabled = true;

  scene.add(new THREE.HemisphereLight("#dfe9ff", "#1a1f29", 1.15));
  const sun = new THREE.DirectionalLight("#ffffff", 1.6);
  sun.position.set(4, 6, 5);
  scene.add(sun);

  const shellShape = data.shell?.shape ?? "sphere";
  const radii = SHELL_RADII[shellShape] ?? SHELL_RADII.sphere;
  const fractions = layerFractions(data.parts);
  const outermost = Math.max(1, ...fractions.values());
  const reach = Math.max(...radii) * Math.max(outermost, data.parts.some((p) => p.zone === "outside" && !isLayer(p)) ? 1.75 : 1.15);

  // --- camera -------------------------------------------------------------------
  const camera = new THREE.PerspectiveCamera(36, 1, 0.1, 100);
  // A little off the cut's axis, so the opening reads as a bowl and not as a flat disc.
  const home = new THREE.Vector3(-0.2, 0.42, 1).normalize().multiplyScalar(reach * 3.6);
  camera.position.copy(home);
  stage.onResize((w, h) => {
    camera.aspect = w / h;
    // Narrow figures pull the camera back so the model still fits.
    camera.fov = w / h < 1 ? 36 / Math.max(0.55, w / h) : 36;
    camera.updateProjectionMatrix();
  });
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enablePan = false;
  controls.enableDamping = true;
  controls.minDistance = reach * 1.6;
  controls.maxDistance = reach * 7;
  controls.autoRotate = !stage.reducedMotion;
  controls.autoRotateSpeed = 0.9;
  // The idle spin stops for good the first time the student takes over.
  controls.addEventListener("start", () => {
    controls.autoRotate = false;
  });

  // --- shell ----------------------------------------------------------------------
  const cutPlane = new THREE.Plane(new THREE.Vector3(...CUT_NORMAL).normalize().negate(), CUT_OFFSET);
  let shell: THREE.Mesh | null = null;
  let shellMaterial: THREE.MeshStandardMaterial | null = null;
  if (data.shell) {
    shellMaterial = new THREE.MeshStandardMaterial({
      color: "#9db6d8",
      transparent: true,
      opacity: 0.22,
      roughness: 0.5,
      side: THREE.DoubleSide,
      depthWrite: false,
    });
    shell = new THREE.Mesh(buildShellGeometry(data.shell.shape, radii), shellMaterial);
    shell.renderOrder = 2; // after the opaque parts, so they show through
    scene.add(shell);
  }

  // --- parts ----------------------------------------------------------------------
  const materials = new Map<string, THREE.MeshStandardMaterial>();
  const groups = new Map<string, THREE.Group>();
  const labelEls = new Map<string, HTMLElement>();
  data.parts.forEach((part, index) => {
    const material = new THREE.MeshStandardMaterial({ color: partColor(part.color, index), roughness: 0.62, flatShading: true, transparent: true });
    materials.set(part.id, material);
    const group = new THREE.Group();
    group.userData.partId = part.id;
    groups.set(part.id, group);
    scene.add(group);
  });
  const placed = placeParts(data.parts, data.shell, !!data.shell?.cutaway);
  // Concentric layers: nested copies of the envelope's shape, opaque and cut
  // open together with it, so the student sees the bands like in a textbook
  // cross-section. Their label sits on the rim, on the side the cut keeps.
  const LABEL_DIR = new THREE.Vector3(-0.75, 0.55, 0.2).normalize();
  const baseColors = new Map<string, THREE.Color>();
  data.parts.forEach((part) => {
    const f = fractions.get(part.id);
    if (f === undefined) return;
    const material = materials.get(part.id)!;
    material.side = THREE.DoubleSide;
    material.transparent = false;
    baseColors.set(part.id, material.color.clone());
    const scaled = radii.map((r) => r * f) as [number, number, number];
    const mesh = new THREE.Mesh(buildShellGeometry(shellShape, scaled), material);
    groups.get(part.id)!.add(mesh);
    const el = document.createElement("button");
    el.type = "button";
    el.className = "scene3d-part-label";
    el.textContent = part.label;
    el.addEventListener("click", () => cb.onPick(part.id));
    const label = makeLabel(el, 1);
    label.position.set(LABEL_DIR.x * scaled[0], LABEL_DIR.y * scaled[1], LABEL_DIR.z * scaled[2]);
    groups.get(part.id)!.add(label);
    labelEls.set(part.id, el);
  });
  const layerIds = new Set(fractions.keys());
  // One label per part, on its outermost copy: labels spread out instead of piling up at the centre.
  const labelAt = new Map<string, (typeof placed)[number]>();
  for (const instance of placed) {
    const best = labelAt.get(instance.partId);
    if (!best || Math.hypot(...instance.position) > Math.hypot(...best.position)) labelAt.set(instance.partId, instance);
  }
  for (const instance of placed) {
    const part = data.parts.find((p) => p.id === instance.partId)!;
    const group = groups.get(part.id)!;
    const copy = buildPart(part.shape, instance.radius, materials.get(part.id)!, hashString(`${part.id}#${instance.index}`));
    copy.position.set(...instance.position);
    copy.rotation.set(...instance.rotation);
    group.add(copy);
    if (labelAt.get(part.id) !== instance) continue;
    const el = document.createElement("button");
    el.type = "button";
    el.className = "scene3d-part-label";
    el.textContent = part.label;
    el.addEventListener("click", () => cb.onPick(part.id));
    const label = makeLabel(el, 1);
    label.position.set(instance.position[0], instance.position[1] + instance.radius + 0.06, instance.position[2]);
    group.add(label);
    labelEls.set(part.id, el);
  }
  stage.onPick(
    () => [...groups.values()],
    (hit) => cb.onPick(pickedId(hit, "partId")),
    // A layer's surface removed by the cut is invisible: let the tap reach what is behind it.
    (hit) => !(cut && layerIds.has(pickedId(hit.object, "partId") ?? "") && hit.point.dot(cutDirection) > CUT_OFFSET),
  );

  // --- state ----------------------------------------------------------------------
  let selected: string | null = null;
  let isolated = false;
  // Layers are only legible cut open, so a layered model starts open.
  let cut = !!data.shell?.cutaway || fractions.size > 0;
  const cutDirection = new THREE.Vector3(...CUT_NORMAL).normalize();
  let labelsVisible = true;

  const refresh = () => {
    const hideOthers = isolated && selected !== null;
    for (const [id, material] of materials) {
      const isSelected = id === selected;
      const group = groups.get(id)!;
      group.visible = !hideOthers || isSelected;
      const dim = selected !== null && !isSelected;
      if (layerIds.has(id)) {
        // Nested shells cannot fade: a see-through outer layer hides nothing and
        // a see-through inner one vanishes. They stay solid and darken instead.
        material.color.copy(baseColors.get(id)!).multiplyScalar(dim ? 0.4 : 1);
      } else {
        material.opacity = dim ? 0.5 : 1;
        material.depthWrite = !dim;
      }
      material.emissive.set(isSelected ? (baseColors.get(id) ?? material.color) : "#000000");
      material.emissiveIntensity = isSelected ? 0.35 : 0;
      if (layerIds.has(id)) {
        material.clippingPlanes = cut ? [cutPlane] : [];
        material.needsUpdate = true;
      }
      const el = labelEls.get(id);
      if (el) {
        el.style.display = labelsVisible && group.visible ? "" : "none";
        el.classList.toggle("is-active", isSelected);
        el.classList.toggle("is-dim", dim);
      }
    }
    if (shell && shellMaterial) {
      const shellSelected = selected === SHELL_TARGET;
      shell.visible = !hideOthers || shellSelected;
      shellMaterial.clippingPlanes = cut ? [cutPlane] : [];
      shellMaterial.opacity = shellSelected ? 0.6 : cut ? 0.36 : 0.22;
      shellMaterial.emissive.set(shellSelected ? shellMaterial.color : "#000000");
      shellMaterial.emissiveIntensity = shellSelected ? 0.3 : 0;
      shellMaterial.needsUpdate = true;
    }
  };
  refresh();

  // Isolating a part also brings the camera to it; leaving isolation steps back.
  const centroid = (id: string) => {
    const f = id === SHELL_TARGET ? 1 : fractions.get(id);
    if (f !== undefined) return { centre: new THREE.Vector3(), spread: Math.max(...radii) * f };
    const copies = placed.filter((i) => i.partId === id);
    const sum = copies.reduce((acc, i) => acc.add(new THREE.Vector3(...i.position)), new THREE.Vector3());
    const centre = sum.divideScalar(Math.max(1, copies.length));
    const spread = Math.max(...copies.map((i) => centre.distanceTo(new THREE.Vector3(...i.position)) + i.radius));
    return { centre, spread };
  };
  const frame = () => {
    const focused = isolated && selected !== null ? centroid(selected) : null;
    const distance = focused ? Math.max(1.7, focused.spread * 6) : home.length();
    const direction = camera.position.clone().sub(controls.target).normalize();
    controls.minDistance = Math.min(distance * 0.6, reach * 1.6);
    controls.target.copy(focused ? focused.centre : new THREE.Vector3());
    camera.position.copy(controls.target).addScaledVector(direction, distance);
    controls.update();
  };

  stage.onFrame(() => controls.update());
  stage.setCamera(camera);

  return {
    select(id) {
      selected = id && (materials.has(id) || (id === SHELL_TARGET && shell)) ? id : null;
      refresh();
      if (isolated) frame();
    },
    setIsolated(value) {
      if (isolated === value) return;
      isolated = value;
      refresh();
      frame();
    },
    setCut(value) {
      cut = value;
      refresh();
    },
    setLabels(visible) {
      labelsVisible = visible;
      refresh();
    },
    resetView() {
      controls.target.set(0, 0, 0);
      camera.position.copy(home);
      frame();
    },
    dispose() {
      controls.dispose();
      stage.dispose();
    },
  };
}
