/**
 * LearnKit Real-LLM Block Generation Evaluation Suite (v3: Advanced Forms & Latency Benchmark)
 *
 * Runs real inference calls using keys from .env to systematically evaluate:
 * 1. Complex scenarios across diverse domains (Distributed Systems, Concurrency, Low-Level Systems,
 *    Compilers, Database Internals, Frontend Architecture).
 * 2. Specialized pedagogical forms (causal_chain_order, boundary_case_audit, transfer challenges,
 *    Mermaid diagrams with guided walkthroughs, and mechanical templates).
 * 3. Latency & Token Optimization experiments (Baseline vs Output Capping vs Mechanical Templates).
 *
 * Usage:
 *   bun scripts/eval-block-generation.ts
 *   bun scripts/eval-block-generation.ts --model=mimo-v2.6-pro
 *   bun scripts/eval-block-generation.ts --model=mimo-v2.6-flash
 *   bun scripts/eval-block-generation.ts --model=deepseek-chat
 *   bun scripts/eval-block-generation.ts --benchmark-latency
 */

import { readFileSync, writeFileSync } from "fs";
import { resolve } from "path";

// 1. Read API keys from .env
function loadEnv(): Record<string, string> {
  const env: Record<string, string> = { ...(process.env as Record<string, string>) };
  try {
    const envPath = resolve(process.cwd(), ".env");
    const content = readFileSync(envPath, "utf-8");
    for (const line of content.split("\n")) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith("#")) continue;
      const eqIdx = trimmed.indexOf("=");
      if (eqIdx > 0) {
        const key = trimmed.slice(0, eqIdx).trim();
        let val = trimmed.slice(eqIdx + 1).trim();
        if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
          val = val.slice(1, -1);
        }
        env[key] = val;
      }
    }
  } catch {}
  return env;
}

const env = loadEnv();
const MIMO_API_KEY = env.MIMO_API_KEY || env.XIAOMI_MIMO_API_KEY || env.XIAOMI_API_KEY;
const DEEPSEEK_API_KEY = env.DEEPSEEK_API_KEY;

// 2. Parse CLI arguments
const args = process.argv.slice(2);
function getArg(prefix: string, defaultVal?: string): string | undefined {
  const found = args.find((a) => a.startsWith(prefix));
  return found ? found.slice(prefix.length) : defaultVal;
}

const requestedModel = getArg("--model=") || (MIMO_API_KEY ? "mimo-v2.6-pro" : "deepseek-chat");
const requestedScenario = getArg("--scenario=");
const isBenchmarkLatency = args.includes("--benchmark-latency");

interface ProviderConfig {
  name: string;
  baseUrl: string;
  apiKey: string;
  model: string;
  extraBody?: Record<string, any>;
}

function resolveProvider(modelName: string): ProviderConfig {
  if (modelName.includes("mimo") || modelName.includes("xiaomi")) {
    if (!MIMO_API_KEY) {
      throw new Error("MIMO_API_KEY no encontrada en .env");
    }
    return {
      name: "Xiaomi",
      baseUrl: "https://api.xiaomimimo.com/v1/chat/completions",
      apiKey: MIMO_API_KEY,
      model: modelName.includes("/") ? modelName.split("/")[1] : modelName,
      extraBody: {
        thinking: { type: "disabled" },
      },
    };
  } else {
    if (!DEEPSEEK_API_KEY) {
      throw new Error("DEEPSEEK_API_KEY no encontrada en .env");
    }
    return {
      name: "DeepSeek",
      baseUrl: "https://api.deepseek.com/chat/completions",
      apiKey: DEEPSEEK_API_KEY,
      model: modelName.includes("/") ? modelName.split("/")[1] : modelName,
    };
  }
}

