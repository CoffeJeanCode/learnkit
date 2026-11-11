/**
 * Visual judge for generated figures (phase D): renders every figure of an
 * eval report in headless Chrome (the dev-only `figure-preview.html`), then
 * asks a VISION model to grade the screenshot against the block's own text.
 *
 *   bun scripts/judge-figures.ts --report=eval-results/scene3d-….json [--judge=mimo-v2.6-pro] [--limit=20]
 *   bun scripts/judge-figures.ts --class=eval-results/class-qa/relatividad_general.json
 *
 * Rubric (1-5 each): idea (shows the rule's core idea), coherence (agrees
 * with the text and its values, never contradicts it), legibility (labels
 * readable, nothing overlapping or cut off), plus concrete `issues`.
 * Writes eval-results/figure-judge-<timestamp>.json.
 */
import { spawn } from "child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "fs";
import { resolve } from "path";

const ROOT = resolve(import.meta.dir, "..");
const argv = process.argv.slice(2);
const arg = (k: string) => argv.find((a) => a.startsWith(`--${k}=`))?.split("=").slice(1).join("=");
const PORT = 1431;
const CHROME = process.env.CHROME_PATH ?? "C:/Program Files/Google/Chrome/Application/chrome.exe";

function env(): Record<string, string> {
  const out: Record<string, string> = { ...(process.env as Record<string, string>) };
  try {
    for (const line of readFileSync(resolve(ROOT, ".env"), "utf-8").split(/\r?\n/)) {
      const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*)\s*$/);
      if (m && !out[m[1]]) out[m[1]] = m[2].replace(/^"|"$/g, "");
    }
  } catch {
    /* no .env */
  }
  return out;
}
const ENV = env();

interface Figure {
  id: string;
  model: string;
  text: string;
  visual: unknown;
}

/** The figures to judge, with the text they must agree with. */
function collect(): Figure[] {
  const report = arg("report");
  const cls = arg("class");
  const out: Figure[] = [];
  const textOf = (b: Record<string, unknown>) =>
    ["title", "systemRule", "frequentError"].map((k) => b[k]).filter(Boolean).join("\n") ||
    (Array.isArray(b.guidedWalkthrough) ? (b.guidedWalkthrough as Array<{ pedagogicalInsight?: string }>).map((s) => s.pedagogicalInsight).join("\n") : "");
  if (report) {
    for (const r of JSON.parse(readFileSync(resolve(ROOT, report), "utf-8")).results as Array<{ model: string; scenario: string; raw: string }>) {
      try {
        let raw = r.raw.trim().replace(/^```(?:json)?\s*/i, "").replace(/\s*```$/, "");
        raw = raw.slice(raw.indexOf("{"));
        const block = JSON.parse(raw);
        if (typeof block.visualAid === "string") block.visualAid = JSON.parse(block.visualAid);
        if (block.visualAid) out.push({ id: `${r.model}/${r.scenario}`, model: r.model, text: textOf(block), visual: block.visualAid });
      } catch {
        /* unparsable output: nothing to judge */
      }
    }
  }
  if (cls) {
    const c = JSON.parse(readFileSync(resolve(ROOT, cls), "utf-8"));
    for (const b of c.blocks as Array<{ order: number; content: Record<string, unknown> }>) {
      if (b.content.visualAid) out.push({ id: `${c.topic}/#${b.order}`, model: "app", text: textOf(b.content), visual: b.content.visualAid });
    }
  }
  const limit = Number(arg("limit") ?? 0);
  return limit ? out.slice(0, limit) : out;
}

async function waitFor(url: string, ms = 60_000) {
  const until = Date.now() + ms;
  while (Date.now() < until) {
    try {
      if ((await fetch(url)).ok) return;
    } catch {
      /* not up yet */
    }
    await Bun.sleep(500);
  }
  throw new Error(`no responde ${url}`);
}

function screenshot(visual: unknown, out: string): Promise<void> {
  const hash = Buffer.from(JSON.stringify({ visual })).toString("base64url");
  const url = `http://localhost:${PORT}/figure-preview.html#${hash}`;
  return new Promise((done, fail) => {
    const p = spawn(CHROME, [
      "--headless=new", "--force-prefers-reduced-motion", "--disable-gpu-sandbox", "--use-angle=swiftshader", "--enable-unsafe-swiftshader",
      "--hide-scrollbars", "--window-size=800,900", "--virtual-time-budget=9000", `--screenshot=${out}`, url,
    ]);
    p.on("exit", (code) => (code === 0 ? done() : fail(new Error(`chrome salió con ${code}`))));
  });
}

