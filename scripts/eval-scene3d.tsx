/**
 * Real-LLM evaluation of the 3D interactive diagrams (three.js engines
 * `topology_flow` / `explorable_model`).
 *
 * Fidelity first: the model sees what it sees in production —
 *   - the SYSTEM PROMPT is the real `lean_prompt(DeclarativeVisualDiagram)`,
 *     rebuilt from the raw-string sections of
 *     `src-tauri/src/agents/block_generator_agent.rs` (so prompt edits are
 *     picked up with no copy to keep in sync);
 *   - the TOOL SCHEMA is the real `visual_aid_json_schema()`, parsed out of
 *     the `json!` literal in `src-tauri/src/tools/notebook_tools.rs` and
 *     wrapped in the `declarative_visual_diagram` variant;
 *   - the USER MESSAGE has the shape of `render_next_block_input` (course,
 *     milestone, class, blocksSoFar, blockPlan, …), as JSON instead of TOON.
 *
 * The model is NOT told which engine to use: it picks from all seven, exactly
 * like in the app. Each output is then judged by the app's own code on the
 * RAW output (before the Rust repair passes in `domain::visual_repair`):
 *   engine choice · `StaticVisualSpecSchema` · `scoreVisual` · walkthrough
 *   targets resolve to real ids · coordinates leaked · layout/placement run
 *   (`layoutTopology`, `placeParts` + `overlapDepth`) · the HTML fallback
 *   renders (`StaticVisual` via renderToStaticMarkup, no GPU needed).
 *
 * Usage:
 *   bun scripts/eval-scene3d.tsx                         # every model with a key, every scenario
 *   bun scripts/eval-scene3d.tsx --models=deepseek-chat,mimo-v2.6-pro
 *   bun scripts/eval-scene3d.tsx --scenario=cell_plant   # one scenario (id or engine)
 *   bun scripts/eval-scene3d.tsx --repeat=3              # N samples per (model, scenario)
 *   bun scripts/eval-scene3d.tsx --dry-run               # print prompt/schema sizes, no API calls
 *   bun scripts/eval-scene3d.tsx --block=theory          # the theory block's own figure (anchored_micro_theory.visualAid)
 *   bun scripts/eval-scene3d.tsx --self-test             # judge the gallery's hand-made 3D blocks (must all pass)
 *
 * Writes the full report (raw outputs included) to eval-results/scene3d-<timestamp>.json.
 */

import { mkdirSync, readFileSync, writeFileSync } from "fs";
import { resolve } from "path";
import { renderToStaticMarkup } from "react-dom/server";
import { StaticVisual } from "../src/features/notebook/blocks/StaticVisual";
import { BLOCK_CATEGORIES } from "../src/features/gallery/mockPresets";
import { StaticVisualSpecSchema } from "../src/lib/schemas";
import { overlapDepth, placeParts } from "../src/lib/scene3d/partPlacement";
import { layoutTopology, routeMessage } from "../src/lib/scene3d/topologyLayout";
import { scoreVisual } from "../src/lib/visualQuality";
import { mermaidTypeOf } from "../src/lib/mermaidTypes";
import { featureBox } from "../src/lib/scene3d/solidBuild";

const ROOT = resolve(import.meta.dir, "..");

// --- env / args ---------------------------------------------------------------

function loadEnv(): Record<string, string> {
  const env: Record<string, string> = { ...(process.env as Record<string, string>) };
  try {
    for (const line of readFileSync(resolve(ROOT, ".env"), "utf-8").split("\n")) {
      const t = line.trim();
      if (!t || t.startsWith("#")) continue;
      const eq = t.indexOf("=");
      if (eq <= 0) continue;
      let v = t.slice(eq + 1).trim();
      if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) v = v.slice(1, -1);
      env[t.slice(0, eq).trim()] = v;
    }
  } catch {}
  return env;
}
const env = loadEnv();
const MIMO_API_KEY = env.MIMO_API_KEY || env.XIAOMI_MIMO_API_KEY || env.XIAOMI_API_KEY;
const DEEPSEEK_API_KEY = env.DEEPSEEK_API_KEY;

const argv = process.argv.slice(2);
const arg = (name: string) => argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3);
const DRY_RUN = argv.includes("--dry-run");
const REPEAT = Math.max(1, Number(arg("repeat") ?? 1));
const CONCURRENCY = Math.max(1, Number(arg("concurrency") ?? 3));

interface Provider {
  label: string;
  url: string;
  key: string;
  model: string;
  extraBody?: Record<string, unknown>;
}

function provider(model: string): Provider | null {
  if (model.includes("mimo")) {
    if (!MIMO_API_KEY) return null;
    return { label: model, url: "https://api.xiaomimimo.com/v1/chat/completions", key: MIMO_API_KEY, model, extraBody: { thinking: { type: "disabled" } } };
  }
  if (!DEEPSEEK_API_KEY) return null;
  return { label: model, url: "https://api.deepseek.com/chat/completions", key: DEEPSEEK_API_KEY, model };
}

const DEFAULT_MODELS = ["mimo-v2.6-pro", "mimo-v2.6-flash", "deepseek-chat"];
const requestedModels = (arg("models") ?? DEFAULT_MODELS.join(",")).split(",").map((m) => m.trim()).filter(Boolean);

// --- the real prompt + schema, read from the Rust sources ----------------------

const AGENT_RS = readFileSync(resolve(ROOT, "src-tauri/src/agents/block_generator_agent.rs"), "utf-8");
const TOOLS_RS = readFileSync(resolve(ROOT, "src-tauri/src/tools/notebook_tools.rs"), "utf-8");
const NOTEBOOK_RS = readFileSync(resolve(ROOT, "src-tauri/src/domain/notebook.rs"), "utf-8");
const ENCODING_RS = readFileSync(resolve(ROOT, "src-tauri/src/prompt_encoding.rs"), "utf-8");
const ROUTING_RS = readFileSync(resolve(ROOT, "src-tauri/src/domain/visual_routing.rs"), "utf-8");

/** Rust expressions inside the `json!` schema literals, replaced by their values. */
function rustJson(body: string): string {
  const mermaidAsStr = NOTEBOOK_RS.slice(NOTEBOOK_RS.indexOf("impl MermaidChartType"));
  const chartTypes = [...mermaidAsStr.slice(mermaidAsStr.indexOf("pub fn as_str"), mermaidAsStr.indexOf("pub fn headers")).matchAll(/=> "([A-Za-z]+)"/g)].map((m) => m[1]);
  const intents = [...ROUTING_RS.matchAll(/key: "([a-z_]+)"/g)].map((m) => m[1]);
  return body
    .replace(/crate::domain::notebook::MermaidChartType::ALL\.map\(\|t\| t\.as_str\(\)\)/g, JSON.stringify(chartTypes))
    .replace(/crate::domain::visual_routing::VISUAL_INTENTS\.iter\(\)\.map\(\|i\| i\.key\)\.collect::<Vec<_>>\(\)/g, JSON.stringify(intents))
    .replace(/crate::domain::([a-z_]+)::([A-Z_]+)/g, (_, module: string, name: string) => {
      const src = module === "notebook" ? NOTEBOOK_RS : readFileSync(resolve(ROOT, `src-tauri/src/domain/${module}.rs`), "utf-8");
      const m = src.match(new RegExp(`pub const ${name}: \\[&str; \\d+\\] =\\s*(\\[[^\\]]*\\])`));
      if (!m) throw new Error(`no encuentro la constante ${name} en domain/${module}.rs`);
      return m[1];
    })
    .replace(/solid_model_schema\(\)/g, () => JSON.stringify(rustSchemaFn("solid_model_schema")))
    .replace(/surface_3d_schema\(\)/g, () => JSON.stringify(rustSchemaFn("surface_3d_schema")))
    .replace(/visual_intent_schema\(\)/g, () => JSON.stringify(rustSchemaFn("visual_intent_schema")));
}

