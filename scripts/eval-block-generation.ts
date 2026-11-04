import { readFileSync, writeFileSync } from "fs";
import { resolve } from "path";

// 1. Read API keys from .env
function loadEnv(): Record<string, string> {
  const env: Record<string, string> = { ...process.env as Record<string, string> };
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

// 3. Exact schema for `publish_notebook_block` tool
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
            options: { type: "array", items: { type: "string" }, minItems: 2, maxItems: 4 },
            correctOption: { type: "string" },
            conceptualFeedbackMap: { type: "object", additionalProperties: { type: "string" } },
          },
          required: ["blockType", "question", "options", "correctOption", "conceptualFeedbackMap"],
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
      ],
    },
  },
};

// 4. Scenarios to test
interface Scenario {
  id: string;
  name: string;
  targetBlockType: string;
  systemPrompt: string;
  userPrompt: string;
}

const SCENARIOS: Scenario[] = [
  {
    id: "hands_on_mission",
    name: "Hands-on Mission (Práctica Guiada con Cloze y Restricciones)",
    targetBlockType: "hands_on_mission",
    systemPrompt: `Eres el generador pedagógico de LearnKit.
Debes emitir ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Para 'hands_on_mission', debes incluir:
- blockType: "hands_on_mission"
- challengeStatement
- expectedMilestoneArtifact
- constraints (array de strings)
- scaffoldingHints (array de 2-3 pistas)
- evaluationRubricSummary (array de criterios)
- supportLevel: "guided"
- workedTemplate: plantilla con huecos ___
NUNCA stringifiques arrays ni objetos internos: usa tipos nativos de JSON.`,
    userPrompt: `Tema: Manejo de Concurrencia en Rust con Arc y Mutex.
Diseña el reto práctico para implementar un contador seguro entre 4 hilos concurrentes.`,
  },
  {
    id: "heuristic_error_audit",
    name: "Heuristic Error Audit (Auditoría de Código y Fallo Conceptual)",
    targetBlockType: "heuristic_error_audit",
    systemPrompt: `Eres el generador pedagógico de LearnKit.
Debes emitir ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Para 'heuristic_error_audit', debes incluir:
- blockType: "heuristic_error_audit"
- instruction
- flawedRepresentation: objeto nativo { context, buggySnippetOrDiagram, errorType: "conceptual_misunderstanding" }
- faultyLineNumber: número de línea 1-based donde está el bug
- guidingQuestions (array de strings)
- modelSolution
NUNCA devuelvas flawedRepresentation como un string escapado: debe ser un objeto JSON nativo.`,
    userPrompt: `Tema: React Hooks - Stale Closures en useEffect.
Crea un ejercicio de auditoría donde un setInterval dentro de useEffect lee una variable de estado que nunca se actualiza porque el array de dependencias está vacío [].`,
  },
  {
    id: "interactive_prediction_gate",
    name: "Interactive Prediction Gate (Compuerta de Predicción de Hipótesis)",
    targetBlockType: "interactive_prediction_gate",
    systemPrompt: `Eres el generador pedagógico de LearnKit.
Debes emitir ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Para 'interactive_prediction_gate', debes incluir:
- blockType: "interactive_prediction_gate"
- question: la pregunta que obliga a formular una hipótesis previa
- options: array de 3 o 4 opciones
- correctOption: exactamente una de las opciones
- conceptualFeedbackMap: mapa clave-valor con una explicación por CADA opción
NO cambies el blockType a anchored_micro_theory. Genera exclusivamente una compuerta 'interactive_prediction_gate'.`,
    userPrompt: `Tema: JavaScript Event Loop - Microtasks vs Macrotasks.
Pregunta qué se imprime primero entre Promise.resolve().then() y setTimeout(..., 0).`,
  },
  {
    id: "anchored_micro_theory",
    name: "Anchored Micro Theory (Teoría en Capas)",
    targetBlockType: "anchored_micro_theory",
    systemPrompt: `Eres el generador pedagógico de LearnKit.
Debes emitir ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Para 'anchored_micro_theory', debes incluir:
- blockType: "anchored_micro_theory"
- title
- intuitiveHook (≤30 palabras, metáfora del mundo real)
- analogyBoundary (≤25 palabras, límite donde la metáfora falla)
- systemRule (regla formal paso a paso)
- frequentError (confusión común contrastada)`,
    userPrompt: `Tema: Protocolo Raft y Consenso Distribuido. Explica el mecanismo de Elección de Líder con temporizadores aleatorios de latido.`,
  },
  {
    id: "metacognitive_closure",
    name: "Metacognitive Closure (Cierre y Metacognición)",
    targetBlockType: "metacognitive_closure",
    systemPrompt: `Eres el generador pedagógico de LearnKit.
Debes emitir ÚNICAMENTE una llamada a la herramienta 'publish_notebook_block'.
Para 'metacognitive_closure', debes incluir:
- blockType: "metacognitive_closure"
- synthesisTask: reflexión de consolidación
- predictionComparison: objeto nativo { initialPrediction, finalResult, contrastNarrative }
- selfEvaluationChecklist: array de 2 a 3 strings
NUNCA devuelvas predictionComparison ni selfEvaluationChecklist como strings escapados.`,
    userPrompt: `El estudiante predijo inicialmente que "setTimeout(fn, 0) siempre se ejecuta de inmediato antes de cualquier otra línea de código".
Durante la clase descubrió que las microtareas y la pila de ejecución tienen prioridad absoluta.`,
  },
];