const RUBRIC = `Eres un revisor de figuras educativas. Ves la CAPTURA de una figura interactiva y el TEXTO de la teoría que debe ilustrar.
Califica de 1 a 5:
- idea: ¿la figura deja VER la idea central del texto (no solo la menciona)?
- coherence: ¿coincide con el texto y sus valores, sin contradecirlo ni enseñar otro modelo?
- legibility: ¿las etiquetas se leen, sin solaparse ni cortarse, y la figura no está vacía ni rota?
Devuelve SOLO JSON: {"idea": n, "coherence": n, "legibility": n, "issues": ["problema concreto", ...]}`;

async function judge(png: string, text: string, model: string): Promise<Record<string, unknown>> {
  const image = `data:image/png;base64,${readFileSync(png).toString("base64")}`;
  const mimo = model.includes("mimo");
  const url = mimo ? "https://api.xiaomimimo.com/v1/chat/completions" : "https://api.anthropic.com/v1/messages";
  if (!mimo) throw new Error("solo se implementa el juez OpenAI-compatible (MiMo); pasa --judge=mimo-v2.6-pro");
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${ENV.MIMO_API_KEY || ENV.XIAOMI_MIMO_API_KEY}` },
    body: JSON.stringify({
      model,
      temperature: 0,
      thinking: { type: "disabled" },
      messages: [
        { role: "system", content: RUBRIC },
        { role: "user", content: [{ type: "text", text: `TEXTO DE LA TEORÍA:\n${text.slice(0, 2500)}` }, { type: "image_url", image_url: { url: image } }] },
      ],
    }),
    signal: AbortSignal.timeout(120_000),
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}: ${(await res.text()).slice(0, 200)}`);
  const data = (await res.json()) as { choices?: Array<{ message?: { content?: string } }> };
  const content = data.choices?.[0]?.message?.content ?? "";
  return JSON.parse(content.slice(content.indexOf("{"), content.lastIndexOf("}") + 1));
}

async function main() {
  const figures = collect();
  if (!figures.length) throw new Error("ninguna figura: pasa --report=… o --class=…");
  const judgeModel = arg("judge") ?? "mimo-v2.6-pro";
  const shots = resolve(ROOT, "eval-results", "figure-shots");
  rmSync(shots, { recursive: true, force: true });
  mkdirSync(shots, { recursive: true });
  const vite = spawn("bun", ["x", "vite", "--port", String(PORT), "--strictPort"], { cwd: ROOT, stdio: "ignore", shell: true });
  try {
    await waitFor(`http://localhost:${PORT}/figure-preview.html`);
    const results = [];
    for (const [i, f] of figures.entries()) {
      const png = resolve(shots, `${String(i).padStart(3, "0")}.png`);
      try {
        await screenshot(f.visual, png);
        const verdict = await judge(png, f.text, judgeModel);
        results.push({ ...f, png, verdict });
        const v = verdict as { idea?: number; coherence?: number; legibility?: number; issues?: string[] };
        console.log(`${f.id.padEnd(40)} idea ${v.idea} · coherencia ${v.coherence} · legibilidad ${v.legibility}  ${(v.issues ?? []).slice(0, 2).join(" | ")}`);
      } catch (e) {
        results.push({ ...f, png, error: String(e) });
        console.log(`${f.id.padEnd(40)} 💥 ${e}`);
      }
    }
    const scored = results.filter((r) => "verdict" in r) as Array<{ verdict: { idea: number; coherence: number; legibility: number } }>;
    const avg = (k: "idea" | "coherence" | "legibility") => (scored.reduce((a, r) => a + Number(r.verdict[k] ?? 0), 0) / Math.max(1, scored.length)).toFixed(2);
    console.log(`\n${scored.length}/${results.length} juzgadas · idea ${avg("idea")} · coherencia ${avg("coherence")} · legibilidad ${avg("legibility")} (juez ${judgeModel})`);
    const out = resolve(ROOT, "eval-results", `figure-judge-${new Date().toISOString().replace(/[:.]/g, "-")}.json`);
    writeFileSync(out, JSON.stringify({ judge: judgeModel, results }, null, 2));
    console.log(`Informe: ${out}`);
  } finally {
    vite.kill();
    if (process.platform === "win32") spawn("powershell", ["-NoProfile", "-Command", `Get-NetTCPConnection -LocalPort ${PORT} -State Listen -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }`]);
  }
}

await main();