/** `visualDirective` of a course, as `domain::visual_routing::directive` writes it. */
function rustDirective(domain: "Spatial3d" | "Network" | "Quantitative"): string {
  const m = ROUTING_RS.match(new RegExp(`VisualDomain::${domain} => Some\\(\\s*"([\\s\\S]*?)",\\s*\\)`));
  if (!m) throw new Error(`no encuentro la directiva ${domain} en visual_routing.rs`);
  return m[1].replace(/\\\r?\n\s*/g, "");
}

function rustConst(name: string): string {
  const m = AGENT_RS.match(new RegExp(`const ${name}: &str = r##"([\\s\\S]*?)"##;`));
  if (!m) throw new Error(`no encuentro const ${name} en block_generator_agent.rs — ¿cambió el prompt?`);
  return m[1];
}

/** Mirrors `lean_prompt(DeclarativeVisualDiagram | AnchoredMicroTheory)` + `with_toon_hint`. */
function leanPrompt(variant: "DeclarativeVisualDiagram" | "AnchoredMicroTheory"): string {
  const entry = AGENT_RS.match(new RegExp(`\\(DynamicBlockType::${variant}, r##"([\\s\\S]*?)"##\\)`));
  if (!entry) throw new Error(`no encuentro la entrada de catálogo de ${variant}`);
  // The theory block also carries its template bullet (`templates_for`).
  let templates = "";
  if (variant === "AnchoredMicroTheory") {
    const bullet = AGENT_RS.match(/\("tpl_theory", r##"([\s\S]*?)"##\)/);
    if (!bullet) throw new Error("no encuentro el bullet de tpl_theory");
    templates = rustConst("P_TEMPLATES_HEADER") + bullet[1] + rustConst("P_TEMPLATES_FOOTER");
  }
  const toon = ENCODING_RS.match(/const TOON_HINT: &str = "([\s\S]*?)";/);
  const toonHint = toon ? toon[1].replace(/\\\r?\n\s*/g, "") : "";
  const s = [
    "P_INTRO", "P_PLAN_NOTE", "P_PERSONALIZATION", "P_SCAFFOLDING", "P_WRITING", "P_CATALOG_HEADER",
  ].map(rustConst).join("") + entry[1] + rustConst("P_CATALOG_TRAILER") + templates + ["P_VISUALS", "P_SCENE3D", "P_SVG", "P_FINAL"].map(rustConst).join("");
  // The app sends TOON; this eval sends JSON, so say so instead of the TOON hint.
  return toonHint ? `${s}\n\nLos datos del mensaje del usuario están en JSON.` : s;
}

/** Balanced-brace slice starting at the `{` at or after `from`. */
function sliceObject(src: string, from: number): string {
  const start = src.indexOf("{", from);
  let depth = 0;
  let inStr = false;
  for (let i = start; i < src.length; i++) {
    const c = src[i];
    if (inStr) {
      if (c === "\\") i++;
      else if (c === '"') inStr = false;
      continue;
    }
    if (c === '"') inStr = true;
    else if (c === "{") depth++;
    else if (c === "}" && --depth === 0) return src.slice(start, i + 1);
  }
  throw new Error("objeto sin cerrar");
}

/** `visual_aid_json_schema()` from notebook_tools.rs, as real JSON. */
function visualAidSchema(): unknown {
  const fn = TOOLS_RS.indexOf("fn visual_aid_json_schema()");
  if (fn < 0) throw new Error("no encuentro visual_aid_json_schema() en notebook_tools.rs");
  const variableAt = TOOLS_RS.indexOf("let variable = serde_json::json!(", fn);
  const variable = sliceObject(TOOLS_RS, variableAt);
  const bodyAt = TOOLS_RS.indexOf("serde_json::json!(", variableAt + variable.length);
  let body = rustJson(sliceObject(TOOLS_RS, bodyAt));
  body = body.replace(/:\s*variable\b/g, `: ${variable}`);
  const schema = JSON.parse(body);
  // Variants the Rust fn pushes after the literal (`molecule_3d_schema()`,
  // `domain_kit_schemas()`): read them the same way.
  schema.oneOf.push(rustSchemaFn("molecule_3d_schema"));
  const kitsAt = TOOLS_RS.indexOf("fn domain_kit_schemas()");
  const kitsEnd = TOOLS_RS.indexOf("\n}\n", kitsAt) > 0 ? TOOLS_RS.indexOf("\n}\n", kitsAt) : TOOLS_RS.indexOf("\r\n}\r\n", kitsAt);
  for (let at = TOOLS_RS.indexOf("serde_json::json!(", kitsAt); at > 0 && at < kitsEnd; ) {
    const obj = sliceObject(TOOLS_RS, at);
    schema.oneOf.push(JSON.parse(rustJson(obj)));
    at = TOOLS_RS.indexOf("serde_json::json!(", at + obj.length);
  }
  return schema;
}

/** `--block=theory` evaluates the theory block's own figure instead of the diagram block. */
const THEORY = arg("block") === "theory";
const SYSTEM_PROMPT = leanPrompt(THEORY ? "AnchoredMicroTheory" : "DeclarativeVisualDiagram");
// In the app the full visual schema lives once, on the diagram variant of the
// same tool, and the theory points at it; with a single-variant tool here the
// theory gets the full schema inline so the model sees the same shapes.
/** The JSON body of a zero-argument Rust schema fn like `theory_visual_aid_schema()`. */
function rustSchemaFn(name: string): unknown {
  const at = TOOLS_RS.indexOf(`fn ${name}()`);
  if (at < 0) throw new Error(`no encuentro ${name}() en notebook_tools.rs`);
  return JSON.parse(rustJson(sliceObject(TOOLS_RS, TOOLS_RS.indexOf("serde_json::json!(", at))));
}

/**
 * One `oneOf` variant of `single_block_json_schema()` / `block_template_schemas()`,
 * as the app sends it: found by its `blockType` const, schema fns inlined.
 */
function rustBlockVariant(blockType: string): unknown {
  const at = TOOLS_RS.indexOf(`"blockType": {"const": "${blockType}"}`);
  if (at < 0) throw new Error(`no encuentro la variante ${blockType} en notebook_tools.rs`);
  // The variant object opens at the `{` right before its `"properties": {`.
  const props = TOOLS_RS.lastIndexOf(`"properties": {`, at);
  const open = TOOLS_RS.lastIndexOf("{", props - 1);
  let body = sliceObject(TOOLS_RS, open);
  body = body
    .replace(/visual_aid_json_schema\(\)/g, JSON.stringify(visualAidSchema()))
    .replace(/theory_visual_aid_schema\(\)/g, JSON.stringify(rustSchemaFn("theory_visual_aid_schema")))
    .replace(/visual_aid_pointer_schema\(\)/g, JSON.stringify(rustSchemaFn("visual_aid_pointer_schema")));
  return JSON.parse(rustJson(body));
}

// Production-shaped: the theory (and its template) carry only the POINTER to
// the visual schema, which is declared in full once, on the diagram variant —
// exactly what the app's tool looks like, so "did it draw?" is measured honestly.
const THEORY_TOOL = {
  type: "function",
  function: {
    name: "publish_notebook_block",
    description: "Publica el SIGUIENTE bloque pedagógico de la clase (uno solo, nunca la clase completa).",
    parameters: {
      type: "object",
      properties: { blockType: { type: "string", enum: ["anchored_micro_theory", "declarative_visual_diagram", "tpl_theory"] } },
      required: ["blockType"],
      oneOf: ["anchored_micro_theory", "declarative_visual_diagram", "tpl_theory"].map(rustBlockVariant),
    },
  },
};
const DIAGRAM_TOOL = {
  type: "function",
  function: {
    name: "publish_notebook_block",
    description: "Publica el SIGUIENTE bloque pedagógico de la clase (uno solo, nunca la clase completa).",
    // The app's own variant (visualIntent included), as one object.
    parameters: { type: "object", ...(rustBlockVariant("declarative_visual_diagram") as object) },
  },
};

// --- scenarios ---------------------------------------------------------------

type Engine = "topology_flow" | "explorable_model" | "solid_model";
interface Scenario {
  /** Theory mode: engines (`engine` or `mermaid:<chartType>`) that count as the right pick. */
  expectAny?: string[];
  /** The course hint the app would send (`visualDirective`). */
  directive?: "Spatial3d" | "Network" | "Quantitative";
  id: string;
  domain: string;
  course: string;
  goal: string;
  milestone: string;
  classTitle: string;
  /** Engines that count as the right choice. `null` = a control: 3D would be the WRONG pick. */
  expect: Engine | null;
  /** For controls: engines that are fine. */
  acceptable?: string[];
  /** Ids/labels the scene should plausibly contain (lower-case substrings, any of each group). */
  mustMention?: string[][];
}

const SCENARIOS: Scenario[] = [
  {
    id: "cad_sketch_extrude",
    domain: "CAD",
    course: "Diseño de piezas con Fusion 360",
    goal: "Modelar y exportar piezas para impresión 3D",
    milestone: "Del croquis al sólido",
    classTitle: "Croquis y extrusión: de un rectángulo a una pieza con agujeros",
    expect: "solid_model",
    directive: "Spatial3d",
    mustMention: [["extru"], ["agujero", "hole"]],
  },
  {
    id: "cad_views",
    domain: "CAD",
    course: "Dibujo técnico y CAD para principiantes",
    goal: "Leer y dibujar planos de piezas",
    milestone: "Vistas",
    classTitle: "Vistas ortogonales: frontal, superior y lateral de una pieza en L",
    expect: "solid_model",
    directive: "Spatial3d",
    mustMention: [["front", "frontal"]],
  },
  {
    id: "tcp_handshake",
    domain: "Redes",
    course: "Redes de computadoras desde cero",
    goal: "Entender qué pasa en la red cuando abro una página web",
    milestone: "Transporte: TCP",
    classTitle: "El saludo de tres vías de TCP y la primera petición HTTP",
    expect: "topology_flow",
    mustMention: [["syn"], ["ack"]],
  },
  {
    id: "oauth_code",
    domain: "Seguridad web",
    course: "Autenticación moderna para desarrolladores",
    goal: "Integrar login con Google en mi app",
    milestone: "OAuth 2.0",
    classTitle: "El flujo Authorization Code entre usuario, app, servidor de autorización y API",
    expect: "topology_flow",
    mustMention: [["code", "código"], ["token"]],
  },
  {
    id: "order_microservices",
    domain: "Sistemas distribuidos",
    course: "Arquitectura de microservicios",
    goal: "Diseñar el backend de una tienda online",
    milestone: "Mensajería asíncrona",
    classTitle: "Un pedido que cruza servicios por una cola y qué pasa cuando el pago falla",
    expect: "topology_flow",
    mustMention: [["pago", "payment"], ["cola", "queue", "kafka", "broker"]],
  },
  {
    id: "dns_recursive",
    domain: "Redes",
    course: "Redes de computadoras desde cero",
    goal: "Entender qué pasa en la red cuando abro una página web",
    milestone: "Capa de aplicación",
    classTitle: "Resolución DNS recursiva: del resolutor a la raíz, el TLD y el autoritativo",
    expect: "topology_flow",
    mustMention: [["raíz", "raiz", "root"], ["tld", ".com"]],
  },
  {
    id: "cell_plant",
    domain: "Biología celular",
    course: "Biología para el examen de admisión",
    goal: "Aprobar la parte de biología del examen",
    milestone: "La célula",
    classTitle: "Los orgánulos de la célula vegetal y qué hace cada uno",
    expect: "explorable_model",
    mustMention: [["cloroplast"], ["núcleo", "nucleo"], ["mitocondri"]],
  },
  {
    id: "atom_bohr",
    domain: "Química",
    course: "Química general",
    goal: "Entender la tabla periódica",
    milestone: "Estructura atómica",
    classTitle: "Dentro de un átomo de carbono: núcleo, protones, neutrones y capas de electrones",
    expect: "explorable_model",
    mustMention: [["protón", "proton"], ["electr"]],
  },
  {
    id: "earth_layers",
    domain: "Geología",
    course: "Ciencias de la Tierra",
    goal: "Explicar terremotos y volcanes a mis alumnos de secundaria",
    milestone: "Estructura interna",
    classTitle: "Las capas de la Tierra: corteza, manto, núcleo externo e interno",
    expect: "explorable_model",
    mustMention: [["manto"], ["corteza"]],
  },
  {
    id: "virus_hiv",
    domain: "Microbiología",
    course: "Inmunología básica",
    goal: "Entender cómo actúan los antirretrovirales",
    milestone: "Virus",
    classTitle: "La estructura del VIH: envoltura, glicoproteínas, cápside y ARN",
    expect: "explorable_model",
    mustMention: [["arn", "rna"], ["cápside", "capside", "capsid"]],
  },
  {
    id: "control_tcp_vs_udp",
    domain: "Redes (control)",
    course: "Redes de computadoras desde cero",
    goal: "Elegir protocolo para un juego online",
    milestone: "Transporte",
    classTitle: "TCP vs UDP: garantías, costo y cuándo elegir cada uno",
    expect: null,
    acceptable: ["conceptual_matrix", "diagram_spec", "mermaid", "topology_flow"],
  },
  {
    id: "control_derivative",
    domain: "Cálculo (control)",
    course: "Cálculo I",
    goal: "Aprobar cálculo diferencial",
    milestone: "Derivadas",
    classTitle: "La derivada como pendiente de la recta tangente",
    expect: null,
    acceptable: ["interactive_module"],
  },
];

/** Theory mode: the class's FIRST block, on topics whose mechanism has a shape. */
const THEORY_SCENARIOS: Scenario[] = [
  {
    id: "sn2_mechanism",
    domain: "Química orgánica",
    course: "Química orgánica I",
    goal: "Predecir productos de reacciones orgánicas",
    milestone: "Sustitución nucleofílica",
    classTitle: "El mecanismo SN2: ataque por detrás e inversión del carbono",
    expect: null,
    expectAny: ["molecule_3d"],
  },
  {
    id: "lagrange",
    domain: "Álgebra abstracta",
    course: "Álgebra abstracta: grupos",
    goal: "Aprobar Álgebra Moderna",
    milestone: "Subgrupos y clases laterales",
    classTitle: "Clases laterales y el teorema de Lagrange en grupos finitos",
    expect: null,
    expectAny: ["algebraic_structure"],
  },
  {
    id: "guitar_changes",
    domain: "Guitarra",
    course: "Guitarra acústica desde cero",
    goal: "Acompañar canciones con acordes abiertos",
    milestone: "Acordes abiertos y cambios",
    classTitle: "Cambiar entre Sol, Do y Re a tiempo con el rasgueo",
    expect: null,
    expectAny: ["instrument"],
  },
  {
    id: "atwood",
    domain: "Física",
    course: "Física I: mecánica",
    goal: "Resolver problemas de dinámica",
    milestone: "Leyes de Newton",
    classTitle: "La máquina de Atwood: tensión y aceleración de dos masas",
    expect: null,
    expectAny: ["illustration_kit", "declarative_svg"],
  },
  {
    id: "gr_curvature",
    domain: "Relatividad general",
    course: "Relatividad general para estudiantes de física",
    goal: "Entender la gravedad como geometría",
    milestone: "Curvatura y geodésicas",
    classTitle: "Curvatura del espacio-tiempo y geodésicas: por qué cae una manzana",
    expect: null,
    expectAny: ["surface_3d"],
    mustMention: [["geodesic"]],
  },
  {
    id: "integral_area",
    domain: "Cálculo",
    course: "Cálculo integral desde cero",
    goal: "Aprobar Cálculo II en la universidad",
    milestone: "La integral definida",
    classTitle: "La integral como área acumulada y el teorema fundamental del cálculo",
    expect: null,
    expectAny: ["interactive_module"],
    mustMention: [["area"]],
  },
  {
    id: "double_integral",
    domain: "Cálculo multivariable",
    course: "Cálculo vectorial",
    goal: "Aprobar Cálculo III",
    milestone: "Integrales dobles",
    classTitle: "La integral doble como volumen bajo una superficie",
    expect: null,
    expectAny: ["surface_3d"],
    mustMention: [["region"]],
  },
  {
    id: "cad_orbit",
    domain: "CAD",
    course: "Diseño de piezas con Fusion 360",
    goal: "Modelar y exportar piezas para impresión 3D",
    milestone: "Moverse en el espacio 3D",
    classTitle: "Orbitar, encuadrar y las vistas estándar: cómo te mueves alrededor de la pieza",
    expect: null,
    expectAny: ["solid_model"],
    directive: "Spatial3d",
    mustMention: [["camera", "showcamera"]],
  },
  {
    id: "cad_extrude",
    domain: "CAD",
    course: "Diseño de piezas con Fusion 360",
    goal: "Modelar y exportar piezas para impresión 3D",
    milestone: "Del croquis al sólido",
    classTitle: "Extruir un croquis: cómo un rectángulo se convierte en un bloque",
    expect: null,
    expectAny: ["solid_model"],
    directive: "Spatial3d",
    mustMention: [["extrude"]],
  },
  {
    id: "cad_revolve",
    domain: "CAD",
    course: "Diseño de piezas con Fusion 360",
    goal: "Modelar y exportar piezas para impresión 3D",
    milestone: "Operaciones de revolución",
    classTitle: "Revolución: un vaso o un eje a partir de medio perfil",
    expect: null,
    expectAny: ["solid_model"],
    directive: "Spatial3d",
    mustMention: [["revolve"]],
  },
  {
    id: "cad_history",
    domain: "CAD (cronología)",
    course: "Diseño de piezas con Fusion 360",
    goal: "Modelar y exportar piezas para impresión 3D",
    milestone: "Contexto",
    classTitle: "Breve historia del CAD: de Sketchpad (1963) a los modeladores en la nube",
    expect: null,
    expectAny: ["mermaid:timeline", "interactive_module"],
    directive: "Spatial3d",
    mustMention: [["1963", "sketchpad"]],
  },
  {
    id: "concept_map",
    domain: "Aprendizaje",
    course: "Técnicas de estudio",
    goal: "Organizar lo que estudio",
    milestone: "Mapas de ideas",
    classTitle: "Las familias de técnicas de estudio y cómo se agrupan",
    expect: null,
    expectAny: ["mermaid:mindmap"],
  },
  {
    id: "project_plan",
    domain: "Gestión",
    course: "Gestión de proyectos para freelancers",
    goal: "Entregar a tiempo",
    milestone: "Planificación",
    classTitle: "Planificar un proyecto de 3 semanas: tareas, dependencias e hitos en el calendario",
    expect: null,
    expectAny: ["mermaid:gantt"],
  },
  {
    id: "risk_matrix",
    domain: "Gestión",
    course: "Gestión de proyectos para freelancers",
    goal: "Entregar a tiempo",
    milestone: "Riesgos",
    classTitle: "Priorizar riesgos según su impacto y su probabilidad",
    expect: null,
    expectAny: ["mermaid:quadrantChart", "conceptual_matrix"],
  },
  {
    id: "dna_codons",
    domain: "Genética",
    course: "Biología para curiosos",
    goal: "Entender qué es una mutación",
    milestone: "El código genético",
    classTitle: "Cómo el ADN guarda instrucciones: letras, codones y proteínas",
    expect: null,
    mustMention: [["atg", "codón", "codon"]],
  },
  {
    id: "call_stack",
    domain: "Programación",
    course: "JavaScript desde cero",
    goal: "Entender la recursión",
    milestone: "Funciones",
    classTitle: "La pila de llamadas cuando una función se llama a sí misma",
    expect: null,
    mustMention: [["pila", "stack"]],
  },
  {
    id: "ohm_law",
    domain: "Física",
    course: "Electricidad básica",
    goal: "Arreglar circuitos sencillos",
    milestone: "Circuitos",
    classTitle: "La ley de Ohm: voltaje, corriente y resistencia",
    expect: null,
    mustMention: [["voltaje", "volt"]],
  },
  {
    id: "tcp_theory",
    domain: "Redes",
    course: "Redes de computadoras desde cero",
    goal: "Entender qué pasa en la red cuando abro una página web",
    milestone: "Transporte: TCP",
    classTitle: "El saludo de tres vías de TCP",
    expect: null,
    mustMention: [["syn"]],
  },
  {
    id: "cell_theory",
    domain: "Biología celular",
    course: "Biología para el examen de admisión",
    goal: "Aprobar la parte de biología del examen",
    milestone: "La célula",
    classTitle: "Qué hay dentro de una célula animal",
    expect: null,
    mustMention: [["núcleo", "nucleo"]],
  },
  {
    id: "supply_demand",
    domain: "Economía",
    course: "Economía para emprendedores",
    goal: "Poner precio a mi producto",
    milestone: "Mercados",
    classTitle: "Oferta y demanda: cómo se forma un precio",
    expect: null,
    mustMention: [["precio"]],
  },
];

function userMessage(sc: Scenario): string {
  const required = THEORY ? "anchored_micro_theory" : "declarative_visual_diagram";
  const context = {
    course: { title: sc.course, targetGoal: sc.goal, totalWeeks: 6 },
    milestone: { weekNumber: 2, title: sc.milestone, deliverable: `Explicar ${sc.classTitle.toLowerCase()}` },
    class: { title: sc.classTitle, classNumber: 1, allocatedHours: 1.5 },
    blocksSoFar: THEORY ? [] : ["anchored_micro_theory"],
    contentSoFar: THEORY ? [] : [{ blockType: "anchored_micro_theory", title: `Intuición: ${sc.classTitle}` }],
    initialPrediction: "(el estudiante aún no ha registrado una predicción inicial en esta clase)",
    finalResultSoFar: "",
    masteryStatus: { hasPassedConceptualGate: false, hasPassedPracticeGate: false, canClose: false },
    ...(THEORY ? {} : { cannotRepeatBlockType: "anchored_micro_theory" }),
    scaffolding: { level: "guided" },
    ...(sc.directive ? { visualDirective: rustDirective(sc.directive) } : {}),
  };
  // Same shape as `BlockPlan::to_hint_json`, appended like `generation.rs` does.
  const plan = {
    requiredBlockType: required,
    reason: "profile_chain",
    guidance: "El sistema ya decidió el tipo de este bloque: escribe SOLO su contenido. No cambies de tipo salvo que sea imposible.",
  };
  return `${JSON.stringify(context)}\n\nPLAN_DEL_SISTEMA: ${JSON.stringify(plan)}`;
}

/** Theory mode: does the block carry a figure, does it render, and does it keep the theory's own rules? */
function judgeTheory(sc: Scenario, raw: string): ReturnType<typeof judge> {
  const base = {
    ok: false, engineOk: false, schemaOk: false, qualityIssues: [] as string[], walkthroughSteps: 0, walkthroughResolved: 0,
    coordinatesLeaked: false, contentHits: 0, contentGroups: sc.mustMention?.length ?? 0, stats: {} as Record<string, number | string>, renderOk: false,
  };
  let block: any;
  try {
    block = parseArgs(raw);
  } catch (e: any) {
    return { ...base, error: `JSON inválido: ${e.message}` };
  }
  const words = (s: unknown) => String(s ?? "").split(/\s+/).filter(Boolean).length;
  const issues: string[] = [];
  const [hook, boundary, error, rule] = [block.intuitiveHook, block.analogyBoundary, block.frequentError, block.systemRule].map(words);
  if (hook + boundary + error > 100) issues.push(`hook+boundary+error = ${hook + boundary + error} palabras (máx 100)`);
  if (rule > 240) issues.push(`systemRule ${rule} palabras (máx 240)`);
  const err = String(block.frequentError ?? "").trim();
  if (words(err) >= 6 && String(block.systemRule ?? "").includes(err)) issues.push("systemRule repite frequentError");
  const stats: Record<string, number | string> = { blockType: block.blockType, ruleWords: rule, otherWords: hook + boundary + error };

  const va = block.visualAid;
  if (!va || typeof va !== "object") {
    return { ...base, engine: "(sin figura)", qualityIssues: [...issues, "sin visualAid"], stats };
  }
  const engine: string = va.renderEngine === "mermaid" ? `mermaid:${va.chartType}` : (va.renderEngine ?? va.type ?? "?");
  // An expectation names the engine, or for Mermaid the chart type.
  const engineOk = !sc.expectAny || sc.expectAny.some((e) => e === engine || e === va.renderEngine);
  stats.intent = String(block.visualIntent ?? "-");
  const parsed = StaticVisualSpecSchema.safeParse(va);
  if (!parsed.success) {
    const issue = parsed.error.issues[0];
    return { ...base, engine, engineOk, qualityIssues: issues, stats, schemaError: `${issue?.path.join(".")}: ${issue?.message}` };
  }
  issues.push(...scoreVisual(parsed.data).issues);
  let renderOk = false;
  try {
    renderOk = renderToStaticMarkup(<StaticVisual visual={parsed.data} />).length > 0;
  } catch (e: any) {
    issues.push(`render HTML falló: ${e.message}`);
  }
  const text = JSON.stringify(va).toLowerCase();
  const contentHits = (sc.mustMention ?? []).filter((group) => group.some((w) => text.includes(w))).length;
  const coordinatesLeaked = (engine === "topology_flow" || engine === "explorable_model") && leaksCoordinates(va);
  return {
    ...base, ok: engineOk && issues.length === 0 && renderOk && !coordinatesLeaked, engine, engineOk, schemaOk: true,
    qualityIssues: issues, coordinatesLeaked, contentHits, stats, renderOk,
  };
}

// --- judging -----------------------------------------------------------------

interface Judged {
  model: string;
  scenario: string;
  sample: number;
  ok: boolean;
  error?: string;
  nativeToolCall: boolean;
  engine?: string;
  engineOk: boolean;
  schemaOk: boolean;
  schemaError?: string;
  qualityIssues: string[];
  walkthroughSteps: number;
  walkthroughResolved: number;
  coordinatesLeaked: boolean;
  contentHits: number;
  contentGroups: number;
  stats: Record<string, number | string>;
  renderOk: boolean;
  latencyMs: number;
  promptTokens: number;
  completionTokens: number;
  raw: string;
  /** Same judgement after the deterministic repair the app applies (`repairLikeApp`). */
  repairedOk?: boolean;
  repairedIssue?: string;
}

// --- mirror of the app's deterministic repair ----------------------------------
// The app repairs every generated visual BEFORE grounding (`domain::
// visual_repair::repair_block`). These are the 3D-specific rules, mirrored so
// the eval can report what the student actually gets, not only the raw output.
// Keep in sync with visual_repair.rs (repair_model / repair_walkthrough).

const fold = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^a-z0-9ñ]/g, "")
    .replace(/ñ/g, "n");

