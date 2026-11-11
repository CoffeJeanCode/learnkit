// The shared three.js "stage" of the 3D engines: renderer, HTML label layer,
// resize handling, an animation loop that only runs while the figure is on
// screen, picking, and disposal. Everything three-specific that both scenes
// need lives here, so the scenes only describe WHAT is drawn.
//
// This module (and everything importing it) is loaded with a dynamic
// `import()` from the React components, so three.js never enters the main
// bundle and never runs under the headless render checks.

import * as THREE from "three";
import { CSS2DObject, CSS2DRenderer } from "three/addons/renderers/CSS2DRenderer.js";

export { PALETTE } from "../../../../lib/scene3d/palette";

export const INK = "#8fb4e8"; // blueprint line colour (blue chalk)
export const INK_DIM = "#3d4c63";
export const FILL = "#161b22"; // face colour: a touch lighter than the paper
export const GOLD = "#e8c268";

export interface Stage {
  scene: THREE.Scene;
  renderer: THREE.WebGLRenderer;
  host: HTMLElement;
  /** Size of the drawing area in CSS pixels. */
  size: { width: number; height: number };
  reducedMotion: boolean;
  setCamera(camera: THREE.Camera): void;
  /** Called every frame while visible, with seconds since the last frame. */
  onFrame(cb: (dt: number, elapsed: number) => void): void;
  onResize(cb: (width: number, height: number) => void): void;
  /** Called every frame right after the main render (an inset view, say). */
  onAfterRender(cb: () => void): void;
  /** A click that was not a drag: `pick` receives the hit object (or null). */
  /** `accept` can skip hits the eye cannot see (e.g. on a surface the cut removed). */
  onPick(targets: () => THREE.Object3D[], pick: (hit: THREE.Object3D | null) => void, accept?: (hit: THREE.Intersection) => boolean): void;
  dispose(): void;
}

export function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" && !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

/** Builds the stage inside `host`, or returns null when WebGL is unavailable
 *  (old GPU, blocked context, headless) — callers then show their fallback. */
export function createStage(host: HTMLElement): Stage | null {
  let renderer: THREE.WebGLRenderer;
  try {
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, powerPreference: "low-power" });
  } catch {
    return null;
  }
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setClearColor(0x000000, 0);
  renderer.domElement.className = "scene3d-canvas";
  host.appendChild(renderer.domElement);

  const labels = new CSS2DRenderer();
  labels.domElement.className = "scene3d-labels";
  host.appendChild(labels.domElement);

  const scene = new THREE.Scene();
  let camera: THREE.Camera | null = null;
  const size = { width: 1, height: 1 };
  const frameCbs: Array<(dt: number, elapsed: number) => void> = [];
  const resizeCbs: Array<(w: number, h: number) => void> = [];
  const afterCbs: Array<() => void> = [];

  const applySize = () => {
    const width = Math.max(1, host.clientWidth);
    const height = Math.max(1, host.clientHeight);
    size.width = width;
    size.height = height;
    renderer.setSize(width, height, false);
    labels.setSize(width, height);
    resizeCbs.forEach((cb) => cb(width, height));
  };
  const resizeObserver = new ResizeObserver(applySize);
  resizeObserver.observe(host);

  // Only burn frames while the figure is actually on screen.
  let visible = true;
  const intersection = new IntersectionObserver((entries) => {
    visible = entries.some((e) => e.isIntersecting);
    if (visible) schedule();
  });
  intersection.observe(host);

  let raf = 0;
  let frameCount = 0;
  let last = 0;
  let elapsed = 0;
  let disposed = false;
  const tick = (now: number) => {
    raf = 0;
    if (disposed) return;
    // Capped so a long stall (throttled tab, GC pause) never teleports a packet.
    const dt = last ? Math.min(0.25, (now - last) / 1000) : 0;
    last = now;
    elapsed += dt;
    frameCbs.forEach((cb) => cb(dt, elapsed));
    if (camera) {
      renderer.render(scene, camera);
      labels.render(scene, camera);
      // Labels are re-placed by the camera every frame; untangle them a few
      // times a second (cheap: a handful of rects).
      if (++frameCount < 4 || frameCount % 6 === 0) declutterLabels(labels.domElement, host);
      afterCbs.forEach((cb) => cb());
    }
    if (visible && document.visibilityState !== "hidden") schedule();
    else last = 0;
  };
  function schedule() {
    if (!raf && !disposed) raf = requestAnimationFrame(tick);
  }
  const onVisibility = () => schedule();
  document.addEventListener("visibilitychange", onVisibility);

  const pickCleanups: Array<() => void> = [];

  const stage: Stage = {
    scene,
    renderer,
    host,
    size,
    reducedMotion: prefersReducedMotion(),
    setCamera(c) {
      camera = c;
      applySize();
      schedule();
    },
    onFrame(cb) {
      frameCbs.push(cb);
    },
    onResize(cb) {
      resizeCbs.push(cb);
    },
    onAfterRender(cb) {
      afterCbs.push(cb);
    },
    onPick(targets, pick, accept) {
      const raycaster = new THREE.Raycaster();
      let down: { x: number; y: number } | null = null;
      const el = renderer.domElement;
      const onDown = (e: PointerEvent) => {
        down = { x: e.clientX, y: e.clientY };
      };
      const onUp = (e: PointerEvent) => {
        if (!down || !camera) return;
        const moved = Math.hypot(e.clientX - down.x, e.clientY - down.y);
        down = null;
        if (moved > 6) return; // that was a drag (orbit), not a tap
        const rect = el.getBoundingClientRect();
        const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
        raycaster.setFromCamera(ndc, camera);
        const hit = raycaster.intersectObjects(targets(), true).find((h) => h.object.visible && (!accept || accept(h)));
        pick(hit ? hit.object : null);
      };
      el.addEventListener("pointerdown", onDown);
      el.addEventListener("pointerup", onUp);
      pickCleanups.push(() => {
        el.removeEventListener("pointerdown", onDown);
        el.removeEventListener("pointerup", onUp);
      });
    },
    dispose() {
      disposed = true;
      if (raf) cancelAnimationFrame(raf);
      resizeObserver.disconnect();
      intersection.disconnect();
      document.removeEventListener("visibilitychange", onVisibility);
      pickCleanups.forEach((fn) => fn());
      scene.traverse((obj) => {
        const o = obj as THREE.Mesh;
        o.geometry?.dispose?.();
        const material = o.material as THREE.Material | THREE.Material[] | undefined;
        if (Array.isArray(material)) material.forEach((m) => m.dispose());
        else material?.dispose?.();
        if (obj instanceof CSS2DObject) obj.element.remove();
      });
      renderer.dispose();
      renderer.domElement.remove();
      labels.domElement.remove();
    },
  };
  applySize();
  return stage;
}

