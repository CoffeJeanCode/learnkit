// The sandbox document of a `code_figure` (tier 2): a canvas, the figure SDK
// (`fig.*`, documented to the escalation model in `P_CODE_FIGURE`) and the
// model's code, inside an iframe with `sandbox="allow-scripts"` (no same
// origin: no access to the app, its storage or its DOM) and a CSP that
// forbids every network request. The code was already screened in Rust
// (`domain::code_figure`); this is the second wall.

const SDK = String.raw`
(() => {
  const W = Number(document.body.dataset.w), H = Number(document.body.dataset.h);
  const canvas = document.getElementById("c");
  const dpr = Math.min(2, window.devicePixelRatio || 1);
  canvas.width = W * dpr; canvas.height = H * dpr; canvas.style.width = W + "px"; canvas.style.height = H + "px";
  const ctx = canvas.getContext("2d"); ctx.scale(dpr, dpr);
  const controls = document.getElementById("controls");
  const report = (type, message) => window.parent.postMessage({ source: "code-figure", type, message: String(message || "") }, "*");
  window.addEventListener("error", (e) => report("error", e.message));
  let yaw = 0.6, pitch = 0.35, drag = null;
  canvas.addEventListener("pointerdown", (e) => { drag = [e.clientX, e.clientY]; canvas.setPointerCapture(e.pointerId); });
  canvas.addEventListener("pointermove", (e) => { if (!drag) return; yaw += (e.clientX - drag[0]) * 0.01; pitch = Math.max(-1.4, Math.min(1.4, pitch + (e.clientY - drag[1]) * 0.01)); drag = [e.clientX, e.clientY]; });
  canvas.addEventListener("pointerup", () => { drag = null; });
  const frames = [];
  const fig = {
    ctx, width: W, height: H,
    clear() { ctx.clearRect(0, 0, W, H); },
    slider({ label = "", min = 0, max = 1, value = 0, step }) {
      const row = document.createElement("label"); row.className = "s";
      const name = document.createElement("span"); name.textContent = label;
      const out = document.createElement("output");
      const input = document.createElement("input"); input.type = "range"; input.min = min; input.max = max; input.step = step || (max - min) / 100; input.value = value;
      const sync = () => { out.textContent = Number(input.value).toLocaleString("es", { maximumFractionDigits: 3 }); };
      input.addEventListener("input", sync); sync();
      row.append(name, input, out); controls.append(row);
      return () => Number(input.value);
    },
    onFrame(cb) { frames.push(cb); },
    project([x, y, z]) {
      const cy = Math.cos(yaw), sy = Math.sin(yaw), cp = Math.cos(pitch), sp = Math.sin(pitch);
      const x1 = x * cy - z * sy, z1 = x * sy + z * cy;
      const y2 = y * cp - z1 * sp, z2 = y * sp + z1 * cp;
      const s = Math.min(W, H) * 0.32 / (1 + z2 * 0.12);
      return [W / 2 + x1 * s, H / 2 - y2 * s];
    },
    label(text, x, y, color = "#e2e8f0") {
      ctx.font = "13px system-ui, sans-serif";
      const w = ctx.measureText(text).width + 10;
      ctx.fillStyle = "rgba(15,23,42,0.85)"; ctx.fillRect(x - 5, y - 13, w, 18);
      ctx.fillStyle = color; ctx.fillText(text, x, y);
    },
    arrow(x1, y1, x2, y2, color = "#38bdf8") {
      ctx.strokeStyle = color; ctx.fillStyle = color; ctx.lineWidth = 2;
      ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke();
      const a = Math.atan2(y2 - y1, x2 - x1);
      ctx.beginPath(); ctx.moveTo(x2, y2); ctx.lineTo(x2 - 10 * Math.cos(a - 0.4), y2 - 10 * Math.sin(a - 0.4)); ctx.lineTo(x2 - 10 * Math.cos(a + 0.4), y2 - 10 * Math.sin(a + 0.4)); ctx.closePath(); ctx.fill();
    },
  };
  window.fig = fig;
  let last = performance.now();
  const tick = (now) => {
    const dt = Math.min(0.1, (now - last) / 1000); last = now;
    for (const cb of frames) { try { cb(now / 1000, dt); } catch (e) { report("error", e && e.message); frames.length = 0; } }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  window.__figReady = () => report("ready");
})();
`;

/** `</script` inside the model's code would close the tag early. */
const escapeScript = (code: string) => code.replace(/<\/script/gi, "<\\/script");

export function codeFigureDocument(code: string, height: number, width = 720): string {
  return `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:">
<style>
html,body{margin:0;background:#0f1115;color:#e2e8f0;font:14px system-ui,sans-serif}
canvas{display:block;cursor:grab;touch-action:none}
#controls{display:grid;gap:6px;padding:8px 4px}
.s{display:grid;grid-template-columns:1fr 2fr auto;gap:8px;align-items:center}
input[type=range]{accent-color:#38bdf8}
</style></head>
<body data-w="${width}" data-h="${height}"><canvas id="c"></canvas><div id="controls"></div>
<script>${SDK}</script>
<script>try{${escapeScript(code)}
}catch(e){window.parent.postMessage({source:"code-figure",type:"error",message:String(e&&e.message)},"*")}
window.__figReady&&window.__figReady();</script>
</body></html>`;
}