function repairLikeApp(input: any): any {
  const block = structuredClone(input);
  const va = block?.visualAid;
  if (!va || typeof va !== "object") return block;
  // Mermaid: the code's header wins over a mistyped chartType (`normalize_visual_aid_object` + `repair_visual`).
  if (va.renderEngine === "mermaid") {
    const declared = mermaidTypeOf(String(va.code ?? ""));
    if (declared) va.chartType = declared;
  }
  // solid_model: a revolve has no depth; a cut/hole that only bites when drilled
  // the other way is turned around (`repair_solid` / `repair_cut_direction`).
  if (va.renderEngine === "solid_model" && Array.isArray(va.features)) {
    for (const f of va.features) if (f.op === "revolve") f.depth = null;
    va.features.forEach((f: any, i: number) => {
      if (f.op === "extrude" || f.op === "revolve" || !(f.depth > 0)) return;
      const material = va.features.slice(0, i).filter((p: any) => p.op === "extrude" || p.op === "revolve").map((p: any) => featureBox({ offset: 0, at: [0, 0], ...p, profile: { points: [], ...p.profile } }, 4000)).filter(Boolean);
      const bites = (g: any) => {
        const b = featureBox({ offset: 0, at: [0, 0], ...g, profile: { points: [], ...g.profile } }, 4000);
        return !!b && material.some((m: any) => [0, 1, 2].every((k) => b.min[k] < m.max[k] - 1e-6 && m.min[k] < b.max[k] - 1e-6));
      };
      if (!bites(f) && bites({ ...f, offset: (f.offset ?? 0) + f.depth })) f.offset = (f.offset ?? 0) + f.depth;
    });
  }
  if (va.renderEngine === "explorable_model" && Array.isArray(va.parts)) {
    const centre = (p: any) => String(p.zone ?? "").trim().toLowerCase() === "center";
    let keep = va.parts.findIndex((p: any) => centre(p) && String(p.shape).toLowerCase() === "layer");
    if (keep < 0) keep = va.parts.findIndex(centre);
    va.parts.forEach((p: any, i: number) => {
      if (centre(p) && i !== keep) p.zone = "inner";
      if (String(p.shape).toLowerCase() === "layer") p.count = 1;
    });
  }
  const targets: Array<[string, string]> = [];
  const aliases: Array<[string, string]> = [];
  if (va.renderEngine === "topology_flow") {
    for (const n of va.nodes ?? []) targets.push([n.id, n.label]);
    for (const s of va.steps ?? []) {
      if (s.messages?.[0]) aliases.push([fold(s.title ?? ""), s.messages[0].to]);
      for (const m of s.messages ?? []) aliases.push([fold(m.label ?? ""), m.to]);
    }
  } else if (va.renderEngine === "explorable_model") {
    for (const p of va.parts ?? []) targets.push([p.id, p.label]);
    if (va.shell) for (const a of ["shell", "envoltura", "envelope", va.shell.label ?? ""]) aliases.push([fold(a), "shell"]);
  } else if (va.renderEngine === "diagram_spec") {
    for (const n of va.nodes ?? []) targets.push([n.id, n.label]);
  } else return block;
  const hasShell = aliases.some(([, t]) => t === "shell");
  const resolve = (raw: string): string | null => {
    if (targets.some(([id]) => id === raw) || (raw === "shell" && hasShell)) return null;
    const wanted = fold(raw);
    if (!wanted) return null;
    const exact = targets.find(([id, label]) => fold(id) === wanted || fold(label) === wanted);
    if (exact) return exact[0];
    const alias = aliases.find(([a]) => a === wanted);
    if (alias) return alias[1];
    const hits = targets.filter(([id, label]) => [fold(id), fold(label)].some((t) => t.length >= 3 && (wanted.includes(t) || (wanted.length >= 3 && t.includes(wanted)))));
    return hits.length === 1 ? hits[0][0] : null;
  };
  for (const step of Array.isArray(block.guidedWalkthrough) ? block.guidedWalkthrough : []) {
    const id = resolve(String(step?.targetVisualElement ?? ""));
    if (id) step.targetVisualElement = id;
  }
  return block;
}