/**
 * Keeps 3D labels from piling up on each other (and from spilling out of the
 * stage). CSS2DRenderer rewrites each label's `transform` every frame, so the
 * nudge goes in `margin-top` / `margin-left`, which it never touches. Greedy:
 * the selected label keeps its place, the rest move down past whatever they
 * overlap (a few steps at most), and anything poking out of the stage is
 * pulled back in. Exported for the check scripts.
 */
export function declutterLabels(layer: HTMLElement, host: HTMLElement): number {
  const els = Array.from(layer.children).filter((e): e is HTMLElement => e instanceof HTMLElement && e.style.display !== "none" && e.offsetParent !== null);
  for (const e of els) {
    e.style.marginTop = "";
    e.style.marginLeft = "";
  }
  // Active (selected) labels first: they never move.
  els.sort((a, b) => Number(b.querySelector(".is-active, .is-current") !== null || b.classList.contains("is-active")) - Number(a.querySelector(".is-active, .is-current") !== null || a.classList.contains("is-active")));
  const bounds = host.getBoundingClientRect();
  const placed: DOMRect[] = [];
  let moved = 0;
  const hit = (r: DOMRect, o: DOMRect) => r.left < o.right && o.left < r.right && r.top < o.bottom && o.top < r.bottom;
  for (const e of els) {
    let dy = 0;
    let dx = 0;
    let r = e.getBoundingClientRect();
    if (r.width === 0) continue;
    // Pull back inside the stage horizontally.
    if (r.left < bounds.left + 2) dx = bounds.left + 2 - r.left;
    else if (r.right > bounds.right - 2) dx = bounds.right - 2 - r.right;
    for (let step = 0; step < 4; step++) {
      const shifted = new DOMRect(r.left + dx, r.top + dy, r.width, r.height);
      const blocker = placed.find((o) => hit(shifted, o));
      if (!blocker) break;
      dy += blocker.bottom - shifted.top + 2;
    }
    if (dx || dy) {
      e.style.marginLeft = dx ? `${dx}px` : "";
      e.style.marginTop = dy ? `${dy}px` : "";
      moved++;
      r = new DOMRect(r.left + dx, r.top + dy, r.width, r.height);
    }
    placed.push(r);
  }
  return moved;
}

/** An HTML label anchored to a 3D point. `anchorY` 0 = the label hangs below
 *  the point, 1 = it sits above it. */
export function makeLabel(element: HTMLElement, anchorY = 0): CSS2DObject {
  const label = new CSS2DObject(element);
  label.center.set(0.5, anchorY);
  return label;
}

/** Walks up from a picked mesh to the first ancestor carrying `userData[key]`. */
export function pickedId(hit: THREE.Object3D | null, key: string): string | null {
  for (let o: THREE.Object3D | null = hit; o; o = o.parent) {
    const id = o.userData?.[key];
    if (typeof id === "string") return id;
  }
  return null;
}