// 3. Exact schema for `publish_notebook_block` tool (covering all dynamic blocks and templates)
const PUBLISH_BLOCK_TOOL = {
  type: "function",
  function: {
    name: "publish_notebook_block",
    description: "Publica el SIGUIENTE bloque pedagógico de la clase (uno solo, nunca la clase completa).",
    parameters: {
      type: "object",
      oneOf: [
        {
          properties: {
            blockType: { const: "anchored_micro_theory" },
            title: { type: "string" },
            intuitiveHook: { type: "string" },
            analogyBoundary: { type: "string" },
            systemRule: { type: "string" },
            frequentError: { type: "string" },
          },
          required: ["blockType", "title", "intuitiveHook", "systemRule", "frequentError"],
        },
        {
          properties: {
            blockType: { const: "interactive_prediction_gate" },
            question: { type: "string" },
            interactionMode: { type: "string", enum: ["multiple_choice", "causal_chain_order", "boundary_case_audit"] },
            options: { type: "array", items: { type: "string" }, minItems: 2, maxItems: 4 },
            correctOption: { type: "string" },
            conceptualFeedbackMap: { type: "object", additionalProperties: { type: "string" } },
            shuffledSteps: { type: "array", items: { type: "string" } },
            correctOrder: { type: "array", items: { type: "string" } },
            boundaryCases: {
              type: "array",
              items: {
                type: "object",
                properties: {
                  statement: { type: "string" },
                  followsRule: { type: "boolean" },
                },
                required: ["statement", "followsRule"],
              },
            },
          },
          required: ["blockType", "question"],
        },
        {
          properties: {
            blockType: { const: "branching_scenario_challenge" },
            scenario: { type: "string" },
            decisionPoint: { type: "string" },
            branches: {
              type: "array",
              minItems: 2,
              maxItems: 4,
              items: {
                type: "object",
                properties: {
                  choice: { type: "string" },
                  consequence: { type: "string" },
                  isOptimal: { type: "boolean" },
                },
                required: ["choice", "consequence", "isOptimal"],
              },
            },
          },
          required: ["blockType", "scenario", "decisionPoint", "branches"],
        },
        {
          properties: {
            blockType: { const: "declarative_visual_diagram" },
            title: { type: "string" },
            visualAid: {
              type: "object",
              properties: {
                type: { const: "mermaid" },
                chartType: { type: "string", enum: ["flowchart", "sequenceDiagram", "classDiagram", "stateDiagram", "erDiagram"] },
                code: { type: "string" },
                caption: { type: "string" },
              },
              required: ["type", "chartType", "code", "caption"],
            },
            guidedWalkthrough: {
              type: "array",
              items: {
                type: "object",
                properties: {
                  stepNumber: { type: "number" },
                  targetVisualElement: { type: "string" },
                  pedagogicalInsight: { type: "string" },
                },
                required: ["stepNumber", "targetVisualElement", "pedagogicalInsight"],
              },
            },
          },
          required: ["blockType", "title", "visualAid", "guidedWalkthrough"],
        },
        {
          properties: {
            blockType: { const: "hands_on_mission" },
            challengeStatement: { type: "string" },
            expectedMilestoneArtifact: { type: "string" },
            constraints: { type: "array", items: { type: "string" } },
            scaffoldingHints: { type: "array", items: { type: "string" } },
            evaluationRubricSummary: { type: "array", items: { type: "string" } },
            supportLevel: { type: "string", enum: ["full", "guided", "faded", "independent"] },
            workedTemplate: { type: "string" },
            isTransfer: { type: "boolean" },
          },
          required: [
            "blockType",
            "challengeStatement",
            "expectedMilestoneArtifact",
            "constraints",
            "scaffoldingHints",
            "evaluationRubricSummary",
            "supportLevel",
          ],
        },
        {
          properties: {
            blockType: { const: "heuristic_error_audit" },
            instruction: { type: "string" },
            flawedRepresentation: {
              type: "object",
              properties: {
                context: { type: "string" },
                buggySnippetOrDiagram: { type: "string" },
                errorType: { type: "string", enum: ["syntax", "conceptual_misunderstanding", "structural_inversion"] },
              },
              required: ["context", "buggySnippetOrDiagram", "errorType"],
            },
            guidingQuestions: { type: "array", items: { type: "string" } },
            modelSolution: { type: "string" },
            faultyLineNumber: { type: "number" },
          },
          required: ["blockType", "instruction", "flawedRepresentation", "guidingQuestions", "modelSolution"],
        },
        {
          properties: {
            blockType: { const: "metacognitive_closure" },
            synthesisTask: { type: "string" },
            predictionComparison: {
              type: "object",
              properties: {
                initialPrediction: { type: "string" },
                finalResult: { type: "string" },
                contrastNarrative: { type: "string" },
              },
              required: ["initialPrediction", "finalResult", "contrastNarrative"],
            },
            selfEvaluationChecklist: { type: "array", items: { type: "string" } },
          },
          required: ["blockType", "synthesisTask", "predictionComparison", "selfEvaluationChecklist"],
        },
        {
          properties: {
            blockType: { const: "tpl_scenario_choice" },
            scenario: { type: "string" },
            decisionPoint: { type: "string" },
            optimal: {
              type: "object",
              properties: { choice: { type: "string" }, consequence: { type: "string" } },
              required: ["choice", "consequence"],
            },
            alternatives: {
              type: "array",
              minItems: 1,
              items: {
                type: "object",
                properties: { choice: { type: "string" }, consequence: { type: "string" } },
                required: ["choice", "consequence"],
              },
            },
          },
          required: ["blockType", "scenario", "decisionPoint", "optimal", "alternatives"],
        },
      ],
    },
  },
};