const COORD_KEYS = new Set(["x", "y", "z", "position", "positions", "coords", "coordinates", "rotation", "scale", "width", "height", "radius"]);
function leaksCoordinates(v: unknown): boolean {
  if (Array.isArray(v)) return v.some(leaksCoordinates);
  if (v && typeof v === "object") return Object.entries(v).some(([k, x]) => COORD_KEYS.has(k) || leaksCoordinates(x));
  return false;
}

/** The verdict on what the student actually gets: the block after the app's repair. */
function repairedVerdict(sc: Scenario, raw: string): Pick<Judged, "repairedOk" | "repairedIssue"> {
  let block: any;
  try {
    block = parseArgs(raw);
  } catch {
    return { repairedOk: false, repairedIssue: "JSON inválido" };
  }
  const fixed = JSON.stringify(repairLikeApp(block));
  const r = THEORY ? judgeTheory(sc, fixed) : judge(sc, fixed);
  const issue = r.error ?? r.schemaError ?? (!r.engineOk ? `motor ${r.engine}` : r.qualityIssues[0] ?? (r.walkthroughResolved < r.walkthroughSteps ? "walkthrough → id inexistente" : undefined));
  return { repairedOk: r.ok, repairedIssue: issue };
}

function parseArgs(raw: string): any {
  let s = raw.trim().replace(/^```(?:json)?\s*/i, "").replace(/\s*```$/, "");
  const first = s.indexOf("{");
  if (first > 0) s = s.slice(first);
  const v = JSON.parse(s);
  // Some providers stringify nested objects; the Rust side unwraps them too.
  for (const k of ["visualAid", "guidedWalkthrough"]) if (typeof v[k] === "string") v[k] = JSON.parse(v[k]);
  return v;
}

