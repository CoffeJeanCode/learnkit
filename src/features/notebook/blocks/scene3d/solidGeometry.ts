// Meshes of the `solid_model` engine: each feature's profile extruded or
// revolved in the CAD frame (`lib/scene3d/solidBuild.ts` has the
// conventions), then combined with CSG (three-bvh-csg). Free of the DOM, so
// `scripts/check-solid-model.tsx` builds the real solid and measures it.
//
// three-bvh-csg is imported through its ES source: its `main` is a UMD build
// that `require`s three, which Bun refuses; Vite reads the same file.

import * as THREE from "three";
import { ADDITION, Brush, Evaluator, SUBTRACTION } from "three-bvh-csg/src/index.js";
import { isAdditive, normalRange, partBounds, profileOutline } from "../../../../lib/scene3d/solidBuild";
import type { SolidFeature } from "../../../../lib/schemas";

const SEGMENTS = 48;

/** Local (a, b, c) of an extrusion -> CAD (x, y, z). Every basis is
 *  right-handed so faces keep facing out; `xz` draws its shape with v negated
 *  to stay so (z = -b). */
function extrudeBasis(plane: string): THREE.Matrix4 {
  const m = new THREE.Matrix4();
  if (plane === "yz") m.set(0, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1);
  else if (plane === "xz") m.set(1, 0, 0, 0, 0, 0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1);
  return m;
}

/** Local (x, y, z) of a lathe (axis = local Y = the plane's v) -> CAD. */
function latheBasis(plane: string): THREE.Matrix4 {
  const m = new THREE.Matrix4();
  if (plane === "yz") m.set(0, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1);
  else if (plane === "xz") m.set(1, 0, 0, 0, 0, 0, -1, 0, 0, 1, 0, 0, 0, 0, 0, 1);
  return m;
}

export function planeNormal(plane: string): THREE.Vector3 {
  return plane === "yz" ? new THREE.Vector3(1, 0, 0) : plane === "xz" ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(0, 0, 1);
}

/** The point (u, v, n) of a plane, in CAD coordinates. */
export function planePoint(plane: string, u: number, v: number, n: number): THREE.Vector3 {
  if (plane === "yz") return new THREE.Vector3(n, u, v);
  if (plane === "xz") return new THREE.Vector3(u, n, v);
  return new THREE.Vector3(u, v, n);
}

/** Half-length of a "through all" cut: comfortably past the whole part. */
export function throughLength(features: SolidFeature[]): number {
  const b = partBounds(features);
  if (!b) return 100;
  const span = Math.max(...[0, 1, 2].map((k) => b.max[k] - b.min[k]), 1);
  const far = Math.max(...[0, 1, 2].map((k) => Math.max(Math.abs(b.min[k]), Math.abs(b.max[k]))));
  return far + span + 10;
}

/** One feature's solid, in CAD coordinates (null if its profile is incomplete). */
export function featureGeometry(f: SolidFeature, through: number): THREE.BufferGeometry | null {
  const outline = profileOutline(f, SEGMENTS);
  if (outline.length < 3) return null;
  let geometry: THREE.BufferGeometry;
  if (f.op === "revolve") {
    // Closed, counter-clockwise outline in (radius, height): faces point out.
    const points = [...outline, outline[0]].map(([u, v]) => new THREE.Vector2(Math.max(0, u), v));
    geometry = new THREE.LatheGeometry(points, SEGMENTS);
    geometry.applyMatrix4(latheBasis(f.plane));
    const shift = planeNormal(f.plane).multiplyScalar(f.offset);
    geometry.translate(shift.x, shift.y, shift.z);
  } else {
    const [n0, n1] = normalRange(f, through);
    const flip = f.plane === "xz" ? -1 : 1;
    const shape = new THREE.Shape(outline.map(([u, v]) => new THREE.Vector2(u, flip * v)));
    geometry = new THREE.ExtrudeGeometry(shape, { depth: Math.max(n1 - n0, 1e-3), bevelEnabled: false, curveSegments: SEGMENTS });
    geometry.translate(0, 0, n0);
    geometry.applyMatrix4(extrudeBasis(f.plane));
  }
  // CSG needs the same attributes on every brush.
  for (const name of Object.keys(geometry.attributes)) {
    if (!["position", "normal", "uv"].includes(name)) geometry.deleteAttribute(name);
  }
  geometry.computeVertexNormals();
  return geometry;
}

/** The solid after the first `upTo` features. Each feature brings its own
 *  material, so every face of the result — a hole's wall included — knows
 *  which feature made it (`material.userData.featureId`). */
export function buildSolid(features: SolidFeature[], upTo: number, materialFor: (f: SolidFeature, index: number) => THREE.Material): THREE.Mesh | null {
  const through = throughLength(features);
  const evaluator = new Evaluator();
  evaluator.useGroups = true;
  evaluator.attributes = ["position", "normal", "uv"];
  let result: Brush | null = null;
  features.slice(0, upTo).forEach((f, i) => {
    const geometry = featureGeometry(f, through);
    if (!geometry) return;
    const material = materialFor(f, i);
    material.userData.featureId = f.id;
    const brush = new Brush(geometry, material);
    brush.updateMatrixWorld();
    if (!result) {
      if (isAdditive(f.op)) result = brush;
      return;
    }
    result = evaluator.evaluate(result, brush, isAdditive(f.op) ? ADDITION : SUBTRACTION);
  });
  return result;
}