// 4. Complex Scenarios across diverse domains and block formats
interface Scenario {
  id: string;
  name: string;
  domain: string;
  targetBlockType: string;
  systemPrompt: string;
  userPrompt: string;
}

const COMPLEX_SCENARIOS: Scenario[] = [
  {
    id: "branching_arch_tradeoff",
    name: "Branching Scenario — Migración de Arquitectura bajo Pico de Tráfico",
    domain: "Sistemas Distribuidos",
    targetBlockType: "branching_scenario_challenge",
    systemPrompt: `Eres el diseñador de retos arquitectónicos de LearnKit.
Emite ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Genera un 'branching_scenario_challenge' con:
- scenario: un monolito con Postgres en 100% CPU por un pico masivo inesperado
- decisionPoint: qué patrón arquitectónico aplicar de inmediato para mitigar el incidente
- branches: 3 opciones concretas (Read-Replicas con consistencia eventual, Cache Redis Write-Through, o Cola Kafka asíncrona). Exactamente una debe ser isOptimal: true con su consecuencia detallada.
Usa objetos JSON nativos.`,
    userPrompt: `Crea el desafío situacional para ingenieros senior evaluando trade-offs entre consistencia y latencia en mitigación de incidentes.`,
  },
  {
    id: "visual_raft_consensus",
    name: "Declarative Visual Diagram — Transición de Estados en Consenso Raft",
    domain: "Algoritmos Distribuidos",
    targetBlockType: "declarative_visual_diagram",
    systemPrompt: `Eres el diseñador visual de LearnKit.
Emite ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Genera un 'declarative_visual_diagram' con:
- title: Máquina de Estados del Protocolo Raft
- visualAid: { type: "mermaid", chartType: "stateDiagram", code: "...", caption: "..." }
  El código Mermaid DEBE ser syntácticamente impecable para stateDiagram (Follower, Candidate, Leader).
- guidedWalkthrough: 3 pasos numerados explicando cada transición con targetVisualElement y pedagogicalInsight.`,
    userPrompt: `Genera el diagrama visual interactivo que modela la transición de roles en un clúster Raft cuando el líder cae.`,
  },
  {
    id: "gate_causal_chain_wal",
    name: "Prediction Gate (Causal Chain) — Secuencia de Recuperación WAL en DB",
    domain: "Bases de Datos Internals",
    targetBlockType: "interactive_prediction_gate",
    systemPrompt: `Eres el generador pedagógico de LearnKit.
Emite ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Genera un 'interactive_prediction_gate' en modo CAUSAL CHAIN:
- interactionMode: "causal_chain_order"
- question: ¿Cuál es el orden estricto de recuperación tras un crash inesperado del motor de BD?
- shuffledSteps: 4 eslabones mezclados (Redo Log, Checkpoint, Undo de transacciones activas, Abrir conexiones)
- correctOrder: los MISMOS 4 eslabones en orden cronológico causal idénticos textualmente.
Sin options ni correctOption en este modo.`,
    userPrompt: `Diseña una compuerta causal para que el alumno ordene el proceso de recuperación ARIES / WAL.`,
  },
  {
    id: "gate_boundary_ieee754",
    name: "Prediction Gate (Boundary Cases) — Casos Límite en Aritmética Flotante",
    domain: "Computación Numérica",
    targetBlockType: "interactive_prediction_gate",
    systemPrompt: `Eres el generador pedagógico de LearnKit.
Emite ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Genera un 'interactive_prediction_gate' en modo BOUNDARY CASES:
- interactionMode: "boundary_case_audit"
- question: Determina si cada afirmación sobre IEEE 754 es verdadera o falsa según la norma
- boundaryCases: exactamente 3 casos concretos con { statement, followsRule: boolean }
  (ejemplo: NaN === NaN, 0.1 + 0.2 === 0.3, +0 === -0). Al menos uno debe ser true y uno false.`,
    userPrompt: `Genera la compuerta de casos límite sobre aritmética flotante de precisión doble.`,
  },
  {
    id: "audit_deadlock_inversion",
    name: "Heuristic Error Audit — Inversión de Bloqueos (Deadlock Concurrente)",
    domain: "Concurrencia & Sistemas Operativos",
    targetBlockType: "heuristic_error_audit",
    systemPrompt: `Eres el generador pedagógico de LearnKit.
Emite ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Genera un 'heuristic_error_audit':
- instruction: Analiza la función de transferencia de fondos bancarios e identifica el riesgo de interbloqueo
- flawedRepresentation: { context, buggySnippetOrDiagram, errorType: "structural_inversion" }
  El snippet de código debe tener 10-15 líneas en Go o Rust mostrando dos hilos bloqueando cuentaA y cuentaB en orden inverso.
- faultyLineNumber: línea 1-based exacta donde se invierte el lock
- guidingQuestions: 2 preguntas dirigidas al orden jerárquico de adquisición de recursos
- modelSolution: la corrección usando ordenamiento por ID de cuenta.`,
    userPrompt: `Diseña el ejercicio de cacería de bugs en transferencias bancarias concurrentes.`,
  },
  {
    id: "mission_zerocopy_transfer",
    name: "Hands-on Mission (Transfer) — Parser Zero-Copy de Paquetes de Red",
    domain: "Ingeniería de Redes & Sistemas",
    targetBlockType: "hands_on_mission",
    systemPrompt: `Eres el generador pedagógico de LearnKit.
Emite ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Genera una 'hands_on_mission' de TRANSFERENCIA (isTransfer: true):
- isTransfer: true
- challengeStatement: implementar un deserializador de encabezados de paquetes sin ninguna asignación en el heap
- expectedMilestoneArtifact: función que acepta un slice &[u8] y retorna una struct Header<'a> referenciando el buffer
- constraints: array con 4 restricciones rigurosas (cero llamadas a malloc/Box, tiempo O(1), validación de magic bytes)
- scaffoldingHints: [] (ESTRICTAMENTE VACÍO: es un reto de transferencia, no lleva pistas)
- evaluationRubricSummary: 3 criterios de aceptación medibles
- supportLevel: "independent"`,
    userPrompt: `Genera el reto de transferencia final para comprobar dominio de lifetimes y referencias en Rust sin copia.`,
  },
  {
    id: "tpl_scenario_idempotency",
    name: "Plantilla Mecánica — tpl_scenario_choice (Claves de Idempotencia)",
    domain: "Diseño de APIs & Pagos",
    targetBlockType: "tpl_scenario_choice",
    systemPrompt: `Eres el generador pedagógico de LearnKit.
Emite ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Usa la PLANTILLA MECÁNICA 'tpl_scenario_choice':
- blockType: "tpl_scenario_choice"
- scenario: Un cliente móvil pierde conexión mientras procesa un cargo con tarjeta de crédito
- decisionPoint: ¿Cómo debe manejar el cliente y la pasarela el reintento para evitar cobros dobles?
- optimal: { choice: "Reenviar la solicitud con la misma Idempotency-Key UUID generada previamente", consequence: "La pasarela retorna la respuesta original almacenada sin duplicar el cobro" }
- alternatives: 2 alternativas con decisiones subóptimas y sus consecuencias.`,
    userPrompt: `Crea la plantilla de decisión de idempotencia bancaria.`,
  },
];