// 5. Tolerant JSON repair and uncoercing utility
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

// 6. Schema & Field Auditor
interface AuditResult {
  passed: boolean;
  blockTypeFound?: string;
  missingRequiredFields: string[];
  casingMismatches: string[];
  syntaxError?: string;
  hadStringifiedInnerFields: boolean;
  rawChars: number;
  tokens: { prompt: number; completion: number };
  durationMs: number;
  finishReason: string;
  nativeToolCall: boolean;
  repairedByTolerantParser: boolean;
}

function auditBlockPayload(
  expectedType: string,
  rawArgs: string,
  nativeToolCall: boolean,
  tokens: { prompt: number; completion: number },
  durationMs: number,
  finishReason: string
): AuditResult {
  const missing: string[] = [];
  const casingIssues: string[] = [];
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
      casingMismatches: [],
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

  // Unwrap potential wrapper objects
  if (parsed.arguments && typeof parsed.arguments === "object") parsed = parsed.arguments;
  if (parsed.parameters && typeof parsed.parameters === "object") parsed = parsed.parameters;
  if (parsed.block && typeof parsed.block === "object") parsed = parsed.block;

  // Uncoerce inner stringified fields
  const unwrapResult = unwrapInnerStringified(parsed);
  parsed = unwrapResult.obj;
  const hadStringifiedInnerFields = unwrapResult.modified;

  const actualType = parsed.blockType;

  // Check required fields per block type
  if (actualType === "hands_on_mission") {
    if (!parsed.challengeStatement) missing.push("challengeStatement");
    if (!parsed.expectedMilestoneArtifact) missing.push("expectedMilestoneArtifact");
    if (!parsed.constraints) missing.push("constraints");
    if (!parsed.scaffoldingHints) {
      if (parsed.hints) casingIssues.push("used 'hints' instead of 'scaffoldingHints'");
      else missing.push("scaffoldingHints");
    }
    if (!parsed.evaluationRubricSummary) missing.push("evaluationRubricSummary");
    if (!parsed.supportLevel) missing.push("supportLevel");
  } else if (actualType === "heuristic_error_audit") {
    if (!parsed.instruction) missing.push("instruction");
    if (!parsed.flawedRepresentation || typeof parsed.flawedRepresentation !== "object") {
      missing.push("flawedRepresentation");
    } else {
      if (!parsed.flawedRepresentation.context) missing.push("flawedRepresentation.context");
      if (!parsed.flawedRepresentation.buggySnippetOrDiagram) missing.push("flawedRepresentation.buggySnippetOrDiagram");
      if (!parsed.flawedRepresentation.errorType) missing.push("flawedRepresentation.errorType");
    }
    if (!parsed.guidingQuestions) missing.push("guidingQuestions");
    if (!parsed.modelSolution) missing.push("modelSolution");
  } else if (actualType === "interactive_prediction_gate") {
    if (!parsed.question) missing.push("question");
    if (!Array.isArray(parsed.options) || parsed.options.length < 2) missing.push("options");
    if (!parsed.correctOption) missing.push("correctOption");
    if (!parsed.conceptualFeedbackMap) missing.push("conceptualFeedbackMap");
  } else if (actualType === "anchored_micro_theory") {
    if (!parsed.title) missing.push("title");
    if (!parsed.intuitiveHook) missing.push("intuitiveHook");
    if (!parsed.systemRule) missing.push("systemRule");
    if (!parsed.frequentError) missing.push("frequentError");
  } else if (actualType === "metacognitive_closure") {
    if (!parsed.synthesisTask) missing.push("synthesisTask");
    if (!parsed.predictionComparison || typeof parsed.predictionComparison !== "object") {
      missing.push("predictionComparison");
    } else {
      if (!parsed.predictionComparison.initialPrediction) missing.push("predictionComparison.initialPrediction");
      if (!parsed.predictionComparison.finalResult) missing.push("predictionComparison.finalResult");
      if (!parsed.predictionComparison.contrastNarrative) missing.push("predictionComparison.contrastNarrative");
    }
    if (!parsed.selfEvaluationChecklist) missing.push("selfEvaluationChecklist");
  }

  // Check snake_case slips
  for (const key of Object.keys(parsed)) {
    if (key.includes("_") && key !== "blockType") {
      casingIssues.push(`snake_case field '${key}'`);
    }
  }

  return {
    passed: missing.length === 0 && !syntaxError && actualType === expectedType,
    blockTypeFound: actualType,
    missingRequiredFields: missing,
    casingMismatches: casingIssues,
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

// 7. Execution Engine
async function runEvaluation() {
  const provider = resolveProvider(requestedModel);
  console.log(`\n================================================================================`);
  console.log(`🔬 LearnKit - Evaluador de Generación de Bloques con LLM Real (v2 con Inner Unwrapping)`);
  console.log(`Proveedor:  ${provider.name}`);
  console.log(`Modelo:     ${provider.model}`);
  console.log(`Endpoint:   ${provider.baseUrl}`);
  console.log(`Timestamp:  ${new Date().toISOString()}`);
  console.log(`================================================================================\n`);

  const activeScenarios = requestedScenario
    ? SCENARIOS.filter((s) => s.id === requestedScenario || s.targetBlockType === requestedScenario)
    : SCENARIOS;

  const results: { scenario: Scenario; audit: AuditResult; rawOutput: string }[] = [];

  for (let i = 0; i < activeScenarios.length; i++) {
    const sc = activeScenarios[i];
    console.log(`[${i + 1}/${activeScenarios.length}] Evaluando: ${sc.name} (${sc.targetBlockType})...`);

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

    const audit = auditBlockPayload(
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
        console.log(`      • Discrepancia de blockType: esperado '${sc.targetBlockType}', emitido '${audit.blockTypeFound}'`);
      }
      if (audit.syntaxError) console.log(`      • Error Sintáctico JSON: ${audit.syntaxError}`);
      if (audit.missingRequiredFields.length > 0) {
        console.log(`      • Campos Faltantes: ${audit.missingRequiredFields.join(", ")}`);
      }
      if (audit.casingMismatches.length > 0) {
        console.log(`      • Desviación de Casing: ${audit.casingMismatches.join(", ")}`);
      }
    }
  }

  generateReport(provider, results);
}

function generateReport(provider: ProviderConfig, results: { scenario: Scenario; audit: AuditResult; rawOutput: string }[]) {
  console.log(`\n================================================================================`);
  console.log(`📊 REPORTE DE EVALUACIÓN DIAGNÓSTICA DE GENERACIÓN DE BLOQUES`);
  console.log(`================================================================================\n`);

  let md = `# Reporte de Evaluación de Generación de Bloques con LLM Real\n\n`;
  md += `**Proveedor:** \`${provider.name}\` | **Modelo:** \`${provider.model}\` | **Fecha:** \`${new Date().toISOString()}\`\n\n`;

  md += `### 1. Resumen Ejecutivo de Errores por Tipo de Bloque\n\n`;
  md += `| Tipo de Bloque | Finish Reason | Tool Call Nativo? | JSON Válido? | Campos Internos Stringificados? | Resultado | Latencia | Tokens |\n`;
  md += `|---|---|---|---|---|---|---|---|\n`;

  for (const r of results) {
    const a = r.audit;
    const jsonStatus = a.syntaxError ? (a.repairedByTolerantParser ? "⚠️ Truncado (Reparado)" : "❌ Inválido") : "✅ Válido";
    const resultTag = a.passed ? "✅ PASS" : "❌ DEFECTO";
    const stringifiedTag = a.hadStringifiedInnerFields ? "⚠️ Sí (Desescapado)" : "No (Nativo)";
    md += `| **${r.scenario.targetBlockType}** | \`${a.finishReason}\` | ${a.nativeToolCall ? "✅ Sí" : "❌ No"} | ${jsonStatus} | ${stringifiedTag} | ${resultTag} | ${a.durationMs}ms | in:${a.tokens.prompt}/out:${a.tokens.completion} |\n`;
  }

  md += `\n### 2. Hallazgos y Análisis Causal de Fallos\n\n`;

  for (const r of results) {
    const a = r.audit;
    md += `#### Bloque: \`${r.scenario.targetBlockType}\` (${r.scenario.name})\n\n`;
    md += `- **Llamada de Herramienta:** ${a.nativeToolCall ? "Nativa (`tool_calls`)" : "En texto/prose"}\n`;
    md += `- **Longitud del payload:** ${a.rawChars} caracteres | **Tokens de salida:** ${a.tokens.completion}\n`;
    md += `- **Tiempo de generación:** ${(a.durationMs / 1000).toFixed(2)}s\n`;

    if (a.hadStringifiedInnerFields) {
      md += `> [!NOTE]\n`;
      md += `> **Anomalía de Doble Stringificación Detectada:** El modelo serializó campos internos (como arrays u objetos) como strings JSON con comillas escapadas en lugar de objetos JSON nativos. La función de deserialización tolerante corrigió esto automáticamente.\n\n`;
    }

    if (a.missingRequiredFields.length > 0) {
      md += `> [!IMPORTANT]\n`;
      md += `> **Campos requeridos por el esquema de LearnKit omitidos por el modelo:**\n`;
      for (const f of a.missingRequiredFields) {
        md += `> - \`${f}\`\n`;
      }
      md += `> En LearnKit, la función \`ensure_required_block_fields\` interviene aquí inyectando defaults estructurados para prevenir rechazos Serde.\n\n`;
    }

    md += `<details><summary>Ver payload JSON generado (${a.rawChars} chars)</summary>\n\n\`\`\`json\n${r.rawOutput.slice(0, 2000)}${r.rawOutput.length > 2000 ? "\n... (truncado para visualización)" : ""}\n\`\`\`\n\n</details>\n\n---\n\n`;
  }

  const outPath = resolve(process.cwd(), "eval-block-results.md");
  writeFileSync(outPath, md, "utf-8");
  console.log(md);
  console.log(`\n📄 Reporte completo guardado en: ${outPath}\n`);
}

runEvaluation().catch((e) => {
  console.error("Fatal:", e);
  process.exit(1);
});
