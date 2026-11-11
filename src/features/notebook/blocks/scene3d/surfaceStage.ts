// three.js scene of the `surface_3d` engine: a surface z = f(x, y) the
// student turns around, with the paths the system traced on it (geodesics,
// descents, level curves), beads that travel along them, named markers and
// the shaded volume of a region. React (`Surface3dVisual.tsx`) computes the
// math (`lib/scene3d/surfaceBuild.ts`) and owns the state; this draws it.

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { Pt3, SurfaceGrid, TracedPath } from "../../../../lib/scene3d/surfaceBuild";
import { GOLD, INK, createStage, makeLabel, pickedId } from "./stage";

export interface SurfaceScene {
  grid: SurfaceGrid;
  /** Vertical exaggeration applied to every height. */
  scale: number;
  paths: Array<TracedPath & { label: string; color: string }>;
  markers: Array<{ id: string; label: string; at: Pt3; color: string }>;
  /** [x0, x1, y0, y1] of the shaded volume. */
  region?: [number, number, number, number] | null;
  zLabel?: string | null;
}

export interface SurfaceController {
  setScene(scene: SurfaceScene): void;
  select(id: string | null): void;
  setAnimate(on: boolean): void;
  setLabels(on: boolean): void;
  dispose(): void;
}

/** Height colours, low → high (the palette's sky → violet → amber). */
const RAMP = [new THREE.Color("#1e3a5f"), new THREE.Color("#38bdf8"), new THREE.Color("#c084fc"), new THREE.Color("#fbbf24")];
function ramp(t: number): THREE.Color {
  const x = Math.min(0.999, Math.max(0, t)) * (RAMP.length - 1);
  const i = Math.floor(x);
  return RAMP[i].clone().lerp(RAMP[i + 1], x - i);
}