function judge(sc: Scenario, raw: string): Omit<Judged, "model" | "scenario" | "sample" | "nativeToolCall" | "latencyMs" | "promptTokens" | "completionTokens" | "raw"> {
  const base = {
    ok: false, engineOk: false, schemaOk: false, qualityIssues: [] as string[], walkthroughSteps: 0, walkthroughResolved: 0,
    coordinatesLeaked: false, contentHits: 0, contentGroups: sc.mustMention?.length ?? 0, stats: {} as Record<string, number | string>, renderOk: false,
  };
  let block: any;
  try {
    block = parseArgs(raw);
  } catch (e: any) {
    return { ...base, error: `JSON inválido: ${e.message}` };
  }
  const va = block?.visualAid;
  if (!va || typeof va !== "object") return { ...base, error: "sin visualAid" };
  const engine: string = va.renderEngine ?? va.type ?? "?";
  const engineOk = sc.expect ? engine === sc.expect : (sc.acceptable ?? []).includes(engine);
  const is3d = engine === "topology_flow" || engine === "explorable_model" || engine === "solid_model";
  // A solid_model's numbers are its measurements, not layout coordinates.
  const coordinatesLeaked = is3d && engine !== "solid_model" && leaksCoordinates(va);

  const parsed = StaticVisualSpecSchema.safeParse(va);
  if (!parsed.success) {
    const issue = parsed.error.issues[0];
    return { ...base, engine, engineOk, coordinatesLeaked, schemaError: `${issue?.path.join(".")}: ${issue?.message}` };
  }
  const visual = parsed.data;
  const qualityIssues = scoreVisual(visual).issues;

  const ids = new Set<string>();
  const stats: Record<string, number | string> = {};
  if (visual.renderEngine === "topology_flow") {
    visual.nodes.forEach((n) => ids.add(n.id));
    const msgs = visual.steps.flatMap((s) => s.messages);
    const hops = msgs.map((m) => (routeMessage(visual.nodes, visual.links, m.from, m.to)?.length ?? 0) - 1);
    stats.nodes = visual.nodes.length;
    stats.links = visual.links.length;
    stats.steps = visual.steps.length;
    stats.messages = msgs.length;
    stats.emptySteps = visual.steps.filter((s) => !s.messages.length).length;
    stats.multiHopMessages = hops.filter((h) => h > 1).length;
    stats.tones = [...new Set(msgs.map((m) => m.tone ?? "request"))].join("/");
    stats.icons = [...new Set(visual.nodes.map((n) => n.icon))].length;
    stats.withExplanation = visual.nodes.filter((n) => n.explanation?.trim()).length;
    // Isolated nodes: scoreVisual doesn't flag them, but the prompt forbids them.
    const linked = new Set(visual.links.flatMap((l) => [l.from, l.to]));
    const isolated = visual.nodes.filter((n) => !linked.has(n.id)).length;
    if (isolated) qualityIssues.push(`${isolated} nodo(s) sin ningún link`);
    try {
      layoutTopology(visual.nodes, visual.links);
    } catch (e: any) {
      qualityIssues.push(`layoutTopology falló: ${e.message}`);
    }
  } else if (visual.renderEngine === "explorable_model") {
    visual.parts.forEach((p) => ids.add(p.id));
    if (visual.shell) ids.add("shell"); // the envelope is a valid target (SHELL_TARGET)
    stats.parts = visual.parts.length;
    stats.layers = visual.parts.filter((p) => p.shape === "layer").length;
    stats.shell = visual.shell ? `${visual.shell.shape}${visual.shell.cutaway ? "+cut" : ""}` : "none";
    stats.distinctShapes = new Set(visual.parts.map((p) => p.shape)).size;
    stats.distinctColors = new Set(visual.parts.map((p) => p.color ?? "")).size;
    stats.instances = visual.parts.reduce((a, p) => a + (p.count ?? 1), 0);
    try {
      stats.overlap = Number(overlapDepth(placeParts(visual.parts, visual.shell, visual.shell?.cutaway ?? false)).toFixed(3));
    } catch (e: any) {
      qualityIssues.push(`placeParts falló: ${e.message}`);
    }
    if (stats.distinctColors < Math.min(visual.parts.length, 6)) qualityIssues.push("colores repetidos entre partes");
  } else if (visual.renderEngine === "solid_model") {
    visual.features.forEach((f) => ids.add(f.id));
    ["iso", "front", "top", "right", "camera"].forEach((v) => ids.add(v));
    stats.features = visual.features.length;
    stats.ops = visual.features.map((f) => f.op).join("/");
    stats.showCamera = String(visual.showCamera);
  }
  stats.intent = String(block.visualIntent ?? "-");

  const walk: any[] = Array.isArray(block.guidedWalkthrough) ? block.guidedWalkthrough : [];
  const walkthroughResolved = is3d ? walk.filter((w) => ids.has(String(w?.targetVisualElement ?? ""))).length : walk.length;
  if (is3d && walk.length < 2) qualityIssues.push(`guidedWalkthrough con ${walk.length} pasos (se piden 2-4)`);

  const text = JSON.stringify(va).toLowerCase();
  const contentHits = (sc.mustMention ?? []).filter((group) => group.some((w) => text.includes(w))).length;

  let renderOk = false;
  try {
    renderOk = renderToStaticMarkup(<StaticVisual visual={visual} />).length > 0;
  } catch (e: any) {
    qualityIssues.push(`render HTML falló: ${e.message}`);
  }

  const ok = engineOk && qualityIssues.length === 0 && !coordinatesLeaked && walkthroughResolved === walk.length && renderOk;
  return { ...base, ok, engine, engineOk, schemaOk: true, qualityIssues, walkthroughSteps: walk.length, walkthroughResolved, coordinatesLeaked, contentHits, stats, renderOk };
}