// 5. Tolerant JSON repair & Inner Unwrapping
function tolerantRepairJson(raw: string): { success: boolean; repaired?: any; error?: string } {
  try {
    return { success: true, repaired: JSON.parse(raw) };
  } catch (err: any) {
    let s = raw.trim();
    if (s.startsWith("```")) {
      s = s.replace(/^```(?:json)?\s*/i, "").replace(/\s*```$/, "");
    }
    s = s.replace(/,(\s*[}\]])/g, "$1");
    const stack: string[] = [];
    let inStr = false;
    let esc = false;
    for (let i = 0; i < s.length; i++) {
      const c = s[i];
      if (esc) {
        esc = false;
        continue;
      }
      if (c === "\\") {
        esc = true;
        continue;
      }
      if (c === '"') {
        inStr = !inStr;
        continue;
      }
      if (inStr) continue;
      if (c === "{" || c === "[") stack.push(c);
      else if (c === "}" && stack[stack.length - 1] === "{") stack.pop();
      else if (c === "]" && stack[stack.length - 1] === "[") stack.pop();
    }
    if (inStr) s += '"';
    while (stack.length > 0) {
      const open = stack.pop();
      s += open === "{" ? "}" : "]";
    }

    try {
      return { success: true, repaired: JSON.parse(s) };
    } catch (e: any) {
      return { success: false, error: e.message };
    }
  }
}