export function mountSurface(host: HTMLElement, xr: [number, number], yr: [number, number], cb: { onPick(id: string | null): void }): SurfaceController | null {
  const stage = createStage(host);
  if (!stage) return null;
  const { scene, renderer } = stage;
  scene.add(new THREE.HemisphereLight("#e6eeff", "#1a1f29", 1.05));
  const sun = new THREE.DirectionalLight("#ffffff", 1.4);
  sun.position.set(4, 9, 6);
  scene.add(sun);

  // Math frame (x, y, z up) -> three (Y up), centred and normalised to ~10 units.
  const span = Math.max(xr[1] - xr[0], yr[1] - yr[0]);
  const k = 10 / span;
  const cx = (xr[0] + xr[1]) / 2;
  const cy = (yr[0] + yr[1]) / 2;
  const world = new THREE.Group();
  world.rotation.x = -Math.PI / 2;
  world.scale.setScalar(k);
  world.position.set(-cx * k, 0, cy * k);
  scene.add(world);

  const camera = new THREE.PerspectiveCamera(38, 1, 0.1, 400);
  camera.position.set(9, 9, 12);
  stage.onResize((w, h) => {
    camera.aspect = w / h;
    camera.fov = w / h < 1 ? 38 / Math.max(0.55, w / h) : 38;
    camera.updateProjectionMatrix();
  });
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.enablePan = false;
  controls.minDistance = 6;
  controls.maxDistance = 45;

  let content = new THREE.Group();
  world.add(content);
  const pickables = new Map<string, THREE.Object3D>();
  const pathMaterials = new Map<string, THREE.MeshStandardMaterial>();
  const labelEls = new Map<string, HTMLElement>();
  const beads: Array<{ id: string; mesh: THREE.Mesh; points: Pt3[] }> = [];
  let selected: string | null = null;
  let animate = !stage.reducedMotion;
  let labelsOn = true;
  let clock = 0;

  const lift = span * 0.004; // paths float a hair above the surface, never inside it

  const disposeGroup = (g: THREE.Object3D) =>
    g.traverse((o) => {
      const m = o as THREE.Mesh;
      m.geometry?.dispose?.();
      const mat = m.material as THREE.Material | THREE.Material[] | undefined;
      if (Array.isArray(mat)) mat.forEach((x) => x.dispose());
      else mat?.dispose?.();
      if ((o as { isCSS2DObject?: boolean }).isCSS2DObject) (o as unknown as { element: HTMLElement }).element.remove();
    });

  function build(s: SurfaceScene) {
    world.remove(content);
    disposeGroup(content);
    content = new THREE.Group();
    world.add(content);
    pickables.clear();
    pathMaterials.clear();
    labelEls.clear();
    beads.length = 0;

    const { grid, scale } = s;
    const n = grid.n;
    const lo = grid.zMin * scale;
    const hi = grid.zMax * scale;
    // Surface: one vertex per sample, coloured by height.
    const pos = new Float32Array(n * n * 3);
    const col = new Float32Array(n * n * 3);
    for (let i = 0; i < n; i++) {
      for (let j = 0; j < n; j++) {
        const z = Number.isFinite(grid.z[i][j]) ? grid.z[i][j] * scale : lo;
        const o = (i * n + j) * 3;
        pos.set([grid.xs[i], grid.ys[j], z], o);
        const c = ramp(hi > lo ? (z - lo) / (hi - lo) : 0.5);
        col.set([c.r, c.g, c.b], o);
      }
    }
    const index: number[] = [];
    for (let i = 0; i < n - 1; i++) {
      for (let j = 0; j < n - 1; j++) {
        const a = i * n + j;
        index.push(a, a + n, a + 1, a + 1, a + n, a + n + 1);
      }
    }
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    geom.setAttribute("color", new THREE.BufferAttribute(col, 3));
    geom.setIndex(index);
    geom.computeVertexNormals();
    const surface = new THREE.Mesh(geom, new THREE.MeshStandardMaterial({ vertexColors: true, side: THREE.DoubleSide, roughness: 0.75, transparent: true, opacity: 0.92 }));
    content.add(surface);
    // The "fabric": a coarse grid drawn on the surface, so the curvature reads.
    const lines: number[] = [];
    const every = Math.max(1, Math.round(n / 16));
    for (let i = 0; i < n; i += every) {
      for (let j = 0; j < n - 1; j++) {
        const a = (i * n + j) * 3;
        const b = (i * n + j + 1) * 3;
        lines.push(pos[a], pos[a + 1], pos[a + 2] + lift / 2, pos[b], pos[b + 1], pos[b + 2] + lift / 2);
        const c = (j * n + i) * 3;
        const d = ((j + 1) * n + i) * 3;
        lines.push(pos[c], pos[c + 1], pos[c + 2] + lift / 2, pos[d], pos[d + 1], pos[d + 2] + lift / 2);
      }
    }
    const fabric = new THREE.LineSegments(
      new THREE.BufferGeometry().setAttribute("position", new THREE.Float32BufferAttribute(lines, 3)),
      new THREE.LineBasicMaterial({ color: "#0b0e13", transparent: true, opacity: 0.35 }),
    );
    content.add(fabric);

    // Axes along the near edges, at the height of that corner of the surface
    // (not at the bottom of a well, where they would float in the void); the
    // height axis is a bar spanning the whole relief.
    const corner = Number.isFinite(grid.z[0][0]) ? grid.z[0][0] * scale : lo;
    const base = new THREE.Vector3(xr[0], yr[0], corner);
    for (const [name, from, dir, len] of [
      ["x", base, new THREE.Vector3(1, 0, 0), xr[1] - xr[0]],
      ["y", base, new THREE.Vector3(0, 1, 0), yr[1] - yr[0]],
      [s.zLabel || "z", new THREE.Vector3(xr[0], yr[0], lo), new THREE.Vector3(0, 0, 1), Math.max(hi - lo, span * 0.2)],
    ] as const) {
      const end = from.clone().addScaledVector(dir, len * 1.05);
      content.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints([from, end]), new THREE.LineBasicMaterial({ color: INK })));
      const el = document.createElement("span");
      el.className = "solid-axis-label";
      el.style.color = INK;
      el.textContent = name;
      const label = makeLabel(el, 0.5);
      label.position.copy(end);
      content.add(label);
    }

    // Paths: a tube along the traced points, and a bead that travels on it.
    for (const p of s.paths) {
      if (p.points.length < 2) continue;
      const pts = p.points.map(([x, y, z]) => new THREE.Vector3(x, y, z + lift));
      const curve = new THREE.CatmullRomCurve3(pts.filter((_, i) => i % 3 === 0 || i === pts.length - 1));
      const material = new THREE.MeshStandardMaterial({ color: p.color, emissive: p.color, emissiveIntensity: 0.25, roughness: 0.4 });
      const tube = new THREE.Mesh(new THREE.TubeGeometry(curve, Math.min(400, pts.length), span * 0.009, 8, false), material);
      tube.userData.pickId = p.id;
      content.add(tube);
      pickables.set(p.id, tube);
      pathMaterials.set(p.id, material);
      const bead = new THREE.Mesh(new THREE.SphereGeometry(span * 0.022, 16, 12), new THREE.MeshStandardMaterial({ color: p.color, emissive: p.color, emissiveIntensity: 0.5 }));
      bead.userData.pickId = p.id;
      content.add(bead);
      beads.push({ id: p.id, mesh: bead, points: p.points.map(([x, y, z]) => [x, y, z + lift]) });
      addLabel(p.id, p.label, pts[Math.floor(pts.length * 0.75)] ?? pts[pts.length - 1]);
    }
    for (const m of s.markers) {
      const dot = new THREE.Mesh(new THREE.SphereGeometry(span * 0.03, 18, 14), new THREE.MeshStandardMaterial({ color: m.color, emissive: m.color, emissiveIntensity: 0.4 }));
      dot.position.set(m.at[0], m.at[1], m.at[2] + lift * 3);
      dot.userData.pickId = m.id;
      content.add(dot);
      pickables.set(m.id, dot);
      addLabel(m.id, m.label, dot.position.clone().add(new THREE.Vector3(0, 0, span * 0.05)));
    }
    if (s.region) {
      // The volume under the surface over the rectangle, down to z = 0.
      const [x0, x1, y0, y1] = s.region;
      const steps = 24;
      const top: number[] = [];
      const walls: number[] = [];
      const f = (x: number, y: number) => {
        const i = Math.round(((x - xr[0]) / (xr[1] - xr[0])) * (n - 1));
        const j = Math.round(((y - yr[0]) / (yr[1] - yr[0])) * (n - 1));
        const v = grid.z[Math.min(n - 1, Math.max(0, i))][Math.min(n - 1, Math.max(0, j))];
        return Number.isFinite(v) ? v * scale : 0;
      };
      for (let a = 0; a < steps; a++) {
        for (let b = 0; b < steps; b++) {
          const xa = x0 + ((x1 - x0) * a) / steps;
          const xb = x0 + ((x1 - x0) * (a + 1)) / steps;
          const ya = y0 + ((y1 - y0) * b) / steps;
          const yb = y0 + ((y1 - y0) * (b + 1)) / steps;
          top.push(xa, ya, f(xa, ya), xb, ya, f(xb, ya), xb, yb, f(xb, yb), xa, ya, f(xa, ya), xb, yb, f(xb, yb), xa, yb, f(xa, yb));
        }
      }
      for (let a = 0; a <= steps; a++) {
        for (const [x, y] of [
          [x0 + ((x1 - x0) * a) / steps, y0],
          [x0 + ((x1 - x0) * a) / steps, y1],
          [x0, y0 + ((y1 - y0) * a) / steps],
          [x1, y0 + ((y1 - y0) * a) / steps],
        ]) {
          walls.push(x, y, 0, x, y, f(x, y));
        }
      }
      const topGeom = new THREE.BufferGeometry().setAttribute("position", new THREE.Float32BufferAttribute(top, 3));
      topGeom.computeVertexNormals();
      content.add(new THREE.Mesh(topGeom, new THREE.MeshStandardMaterial({ color: GOLD, transparent: true, opacity: 0.55, side: THREE.DoubleSide })));
      content.add(new THREE.LineSegments(new THREE.BufferGeometry().setAttribute("position", new THREE.Float32BufferAttribute(walls, 3)), new THREE.LineBasicMaterial({ color: GOLD, transparent: true, opacity: 0.7 })));
    }
    // Aim at the middle of the relief.
    controls.target.set(0, ((lo + hi) / 2) * k, 0);
    refresh();
  }

  function addLabel(id: string, text: string, at: THREE.Vector3) {
    const el = document.createElement("button");
    el.type = "button";
    el.className = "scene3d-part-label";
    el.textContent = text;
    el.addEventListener("click", () => cb.onPick(id));
    const label = makeLabel(el, 0);
    label.position.copy(at);
    content.add(label);
    labelEls.set(id, el);
  }

  function refresh() {
    for (const [id, m] of pathMaterials) {
      const on = selected === null || selected === id;
      m.emissiveIntensity = selected === id ? 0.7 : 0.25;
      m.opacity = on ? 1 : 0.35;
      m.transparent = !on;
    }
    for (const [id, el] of labelEls) {
      el.style.display = labelsOn ? "" : "none";
      el.classList.toggle("is-active", id === selected);
      el.classList.toggle("is-dim", selected !== null && id !== selected);
    }
  }

  stage.onPick(
    () => [...pickables.values(), ...beads.map((b) => b.mesh)],
    (hit) => cb.onPick(pickedId(hit, "pickId")),
  );
  stage.onFrame((dt) => {
    if (animate) clock += dt;
    // Each bead loops along its path in ~6 s (reduced motion: parked at 60 %).
    const t = animate ? (clock / 6) % 1 : 0.6;
    for (const b of beads) {
      const p = b.points[Math.min(b.points.length - 1, Math.floor(t * b.points.length))];
      if (p) b.mesh.position.set(p[0], p[1], p[2]);
    }
    controls.update();
  });
  stage.setCamera(camera);

  return {
    setScene: build,
    select(id) {
      selected = id;
      refresh();
    },
    setAnimate(on) {
      animate = on;
    },
    setLabels(on) {
      labelsOn = on;
      refresh();
    },
    dispose() {
      controls.dispose();
      stage.dispose();
    },
  };
}