/** The feature id of a picked face of a `buildSolid` mesh. */
export function featureOfHit(hit: THREE.Intersection): string | null {
  const mesh = hit.object as THREE.Mesh;
  const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
  const index = hit.face?.materialIndex ?? 0;
  const id = materials[index]?.userData?.featureId;
  return typeof id === "string" ? id : null;
}

/** The part's real edges — where two faces meet at an angle — as line
 *  segments. `THREE.EdgesGeometry` is not enough after CSG: the result is
 *  full of T-junctions (a long edge on one side, two short ones on the
 *  other), which it reports as open edges, drawing the triangulation of flat
 *  faces as fake lines. Here every segment is judged by ALL faces whose edges
 *  run along it, so a split inside a flat face is dropped. */
export function creaseEdges(geometry: THREE.BufferGeometry, thresholdDeg = 25): THREE.BufferGeometry {
  const pos = geometry.getAttribute("position");
  const index = geometry.getIndex();
  const count = index ? index.count : pos.count;
  const at = (i: number) => new THREE.Vector3().fromBufferAttribute(pos, index ? index.getX(i) : i);
  geometry.computeBoundingBox();
  const span = geometry.boundingBox!.getSize(new THREE.Vector3()).length() || 1;
  const eps = span * 1e-5;
  const cosLimit = Math.cos((thresholdDeg * Math.PI) / 180);

  interface Edge {
    a: THREE.Vector3;
    b: THREE.Vector3;
    normal: THREE.Vector3;
  }
  // Edges grouped by their supporting line: only collinear edges can share a crease.
  const lines = new Map<string, Edge[]>();
  const round = (x: number, q: number) => Math.round(x / q);
  for (let i = 0; i < count; i += 3) {
    const [p, q, r] = [at(i), at(i + 1), at(i + 2)];
    const normal = q.clone().sub(p).cross(r.clone().sub(p));
    if (normal.length() < eps * eps) continue; // degenerate sliver
    normal.normalize();
    for (const [a, b] of [
      [p, q],
      [q, r],
      [r, p],
    ]) {
      const d = b.clone().sub(a);
      const len = d.length();
      if (len < eps) continue;
      d.divideScalar(len);
      // One orientation per line, so a->b and b->a land together.
      const k = Math.abs(d.x) > 1e-9 ? d.x : Math.abs(d.y) > 1e-9 ? d.y : d.z;
      if (k < 0) d.negate();
      const foot = a.clone().addScaledVector(d, -a.dot(d));
      const key = [round(d.x, 1e-3), round(d.y, 1e-3), round(d.z, 1e-3), round(foot.x, span * 1e-4), round(foot.y, span * 1e-4), round(foot.z, span * 1e-4)].join(",");
      const list = lines.get(key) ?? [];
      list.push({ a, b, normal });
      lines.set(key, list);
    }
  }

  const out: number[] = [];
  const seen = new Set<string>();
  const contains = (e: Edge, m: THREE.Vector3) => {
    const ab = e.b.clone().sub(e.a);
    const t = m.clone().sub(e.a).dot(ab) / ab.lengthSq();
    return t > 1e-6 && t < 1 - 1e-6;
  };
  for (const list of lines.values()) {
    for (const e of list) {
      const mid = e.a.clone().add(e.b).multiplyScalar(0.5);
      const id = [e.a, e.b]
        .map((v) => [round(v.x, eps * 10), round(v.y, eps * 10), round(v.z, eps * 10)].join(":"))
        .sort()
        .join("|");
      if (seen.has(id)) continue;
      seen.add(id);
      const normals = list.filter((o) => contains(o, mid)).map((o) => o.normal);
      const crease = normals.some((n, i) => normals.slice(i + 1).some((o) => n.dot(o) < cosLimit));
      if (crease) out.push(e.a.x, e.a.y, e.a.z, e.b.x, e.b.y, e.b.z);
    }
  }
  const lines3 = new THREE.BufferGeometry();
  lines3.setAttribute("position", new THREE.Float32BufferAttribute(out, 3));
  return lines3;
}

/** Height of the material at (u, v) of a plane: the face a through cut
 *  enters — where its sketch and its label belong. */
export function surfaceAlong(features: SolidFeature[], plane: string, u: number, v: number): number | null {
  let top: number | null = null;
  for (const f of features) {
    if (!isAdditive(f.op) || f.plane !== plane) continue;
    const outline = profileOutline(f, 24);
    if (!insidePolygon(outline, u, v) && f.op !== "revolve") continue;
    const [, n1] = normalRange(f, 0);
    top = top === null ? n1 : Math.max(top, n1);
  }
  return top;
}

function insidePolygon(points: Array<[number, number]>, x: number, y: number): boolean {
  let inside = false;
  for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
    const [xi, yi] = points[i];
    const [xj, yj] = points[j];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

/** Signed volume of a closed mesh (positive = faces point out). */
export function signedVolume(geometry: THREE.BufferGeometry): number {
  const pos = geometry.getAttribute("position");
  const index = geometry.getIndex();
  const count = index ? index.count : pos.count;
  const a = new THREE.Vector3();
  const b = new THREE.Vector3();
  const c = new THREE.Vector3();
  let volume = 0;
  for (let i = 0; i < count; i += 3) {
    const [ia, ib, ic] = index ? [index.getX(i), index.getX(i + 1), index.getX(i + 2)] : [i, i + 1, i + 2];
    a.fromBufferAttribute(pos, ia);
    b.fromBufferAttribute(pos, ib);
    c.fromBufferAttribute(pos, ic);
    volume += a.dot(b.clone().cross(c)) / 6;
  }
  return volume;
}