// --- running -----------------------------------------------------------------

async function runOne(p: Provider, sc: Scenario, sample: number): Promise<Judged> {
  const start = Date.now();
  const head = { model: p.label, scenario: sc.id, sample, course: sc.course, goal: sc.goal, milestone: sc.milestone, classTitle: sc.classTitle };
  try {
    const res = await fetch(p.url, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${p.key}` },
      body: JSON.stringify({
        model: p.model,
        messages: [
          { role: "system", content: SYSTEM_PROMPT },
          { role: "user", content: userMessage(sc) },
        ],
        tools: [THEORY ? THEORY_TOOL : DIAGRAM_TOOL],
        tool_choice: { type: "function", function: { name: "publish_notebook_block" } },
        temperature: 0.2,
        ...p.extraBody,
      }),
      signal: AbortSignal.timeout(180_000),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}: ${(await res.text()).slice(0, 300)}`);
    // `fetch` resolves on the headers: time the whole body, not the first byte.
    const data: any = await res.json();
    const latencyMs = Date.now() - start;
    const msg = data.choices?.[0]?.message;
    const call = msg?.tool_calls?.[0]?.function?.arguments;
    const raw: string = call ?? msg?.content ?? "";
    return {
      ...head,
      nativeToolCall: Boolean(call),
      latencyMs,
      promptTokens: data.usage?.prompt_tokens ?? 0,
      completionTokens: data.usage?.completion_tokens ?? 0,
      raw,
      ...(THEORY ? judgeTheory(sc, raw) : judge(sc, raw)),
      ...repairedVerdict(sc, raw),
    };
  } catch (e: any) {
    return {
      ...head, ok: false, error: e.message, nativeToolCall: false, engineOk: false, schemaOk: false, qualityIssues: [], walkthroughSteps: 0,
      walkthroughResolved: 0, coordinatesLeaked: false, contentHits: 0, contentGroups: sc.mustMention?.length ?? 0, stats: {}, renderOk: false,
      latencyMs: Date.now() - start, promptTokens: 0, completionTokens: 0, raw: "",
    };
  }
}

