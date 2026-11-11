// The closed shape catalogue of `explorable_model`, as procedural low-poly
// geometry. Every builder returns geometry that fits inside a sphere of the
// given bounding `radius`, so placement (`lib/scene3d/partPlacement.ts`) can
// reason about parts as spheres without knowing their shape.

import * as THREE from "three";
import { mulberry32 } from "../../../../lib/rng";

type Geo = THREE.BufferGeometry;

function moved(geometry: Geo, x: number, y: number, z: number): Geo {
  geometry.translate(x, y, z);
  return geometry;
}

function tube(points: THREE.Vector3[], thickness: number, segments = 40): Geo {
  return new THREE.TubeGeometry(new THREE.CatmullRomCurve3(points), segments, thickness, 6, false);
}

const SHAPES: Record<string, (r: number, seed: number) => Geo[]> = {
  sphere: (r) => [new THREE.IcosahedronGeometry(r, 2)],
  ellipsoid: (r) => [new THREE.IcosahedronGeometry(r, 2).scale(1, 0.62, 0.62)],
  capsule: (r) => [new THREE.CapsuleGeometry(r * 0.42, r * 1.16, 4, 10).rotateZ(Math.PI / 2)],
  torus: (r) => [new THREE.TorusGeometry(r * 0.72, r * 0.26, 8, 18)],
  box: (r) => [new THREE.BoxGeometry(r * 1.15, r * 1.15, r * 1.15)],
  cylinder: (r) => [new THREE.CylinderGeometry(r * 0.6, r * 0.6, r * 1.5, 14)],
  // Flattened sacs piled up and slightly bowed (a Golgi apparatus, a stack of thylakoids).
  disc_stack: (r) =>
    [-1.5, -0.5, 0.5, 1.5].map((level, i) => {
      const width = r * (0.95 - Math.abs(level) * 0.14);
      return moved(new THREE.CylinderGeometry(width, width, r * 0.17, 14).scale(1, 1, 0.62), (i % 2 ? 1 : -1) * r * 0.05, level * r * 0.3, 0);
    }),
  // A tangle of thin tubes (endoplasmic reticulum, vessels, wiring).
  tubes: (r, seed) => {
    const rnd = mulberry32(seed);
    return [0, 1, 2].map(() => {
      const points = Array.from({ length: 5 }, (_, i) => {
        const along = (i / 4) * 2 - 1;
        return new THREE.Vector3(along * r * 0.85, (rnd() - 0.5) * r * 1.1, (rnd() - 0.5) * r * 1.1);
      });
      return tube(points, r * 0.11, 28);
    });
  },
  // Two intertwined strands (DNA, a spring, a coiled protein).
  helix: (r) =>
    [0, Math.PI].map((phase) => {
      const points = Array.from({ length: 25 }, (_, i) => {
        const t = i / 24;
        const angle = t * Math.PI * 4 + phase;
        return new THREE.Vector3(Math.cos(angle) * r * 0.42, (t * 2 - 1) * r * 0.9, Math.sin(angle) * r * 0.42);
      });
      return tube(points, r * 0.09, 60);
    }),
  // A small cluster of granules (ribosomes, vesicles, crystals).
  grains: (r, seed) => {
    const rnd = mulberry32(seed);
    return Array.from({ length: 6 }, () => {
      const u = rnd() * 2 - 1;
      const phi = rnd() * Math.PI * 2;
      const s = Math.sqrt(1 - u * u);
      const d = r * 0.58 * Math.cbrt(rnd());
      return moved(new THREE.IcosahedronGeometry(r * 0.3, 0), s * Math.cos(phi) * d, u * d, s * Math.sin(phi) * d);
    });
  },
  // An irregular lump (a lysosome, a rock, an amoeba).
  blob: (r, seed) => {
    const geometry = new THREE.IcosahedronGeometry(r * 0.86, 3);
    const position = geometry.attributes.position;
    const a = (seed % 7) + 2;
    const v = new THREE.Vector3();
    for (let i = 0; i < position.count; i++) {
      v.fromBufferAttribute(position, i);
      const n = v.clone().normalize();
      const bump = 1 + 0.14 * Math.sin(n.x * a + seed) * Math.cos(n.y * (a + 1.7)) + 0.08 * Math.sin(n.z * (a + 3.1));
      v.multiplyScalar(bump);
      position.setXYZ(i, v.x, v.y, v.z);
    }
    geometry.computeVertexNormals();
    return [geometry];
  },
};

/** One copy of a part: a group of meshes sharing `material`. */
export function buildPart(shape: string, radius: number, material: THREE.Material, seed: number): THREE.Group {
  const group = new THREE.Group();
  for (const geometry of (SHAPES[shape] ?? SHAPES.sphere)(radius, seed)) group.add(new THREE.Mesh(geometry, material));
  return group;
}

/** The translucent envelope; `radii` are its half-extents. */
export function buildShellGeometry(shape: string, radii: [number, number, number]): Geo {
  const [x, y, z] = radii;
  switch (shape) {
    case "box":
      return new THREE.BoxGeometry(x * 2, y * 2, z * 2);
    case "cylinder":
      return new THREE.CylinderGeometry(x, x, y * 2, 40, 1);
    case "capsule":
      return new THREE.CapsuleGeometry(y, (x - y) * 2, 10, 28).rotateZ(Math.PI / 2);
    default:
      return new THREE.SphereGeometry(1, 48, 32).scale(x, y, z);
  }
}