function unwrapInnerStringified(obj: any): { modified: boolean; obj: any } {
  let modified = false;
  if (obj && typeof obj === "object") {
    for (const key of Object.keys(obj)) {
      if (typeof obj[key] === "string") {
        const trimmed = obj[key].trim();
        if ((trimmed.startsWith("{") && trimmed.endsWith("}")) || (trimmed.startsWith("[") && trimmed.endsWith("]"))) {
          try {
            obj[key] = JSON.parse(trimmed);
            modified = true;
          } catch {
            const rep = tolerantRepairJson(trimmed);
            if (rep.success) {
              obj[key] = rep.repaired;
              modified = true;
            }
          }
        }
      }
      if (obj[key] && typeof obj[key] === "object") {
        const inner = unwrapInnerStringified(obj[key]);
        if (inner.modified) modified = true;
      }
    }
  }
  return { modified, obj };
}

// 6. Schema Auditor for Complex Forms
interface AuditResult {
  passed: boolean;
  blockTypeFound?: string;
  missingRequiredFields: string[];
  syntaxError?: string;
  hadStringifiedInnerFields: boolean;
  rawChars: number;
  tokens: { prompt: number; completion: number };
  durationMs: number;
  finishReason: string;
  nativeToolCall: boolean;
  repairedByTolerantParser: boolean;
}

function auditComplexPayload(
  expectedType: string,
  rawArgs: string,
  nativeToolCall: boolean,
  tokens: { prompt: number; completion: number },
  durationMs: number,
  finishReason: string
): AuditResult {
  const missing: string[] = [];
  let syntaxError: string | undefined;
  let repaired = false;
  let parsed: any = null;

  try {
    parsed = JSON.parse(rawArgs);
  } catch (e: any) {
    syntaxError = e.message;
    const repairAttempt = tolerantRepairJson(rawArgs);
    if (repairAttempt.success) {
      parsed = repairAttempt.repaired;
      repaired = true;
    }
  }

  if (!parsed || typeof parsed !== "object") {
    return {
      passed: false,
      missingRequiredFields: ["VALID_JSON_OBJECT"],
      syntaxError: syntaxError || "Payload is not a valid JSON object",
      hadStringifiedInnerFields: false,
      rawChars: rawArgs.length,
      tokens,
      durationMs,
      finishReason,
      nativeToolCall,
      repairedByTolerantParser: false,
    };
  }

  if (parsed.arguments && typeof parsed.arguments === "object") parsed = parsed.arguments;
  if (parsed.parameters && typeof parsed.parameters === "object") parsed = parsed.parameters;
  if (parsed.block && typeof parsed.block === "object") parsed = parsed.block;

  const unwrapResult = unwrapInnerStringified(parsed);
  parsed = unwrapResult.obj;
  const hadStringifiedInnerFields = unwrapResult.modified;

  const actualType = parsed.blockType;

  switch (actualType) {
    case "branching_scenario_challenge":
      if (!parsed.scenario) missing.push("scenario");
      if (!parsed.decisionPoint) missing.push("decisionPoint");
      if (!Array.isArray(parsed.branches) || parsed.branches.length < 2) {
        missing.push("branches (min 2)");
      } else {
        const hasOptimal = parsed.branches.some((b: any) => b.isOptimal === true);
        if (!hasOptimal) missing.push("branches.has_optimal_true");
      }
      break;

    case "declarative_visual_diagram":
      if (!parsed.title) missing.push("title");
      if (!parsed.visualAid || typeof parsed.visualAid !== "object") {
        missing.push("visualAid");
      } else {
        if (!parsed.visualAid.code) missing.push("visualAid.code");
        if (!parsed.visualAid.chartType) missing.push("visualAid.chartType");
      }
      if (!Array.isArray(parsed.guidedWalkthrough) || parsed.guidedWalkthrough.length === 0) {
        missing.push("guidedWalkthrough (non-empty)");
      }
      break;

    case "interactive_prediction_gate":
      if (!parsed.question) missing.push("question");
      if (parsed.interactionMode === "causal_chain_order") {
        if (!Array.isArray(parsed.shuffledSteps) || parsed.shuffledSteps.length < 2) missing.push("shuffledSteps");
        if (!Array.isArray(parsed.correctOrder) || parsed.correctOrder.length < 2) missing.push("correctOrder");
      } else if (parsed.interactionMode === "boundary_case_audit") {
        if (!Array.isArray(parsed.boundaryCases) || parsed.boundaryCases.length < 2) missing.push("boundaryCases");
      } else {
        if (!Array.isArray(parsed.options) || parsed.options.length < 2) missing.push("options");
        if (!parsed.correctOption) missing.push("correctOption");
      }
      break;

    case "heuristic_error_audit":
      if (!parsed.instruction) missing.push("instruction");
      if (!parsed.flawedRepresentation || typeof parsed.flawedRepresentation !== "object") {
        missing.push("flawedRepresentation");
      } else {
        if (!parsed.flawedRepresentation.buggySnippetOrDiagram) missing.push("flawedRepresentation.buggySnippetOrDiagram");
      }
      if (!parsed.modelSolution) missing.push("modelSolution");
      break;

    case "hands_on_mission":
      if (!parsed.challengeStatement) missing.push("challengeStatement");
      if (!parsed.expectedMilestoneArtifact) missing.push("expectedMilestoneArtifact");
      if (!parsed.constraints) missing.push("constraints");
      if (parsed.isTransfer === true) {
        if (Array.isArray(parsed.scaffoldingHints) && parsed.scaffoldingHints.length > 0) {
          missing.push("isTransfer must have empty scaffoldingHints");
        }
      }
      break;

    case "tpl_scenario_choice":
      if (!parsed.scenario) missing.push("scenario");
      if (!parsed.decisionPoint) missing.push("decisionPoint");
      if (!parsed.optimal || typeof parsed.optimal !== "object") missing.push("optimal");
      if (!Array.isArray(parsed.alternatives) || parsed.alternatives.length === 0) missing.push("alternatives");
      break;

    default:
      if (actualType !== expectedType) {
        missing.push(`expected ${expectedType}, got ${actualType}`);
      }
      break;
  }

  return {
    passed: missing.length === 0 && !syntaxError && actualType === expectedType,
    blockTypeFound: actualType,
    missingRequiredFields: missing,
    syntaxError,
    hadStringifiedInnerFields,
    rawChars: rawArgs.length,
    tokens,
    durationMs,
    finishReason,
    nativeToolCall,
    repairedByTolerantParser: repaired,
  };
}

