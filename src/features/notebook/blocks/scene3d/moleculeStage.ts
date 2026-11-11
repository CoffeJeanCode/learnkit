// three.js scene of the `molecule_3d` engine: ball-and-stick atoms laid out by
// `lib/molecule.ts` (never by the model), bonds by order, and — on a
// mechanism step — the bonds forming (dashed gold) and breaking (red).

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { ELEMENT_STYLE, type Vec3 } from "../../../../lib/molecule";
import { createStage, makeLabel, pickedId } from "./stage";

export interface MoleculeFrame {
  atoms: Array<{ id: string; element: string; charge: number; label?: string | null }>;
  bonds: Array<{ a: string; b: string; order: number }>;
  positions: Map<string, Vec3>;
  forming: Array<[string, string]>;
  breaking: Array<[string, string]>;
  highlight: string[];
}

export interface MoleculeController {
  setFrame(frame: MoleculeFrame): void;
  select(id: string | null): void;
  dispose(): void;
}

export function mountMolecule(host: HTMLElement, cb: { onPick(id: string | null): void }): MoleculeController | null {
  const stage = createStage(host);
  if (!stage) return null;
  const { scene, renderer } = stage;
  scene.add(new THREE.HemisphereLight("#eef2ff", "#1a1f29", 1.2));
  const sun = new THREE.DirectionalLight("#ffffff", 1.5);
  sun.position.set(4, 6, 5);
  scene.add(sun);
  const camera = new THREE.PerspectiveCamera(40, 1, 0.1, 200);
  camera.position.set(0, 2, 9);
  stage.onResize((w, h) => {
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  });
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.autoRotate = !stage.reducedMotion;
  controls.autoRotateSpeed = 0.8;
  controls.addEventListener("start", () => (controls.autoRotate = false));

  let group = new THREE.Group();
  scene.add(group);
  const atomMeshes = new Map<string, THREE.Mesh>();
  let selected: string | null = null;
  let highlight = new Set<string>();

  const cylinder = (a: THREE.Vector3, b: THREE.Vector3, radius: number, material: THREE.Material) => {
    const dir = b.clone().sub(a);
    const mesh = new THREE.Mesh(new THREE.CylinderGeometry(radius, radius, dir.length(), 12), material);
    mesh.position.copy(a).addScaledVector(dir, 0.5);
    mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir.clone().normalize());
    return mesh;
  };

  function refresh() {
    for (const [id, m] of atomMeshes) {
      const mat = m.material as THREE.MeshStandardMaterial;
      const on = id === selected || highlight.has(id);
      mat.emissive.set(on ? "#fbbf24" : "#000000");
      mat.emissiveIntensity = on ? 0.45 : 0;
    }
  }

  stage.onPick(
    () => [...atomMeshes.values()],
    (hit) => cb.onPick(pickedId(hit, "atomId")),
  );
  stage.onFrame(() => controls.update());
  stage.setCamera(camera);

  return {
    setFrame(frame) {
      scene.remove(group);
      group.traverse((o) => {
        const m = o as THREE.Mesh;
        m.geometry?.dispose?.();
        (m.material as THREE.Material | undefined)?.dispose?.();
        if ((o as { isCSS2DObject?: boolean }).isCSS2DObject) (o as unknown as { element: HTMLElement }).element.remove();
      });
      group = new THREE.Group();
      scene.add(group);
      atomMeshes.clear();
      highlight = new Set(frame.highlight);
      const at = (id: string) => new THREE.Vector3(...(frame.positions.get(id) ?? [0, 0, 0]));
      for (const a of frame.atoms) {
        const style = ELEMENT_STYLE[a.element] ?? { color: "#94a3b8", r: 0.4 };
        const mesh = new THREE.Mesh(new THREE.SphereGeometry(style.r, 24, 18), new THREE.MeshStandardMaterial({ color: style.color, roughness: 0.4 }));
        mesh.position.copy(at(a.id));
        mesh.userData.atomId = a.id;
        group.add(mesh);
        atomMeshes.set(a.id, mesh);
        const el = document.createElement("span");
        el.className = "molecule-atom-label";
        const sign = a.charge > 0 ? (a.charge > 1 ? `${a.charge}+` : "+") : a.charge < 0 ? (a.charge < -1 ? `${-a.charge}−` : "−") : "";
        el.textContent = `${a.label ?? a.element}${sign ? ` ${sign}` : ""}`;
        const label = makeLabel(el, 0.5);
        label.position.copy(at(a.id)).add(new THREE.Vector3(0, style.r + 0.18, 0));
        group.add(label);
      }
      const bondMat = new THREE.MeshStandardMaterial({ color: "#cbd5e1", roughness: 0.5 });
      for (const b of frame.bonds) {
        const p = at(b.a);
        const q = at(b.b);
        if (b.order === 1) {
          group.add(cylinder(p, q, 0.08, bondMat));
          continue;
        }
        // Parallel sticks for double/triple bonds.
        const dir = q.clone().sub(p).normalize();
        const side = new THREE.Vector3(0, 1, 0).cross(dir).normalize();
        if (side.lengthSq() < 1e-6) side.set(1, 0, 0);
        for (let k = 0; k < b.order; k++) {
          const off = side.clone().multiplyScalar((k - (b.order - 1) / 2) * 0.16);
          group.add(cylinder(p.clone().add(off), q.clone().add(off), 0.055, bondMat));
        }
      }
      const dashed = (pair: [string, string], color: string) => {
        const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints([at(pair[0]), at(pair[1])]), new THREE.LineDashedMaterial({ color, dashSize: 0.15, gapSize: 0.1 }));
        line.computeLineDistances();
        group.add(line);
      };
      frame.forming.forEach((p) => dashed(p, "#fbbf24"));
      frame.breaking.forEach((p) => dashed(p, "#f87171"));
      // Frame the molecule.
      const box = new THREE.Box3().setFromObject(group);
      const size = box.getSize(new THREE.Vector3()).length() || 4;
      controls.target.copy(box.getCenter(new THREE.Vector3()));
      camera.position.copy(controls.target).add(new THREE.Vector3(0, size * 0.2, size * 1.15));
      refresh();
    },
    select(id) {
      selected = id;
      refresh();
    },
    dispose() {
      controls.dispose();
      stage.dispose();
    },
  };
}
