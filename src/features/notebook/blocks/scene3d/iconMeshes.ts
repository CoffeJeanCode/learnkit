// The closed icon catalogue of `topology_flow`, as procedural low-poly meshes
// in a "blueprint" style: dark faces + chalk-blue edges. No assets to load —
// each icon is a handful of boxes/cylinders, so a whole scene is a few
// hundred triangles.

import * as THREE from "three";
import { FILL, INK } from "./stage";

export interface IconHandle {
  group: THREE.Group;
  /** Height of the icon, to place things above it. */
  height: number;
  /** Recolour the outline (`null` restores the default ink) and set opacity. */
  setTone(color: string | null, opacity: number): void;
}

interface Builder {
  group: THREE.Group;
  fill: THREE.MeshBasicMaterial;
  line: THREE.LineBasicMaterial;
  accent: THREE.MeshBasicMaterial;
  top: number;
}

function add(b: Builder, geometry: THREE.BufferGeometry, x: number, y: number, z: number, opts: { accent?: boolean; rotX?: number; rotZ?: number; edgeAngle?: number } = {}) {
  const part = new THREE.Group();
  part.add(new THREE.Mesh(geometry, opts.accent ? b.accent : b.fill));
  part.add(new THREE.LineSegments(new THREE.EdgesGeometry(geometry, opts.edgeAngle ?? 20), b.line));
  part.position.set(x, y, z);
  if (opts.rotX) part.rotation.x = opts.rotX;
  if (opts.rotZ) part.rotation.z = opts.rotZ;
  b.group.add(part);
  geometry.computeBoundingBox();
  b.top = Math.max(b.top, y + (geometry.boundingBox?.max.y ?? 0));
  return part;
}

const box = (w: number, h: number, d: number) => new THREE.BoxGeometry(w, h, d);
const cyl = (r: number, h: number, seg = 16) => new THREE.CylinderGeometry(r, r, h, seg);

const BUILDERS: Record<string, (b: Builder) => void> = {
  laptop(b) {
    add(b, box(1.4, 0.08, 0.95), 0, 0.04, 0.1);
    add(b, box(1.05, 0.02, 0.5), 0, 0.09, 0.2, { accent: true }); // keyboard
    add(b, box(1.4, 0.9, 0.06), 0, 0.5, -0.42, { rotX: -0.22 });
    add(b, box(1.2, 0.72, 0.02), 0, 0.5, -0.38, { rotX: -0.22, accent: true }); // screen
  },
  phone(b) {
    add(b, box(0.56, 1.05, 0.08), 0, 0.56, 0, { rotX: -0.2 });
    add(b, box(0.46, 0.86, 0.02), 0, 0.57, 0.045, { rotX: -0.2, accent: true });
  },
  server(b) {
    add(b, box(0.95, 1.6, 0.95), 0, 0.8, 0);
    for (let i = 0; i < 4; i++) add(b, box(0.72, 0.16, 0.03), 0, 0.3 + i * 0.34, 0.49, { accent: true });
  },
  database(b) {
    for (let i = 0; i < 3; i++) add(b, cyl(0.58, 0.36, 20), 0, 0.2 + i * 0.44, 0, { accent: i === 2, edgeAngle: 40 });
  },
  router(b) {
    add(b, box(1.4, 0.36, 0.95), 0, 0.18, 0);
    for (let i = 0; i < 4; i++) add(b, box(0.14, 0.08, 0.03), -0.42 + i * 0.28, 0.2, 0.49, { accent: true });
    add(b, cyl(0.03, 0.75, 8), 0.5, 0.72, -0.3);
    add(b, new THREE.IcosahedronGeometry(0.07, 0), 0.5, 1.14, -0.3, { accent: true, edgeAngle: 1 });
  },
  switch(b) {
    add(b, box(1.7, 0.3, 0.85), 0, 0.15, 0);
    for (let i = 0; i < 6; i++) add(b, box(0.15, 0.11, 0.03), -0.6 + i * 0.24, 0.16, 0.44, { accent: true });
  },
  cloud(b) {
    const lobes: Array<[number, number, number, number]> = [
      [0, 0.75, 0, 0.55],
      [-0.55, 0.62, 0.05, 0.4],
      [0.55, 0.62, 0.05, 0.42],
      [0.2, 0.98, -0.1, 0.4],
      [-0.25, 0.92, -0.05, 0.36],
    ];
    for (const [x, y, z, r] of lobes) add(b, new THREE.IcosahedronGeometry(r, 1), x, y, z, { edgeAngle: 1 });
  },
  firewall(b) {
    add(b, box(1.4, 1.15, 0.34), 0, 0.575, 0);
    for (let i = 1; i < 4; i++) add(b, box(1.4, 0.03, 0.36), 0, i * 0.29, 0, { accent: true });
  },
  person(b) {
    add(b, new THREE.CylinderGeometry(0.2, 0.42, 0.85, 12), 0, 0.43, 0, { edgeAngle: 40 });
    add(b, new THREE.IcosahedronGeometry(0.27, 1), 0, 1.14, 0, { accent: true, edgeAngle: 1 });
  },
  service(b) {
    add(b, cyl(0.62, 0.7, 6), 0, 0.35, 0);
    add(b, cyl(0.3, 0.06, 6), 0, 0.74, 0, { accent: true });
  },
  queue(b) {
    for (let i = 0; i < 3; i++) add(b, box(0.42, 0.5, 0.62), -0.52 + i * 0.52, 0.25, 0, { accent: i === 2 });
  },
  storage(b) {
    add(b, box(1.35, 0.9, 0.85), 0, 0.45, 0);
    for (let i = 0; i < 2; i++) add(b, box(1.1, 0.26, 0.03), 0, 0.26 + i * 0.4, 0.44, { accent: true });
  },
  box(b) {
    add(b, box(0.95, 0.95, 0.95), 0, 0.475, 0);
  },
};

export function buildIcon(icon: string): IconHandle {
  const b: Builder = {
    group: new THREE.Group(),
    // polygonOffset pushes faces back so their own outline is never z-fought.
    fill: new THREE.MeshBasicMaterial({ color: FILL, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1, transparent: true }),
    accent: new THREE.MeshBasicMaterial({ color: "#24344d", polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1, transparent: true }),
    line: new THREE.LineBasicMaterial({ color: INK, transparent: true }),
    top: 0,
  };
  (BUILDERS[icon] ?? BUILDERS.box)(b);
  return {
    group: b.group,
    height: b.top,
    setTone(color, opacity) {
      b.line.color.set(color ?? INK);
      b.line.opacity = opacity;
      b.fill.opacity = opacity;
      b.accent.opacity = opacity;
    },
  };
}