// 7. Latency Optimization Profiler
async function runLatencyBenchmark(provider: ProviderConfig) {
  console.log(`\n================================================================================`);
  console.log(`⚡ BENCHMARK COMPARATIVO DE LATENCIA & CONSUMO DE TOKENS`);
  console.log(`Proveedor: ${provider.name} | Modelo: ${provider.model}`);
  console.log(`================================================================================\n`);

  const promptConfigs = [
    {
      label: "1. Baseline Libre (Sin topes de palabras)",
      systemPrompt: `Eres el generador pedagógico de LearnKit. Emite 'publish_notebook_block' con hands_on_mission. Describe el reto con todo detalle.`,
      userPrompt: `Reto práctico: Implementar un anillo de tokens distribuido con tolerancia a fallos.`,
    },
    {
      label: "2. Presupuesto Ajustado (Caps estrictos: ≤50 palabras, ≤2 hints)",
      systemPrompt: `Eres el generador pedagógico de LearnKit. Emite 'publish_notebook_block' con hands_on_mission.
REGLAS DE LATENCIA ESTRICTAS:
- challengeStatement: conciso en ≤40 palabras.
- constraints: exactamente 3 viñetas concisas.
- scaffoldingHints: exactamente 2 pistas de ≤20 palabras.
- evaluationRubricSummary: 2 criterios breves.
Sé directo, denso y sin relleno.`,
      userPrompt: `Reto práctico: Implementar un anillo de tokens distribuido con tolerancia a fallos.`,
    },
    {
      label: "3. Plantilla Mecánica tpl_scenario_choice (Mínimos tokens)",
      systemPrompt: `Eres el generador pedagógico de LearnKit. Emite 'publish_notebook_block' con la plantilla tpl_scenario_choice.
Llena exclusivamente scenario (≤30 palabras), decisionPoint, optimal y 1 alternativa.`,
      userPrompt: `Reto de decisión: Elección de topología de replicación para un anillo de tokens.`,
    },
  ];

  const benchResults: { label: string; durationMs: number; outputTokens: number; chars: number; tokPerSec: number }[] = [];

  for (const cfg of promptConfigs) {
    process.stdout.write(`   Ejecutando: [${cfg.label}]... `);
    const start = Date.now();
    const res = await fetch(provider.baseUrl, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${provider.apiKey}`,
      },
      body: JSON.stringify({
        model: provider.model,
        messages: [
          { role: "system", content: cfg.systemPrompt },
          { role: "user", content: cfg.userPrompt },
        ],
        tools: [PUBLISH_BLOCK_TOOL],
        tool_choice: { type: "function", function: { name: "publish_notebook_block" } },
        temperature: 0.2,
        ...provider.extraBody,
      }),
    });

    const durationMs = Date.now() - start;
    const data: any = await res.json();
    const usage = data.usage || { completion_tokens: 0 };
    const raw = data.choices?.[0]?.message?.tool_calls?.[0]?.function?.arguments || "";
    const tokPerSec = usage.completion_tokens > 0 ? (usage.completion_tokens / (durationMs / 1000)).toFixed(1) : "0";

    benchResults.push({
      label: cfg.label,
      durationMs,
      outputTokens: usage.completion_tokens,
      chars: raw.length,
      tokPerSec: Number(tokPerSec),
    });

    console.log(`Completado: ${durationMs}ms | ${usage.completion_tokens} tokens (${tokPerSec} tok/s)`);
  }

  console.log(`\n### Scorecard Comparativo de Latencia\n`);
  console.log(`| Configuración de Prompt | Latencia (ms) | Tokens Salida | Caracteres | Velocidad (tok/s) | Ahorro de Tiempo |\n|---|---|---|---|---|---|`);
  const baseTime = benchResults[0].durationMs;
  for (const b of benchResults) {
    const savings = b.durationMs === baseTime ? "0% (Base)" : `-${Math.round((1 - b.durationMs / baseTime) * 100)}%`;
    console.log(`| **${b.label}** | ${b.durationMs}ms | ${b.outputTokens} | ${b.chars} | ${b.tokPerSec} tok/s | **${savings}** |`);
  }
}

// 8. Main Runner
async function runComplexEvaluation() {
  const provider = resolveProvider(requestedModel);

  console.log(`\n================================================================================`);
  console.log(`🔬 LearnKit - Batería de Pruebas Complejas Multitema y Multiforma`);
  console.log(`Proveedor:  ${provider.name}`);
  console.log(`Modelo:     ${provider.model}`);
  console.log(`Endpoint:   ${provider.baseUrl}`);
  console.log(`Escenarios: ${COMPLEX_SCENARIOS.length} pruebas avanzadas`);
  console.log(`Timestamp:  ${new Date().toISOString()}`);
  console.log(`================================================================================\n`);

  const activeScenarios = requestedScenario
    ? COMPLEX_SCENARIOS.filter((s) => s.id === requestedScenario || s.targetBlockType === requestedScenario)
    : COMPLEX_SCENARIOS;

  const results: { scenario: Scenario; audit: AuditResult; rawOutput: string }[] = [];

  for (let i = 0; i < activeScenarios.length; i++) {
    const sc = activeScenarios[i];
    console.log(`[${i + 1}/${activeScenarios.length}] Dominio: [${sc.domain}] → ${sc.name}...`);

    const start = Date.now();
    let res: Response;
    try {
      res = await fetch(provider.baseUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${provider.apiKey}`,
        },
        body: JSON.stringify({
          model: provider.model,
          messages: [
            { role: "system", content: sc.systemPrompt },
            { role: "user", content: sc.userPrompt },
          ],
          tools: [PUBLISH_BLOCK_TOOL],
          tool_choice: { type: "function", function: { name: "publish_notebook_block" } },
          temperature: 0.2,
          ...provider.extraBody,
        }),
      });
    } catch (netErr: any) {
      console.error(`   ❌ Error de red / HTTP: ${netErr.message}`);
      continue;
    }

    const durationMs = Date.now() - start;
    if (!res.ok) {
      const errText = await res.text();
      console.error(`   ❌ HTTP ${res.status}: ${errText}`);
      continue;
    }

    const data: any = await res.json();
    const choice = data.choices?.[0];
    const message = choice?.message;
    const finishReason = choice?.finish_reason || "unknown";
    const usage = data.usage || { prompt_tokens: 0, completion_tokens: 0 };

    let rawArgs = "";
    let nativeToolCall = false;

    if (message?.tool_calls?.[0]?.function?.arguments) {
      nativeToolCall = true;
      rawArgs = message.tool_calls[0].function.arguments;
    } else if (message?.content) {
      rawArgs = message.content;
    }

    const audit = auditComplexPayload(
      sc.targetBlockType,
      rawArgs,
      nativeToolCall,
      { prompt: usage.prompt_tokens, completion: usage.completion_tokens },
      durationMs,
      finishReason
    );

    results.push({ scenario: sc, audit, rawOutput: rawArgs });

    if (audit.passed) {
      const extra = audit.hadStringifiedInnerFields ? " [Auto-deserializado string interno]" : "";
      console.log(`   ✅ PASS (${durationMs}ms | ${audit.tokens.completion} tokens | ${rawArgs.length} chars)${extra}`);
    } else {
      console.log(`   ❌ DEFECTO (${durationMs}ms | ${audit.finishReason})`);
      if (audit.blockTypeFound !== sc.targetBlockType) {
        console.log(`      • Discrepancia blockType: esperado '${sc.targetBlockType}', obtenido '${audit.blockTypeFound}'`);
      }
      if (audit.syntaxError) console.log(`      • Error Sintáctico JSON: ${audit.syntaxError}`);
      if (audit.missingRequiredFields.length > 0) {
        console.log(`      • Faltantes / Violaciones: ${audit.missingRequiredFields.join(", ")}`);
      }
    }
  }

  // Generate Report
  let md = `# Reporte de Pruebas Complejas Multitema y Análisis de Latencia\n\n`;
  md += `**Proveedor:** \`${provider.name}\` | **Modelo:** \`${provider.model}\` | **Fecha:** \`${new Date().toISOString()}\`\n\n`;
  md += `### 1. Resumen Ejecutivo de Formas Complejas\n\n`;
  md += `| Dominio | Tipo de Bloque | Finish Reason | Tool Call Nativo? | JSON Válido? | Campos Stringificados? | Resultado | Latencia | Tokens |\n`;
  md += `|---|---|---|---|---|---|---|---|---|\n`;

  for (const r of results) {
    const a = r.audit;
    const jsonStatus = a.syntaxError ? (a.repairedByTolerantParser ? "⚠️ Truncado (Reparado)" : "❌ Inválido") : "✅ Válido";
    const resultTag = a.passed ? "✅ PASS" : "❌ DEFECTO";
    const stringifiedTag = a.hadStringifiedInnerFields ? "⚠️ Sí (Desescapado)" : "No (Nativo)";
    md += `| **${r.scenario.domain}** | \`${r.scenario.targetBlockType}\` | \`${a.finishReason}\` | ${a.nativeToolCall ? "✅ Sí" : "❌ No"} | ${jsonStatus} | ${stringifiedTag} | ${resultTag} | ${a.durationMs}ms | in:${a.tokens.prompt}/out:${a.tokens.completion} |\n`;
  }

  md += `\n### 2. Detalle y Payloads Generados por Dominio\n\n`;
  for (const r of results) {
    const a = r.audit;
    md += `#### ${r.scenario.name} (\`${r.scenario.domain}\`)\n\n`;
    md += `- **Tipo de Bloque:** \`${r.scenario.targetBlockType}\`\n`;
    md += `- **Tokens:** In: ${a.tokens.prompt} / Out: ${a.tokens.completion} | **Latencia:** ${(a.durationMs / 1000).toFixed(2)}s\n`;
    if (a.missingRequiredFields.length > 0) {
      md += `> [!WARNING]\n> Campos Faltantes: ${a.missingRequiredFields.join(", ")}\n\n`;
    }
    md += `<details><summary>Ver Payload JSON Generado (${a.rawChars} chars)</summary>\n\n\`\`\`json\n${r.rawOutput.slice(0, 2000)}${r.rawOutput.length > 2000 ? "\n... (truncado para visualización)" : ""}\n\`\`\`\n\n</details>\n\n---\n\n`;
  }

  const outPath = resolve(process.cwd(), "eval-complex-results.md");
  writeFileSync(outPath, md, "utf-8");
  console.log(`\n📄 Reporte de pruebas complejas guardado en: ${outPath}\n`);

  // Run Latency Profiler if requested or as part of full run
  if (isBenchmarkLatency || activeScenarios.length === COMPLEX_SCENARIOS.length) {
    await runLatencyBenchmark(provider);
  }
}

runComplexEvaluation().catch((e) => {
  console.error("Fatal:", e);
  process.exit(1);
});