async function pool<T>(tasks: (() => Promise<T>)[], n: number): Promise<T[]> {
  const out: T[] = new Array(tasks.length);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(n, tasks.length) }, async () => {
    while (next < tasks.length) {
      const i = next++;
      out[i] = await tasks[i]();
    }
  }));
  return out;
}

const pct = (a: number, b: number) => (b ? `${Math.round((100 * a) / b)}%` : "—");
const median = (xs: number[]) => {
  const s = [...xs].sort((a, b) => a - b);
  return s.length ? s[Math.floor(s.length / 2)] : 0;
};

async function main() {
  const filter = arg("scenario");
  const pool0 = THEORY ? THEORY_SCENARIOS : SCENARIOS;
  const wanted = filter?.split(",").map((f) => f.trim());
  const scenarios = wanted ? pool0.filter((s) => wanted.includes(s.id) || wanted.includes(String(s.expect))) : pool0;
  const providers = requestedModels.map((m) => ({ m, p: provider(m) }));
  for (const { m, p } of providers) if (!p) console.warn(`⚠ sin API key para ${m} — se omite`);
  const active = providers.flatMap(({ p }) => (p ? [p] : []));

  console.log(`Bloque: ${THEORY ? "anchored_micro_theory" : "declarative_visual_diagram"} · system prompt: ${SYSTEM_PROMPT.length} chars · tool schema: ${JSON.stringify(THEORY ? THEORY_TOOL : DIAGRAM_TOOL).length} chars`);
  console.log(`Modelos: ${active.map((p) => p.label).join(", ") || "(ninguno)"} · escenarios: ${scenarios.length} · repeticiones: ${REPEAT}`);
  const replay = arg("replay");
  if (replay) {
    // Re-judges a saved report's raw outputs with the CURRENT judge and repair:
    // free (no API calls), so judge/repair changes can be checked first.
    const saved: Judged[] = JSON.parse(readFileSync(resolve(ROOT, replay), "utf-8")).results;
    const all = [...SCENARIOS, ...THEORY_SCENARIOS];
    let before = 0;
    let after = 0;
    for (const r of saved) {
      const sc = all.find((s) => s.id === r.scenario);
      if (!sc || !r.raw) continue;
      const now = { ...(THEORY ? judgeTheory(sc, r.raw) : judge(sc, r.raw)), ...repairedVerdict(sc, r.raw) };
      before += Number(now.ok);
      after += Number(now.repairedOk);
      if (!now.repairedOk) console.log(`❌ ${r.model.padEnd(16)} ${r.scenario.padEnd(22)} ${now.repairedIssue ?? ""}`);
    }
    console.log(`\n${saved.length} salidas: ${before} aprobadas crudas → ${after} tras la reparación`);
    return;
  }
  if (argv.includes("--self-test")) {
    // The gallery's hand-written 3D blocks are the reference for "good": if
    // the judge fails them, the judge is wrong, not the models.
    const presets = BLOCK_CATEGORIES.find((c) => c.id === "scene_3d")?.presets ?? [];
    let bad = 0;
    for (const preset of presets) {
      const block = preset.block.content_json as any;
      const sc = SCENARIOS.find((s) => s.expect === block.visualAid?.renderEngine)!;
      const r = judge({ ...sc, mustMention: [] }, JSON.stringify(block));
      if (!r.ok) bad++;
      console.log(`${r.ok ? "✅" : "❌"} ${preset.id} ${r.engine} ${JSON.stringify(r.stats)} ${[r.error, r.schemaError, ...r.qualityIssues].filter(Boolean).join("; ")}`);
    }
    if (!presets.length || bad) process.exit(1);
    return;
  }
  if (DRY_RUN) {
    console.log("\n--dry-run: prompt y schema reconstruidos desde Rust sin errores.");
    console.log(userMessage(scenarios[0]));
    return;
  }
  if (!active.length) throw new Error("ningún modelo disponible: define MIMO_API_KEY y/o DEEPSEEK_API_KEY en .env");

  const tasks = active.flatMap((p) => scenarios.flatMap((sc) => Array.from({ length: REPEAT }, (_, k) => () =>
    runOne(p, sc, k).then((r) => {
      const mark = r.ok ? "✅" : r.error ? "💥" : r.repairedOk ? "🔧" : "❌";
      const why = r.error ?? r.schemaError ?? (!r.engineOk ? `motor ${r.engine}` : r.qualityIssues[0] ?? (r.walkthroughResolved < r.walkthroughSteps ? "walkthrough → id inexistente" : r.coordinatesLeaked ? "coordenadas" : ""));
      const after = !r.ok && !r.error ? (r.repairedOk ? " → reparado" : ` → tras reparar: ${r.repairedIssue ?? "?"}`) : "";
      console.log(`${mark} ${p.label.padEnd(16)} ${sc.id.padEnd(22)} ${String(r.engine ?? "-").padEnd(17)} ${(r.latencyMs / 1000).toFixed(1).padStart(5)}s ${why}${after}`);
      return r;
    }),
  )));
  const results = await pool(tasks, CONCURRENCY * active.length);

  console.log("\n=== Resumen por modelo ===");
  console.log("modelo           crudo  reparado  motor  schema  calidad  walkthr  coord  contenido  p50 lat  tok out");
  for (const p of active) {
    const rs = results.filter((r) => r.model === p.label);
    const valid = rs.filter((r) => r.schemaOk);
    const walkTotal = valid.reduce((a, r) => a + r.walkthroughSteps, 0);
    const walkOk = valid.reduce((a, r) => a + r.walkthroughResolved, 0);
    const groups = rs.reduce((a, r) => a + r.contentGroups, 0);
    const hits = rs.reduce((a, r) => a + r.contentHits, 0);
    console.log(
      `${p.label.padEnd(16)} ${pct(rs.filter((r) => r.ok).length, rs.length).padStart(5)}  ${pct(rs.filter((r) => r.repairedOk).length, rs.length).padStart(8)}  ${pct(rs.filter((r) => r.engineOk).length, rs.length).padStart(5)}  ${pct(valid.length, rs.length).padStart(6)}  ${pct(valid.filter((r) => !r.qualityIssues.length).length, valid.length).padStart(7)}  ${pct(walkOk, walkTotal).padStart(7)}  ${String(rs.filter((r) => r.coordinatesLeaked).length).padStart(5)}  ${pct(hits, groups).padStart(9)}  ${(median(rs.map((r) => r.latencyMs)) / 1000).toFixed(1).padStart(6)}s  ${String(median(rs.map((r) => r.completionTokens))).padStart(7)}`,
    );
  }

  const issues = new Map<string, number>();
  for (const r of results) for (const i of [...r.qualityIssues, ...(r.schemaError ? [`schema: ${r.schemaError}`] : [])]) {
    const key = i.replace(/"[^"]*"/g, '"…"').replace(/\d+/g, "N");
    issues.set(key, (issues.get(key) ?? 0) + 1);
  }
  if (issues.size) {
    console.log("\n=== Fallos más frecuentes ===");
    [...issues.entries()].sort((a, b) => b[1] - a[1]).slice(0, 12).forEach(([k, n]) => console.log(`${String(n).padStart(3)} × ${k}`));
  }

  mkdirSync(resolve(ROOT, "eval-results"), { recursive: true });
  const out = resolve(ROOT, `eval-results/scene3d-${new Date().toISOString().replace(/[:.]/g, "-")}.json`);
  writeFileSync(out, JSON.stringify({ block: THEORY ? "theory" : "diagram", at: new Date().toISOString(), promptChars: SYSTEM_PROMPT.length, results }, null, 2));
  console.log(`\nInforme completo: ${out}`);
}

await main();
