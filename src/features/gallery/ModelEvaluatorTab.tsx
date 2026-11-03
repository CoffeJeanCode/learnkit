import { useEffect, useState } from "react";
import {
  computeLengthDisparity,
  evaluateDiagnosticQuestion,
  evaluateHandsOnMissionBlock,
  evaluateAnchoredTheoryBlock,
  type GroundingEvaluationReport,
} from "./groundingHarness";
import { listProviders } from "../../lib/tauri";
import type { ProviderWithStatus } from "../../types";

interface ModelBenchmarkPreset {
  id: string;
  name: string;
  provider: string;
  tier: "flagship" | "fast" | "local_8b";
  latencyMs: number;
  output: Record<string, unknown>;
  repairedOutput?: Record<string, unknown>;
}

interface BenchmarkTask {
  id: string;
  title: string;
  description: string;
  evaluationFocus: string;
  models: Record<string, ModelBenchmarkPreset>;
}

const BENCHMARK_TASKS: BenchmarkTask[] = [
  {
    id: "diagnostic_battery",
    title: "Batería Diagnóstica — Sesgo de Longitud (< 15%)",
    description: "Generar preguntas de opción múltiple donde ninguna opción destaque por ser notablemente más larga o corta.",
    evaluationFocus: "Paridad estricta de longitud en distractores: (max - min) / min < 15%.",
    models: {
      model_deepseek: {
        id: "model_deepseek",
        name: "DeepSeek V4 / Chat (deepseek-chat)",
        provider: "DeepSeek",
        tier: "flagship",
        latencyMs: 440,
        output: {
          stem: "¿Cuál es la principal ventaja de definir un token base central en lugar de duplicar valores numéricos?",
          options: [
            "Centraliza el origen del cambio para toda la jerarquía de UI.",
            "Permite escalar el valor origen automáticamente en toda la app.",
            "Garantiza que la densidad escale sin mutar el archivo principal.",
          ],
        },
      },
      model_mimo: {
        id: "model_mimo",
        name: "Xiaomi MiMo v2.6 Pro (mimo-v2.6-pro)",
        provider: "Xiaomi",
        tier: "fast",
        latencyMs: 290,
        output: {
          stem: "¿Cuál es la principal ventaja de definir un token base central en lugar de duplicar valores numéricos?",
          options: [
            "Centraliza el origen del cambio para toda la jerarquía de estilos.",
            "Permite derivar tokens dependientes mediante factores relativos.",
            "Evita refactorizaciones masivas al modificar la escala de la app.",
          ],
        },
      },
      model_flash: {
        id: "model_flash",
        name: "Gemini 1.5 Flash",
        provider: "Google",
        tier: "fast",
        latencyMs: 380,
        output: {
          stem: "¿Cuál es la principal ventaja de definir un token base central en lugar de duplicar valores numéricos?",
          options: [
            "Centraliza el origen del cambio para toda la jerarquía.",
            "Evita valores fijos directos.",
            "Permite que cada token de densidad secundario multiplique el valor base sin tocar el origen del diseño.",
          ],
        },
        repairedOutput: {
          stem: "¿Cuál es la principal ventaja de definir un token base central en lugar de duplicar valores numéricos?",
          options: [
            "Centraliza el origen del cambio para toda la jerarquía de UI.",
            "Permite escalar el valor origen automáticamente en toda la app.",
            "Garantiza que la densidad escale sin mutar el archivo principal.",
          ],
        },
      },
      model_pro: {
        id: "model_pro",
        name: "Gemini 1.5 Pro",
        provider: "Google",
        tier: "flagship",
        latencyMs: 920,
        output: {
          stem: "¿Por qué un token de densidad derivado debe usar una fórmula multiplicativa en vez de un valor absoluto?",
          options: [
            "Permite que la escala completa responda si el origen base cambia mañana.",
            "Asegura que el espaciado mantenga proporciones armónicas en la pantalla.",
            "Evita inconsistencias visuales cuando se adapta a dispositivos táctiles.",
          ],
        },
      },
      model_sonnet: {
        id: "model_sonnet",
        name: "Claude 3.5 Sonnet",
        provider: "Anthropic",
        tier: "flagship",
        latencyMs: 840,
        output: {
          stem: "¿Qué riesgo introduce cablear números fijos (ej. 12px) en tokens derivados secundarios?",
          options: [
            "Rompe la sincronización global cuando se actualiza el valor base de escala.",
            "Obliga a refactorizar manualmente cada componente ante un cambio de estilo.",
            "Impide que el sistema de diseño soporte temas compactos o confortables.",
          ],
        },
      },
      model_8b: {
        id: "model_8b",
        name: "Llama 3.1 8B (Local)",
        provider: "Ollama / Local",
        tier: "local_8b",
        latencyMs: 1200,
        output: {
          stem: "¿Qué función cumple el cálculo de tokens relativos?",
          options: [
            "Hacer los estilos más legibles.",
            "Multiplicar la escala base para mantener consistencia modular.",
            "Escribir menos código en CSS.",
          ],
        },
        repairedOutput: {
          stem: "¿Qué función cumple el cálculo de tokens relativos?",
          options: [
            "Mantiene las proporciones armónicas de la interfaz.",
            "Multiplica la escala base asegurando consistencia.",
            "Evita declarar valores absolutos en componentes.",
          ],
        },
      },
    },
  },
  {
    id: "hands_on_mission",
    title: "Misión Práctica — Formato de Plantilla y Huecos",
    description: "Generar plantilla de ejercicio con huecos '___', saltos de línea claros y reglas invariantes.",
    evaluationFocus: "Sintaxis de huecos '___', legibilidad con espaciado \n\n, regla invariante no rota.",
    models: {
      model_deepseek: {
        id: "model_deepseek",
        name: "DeepSeek V4 / Chat (deepseek-chat)",
        provider: "DeepSeek",
        tier: "flagship",
        latencyMs: 460,
        output: {
          blockType: "hands_on_mission",
          challengeStatement: "Crea la escala de tokens derivados multiplicando la base sin tocar el token origen.",
          workedTemplate: `Tabla de tokens de densidad — completa los huecos:

Fila 1: spacing-base, valor ___ px, fórmula calc(var(--spacing-base) * ___)

Fila 2: radius-base, valor ___ px, fórmula calc(var(--radius-base) * ___)

Fila 3: padding-base, valor ___ px, fórmula calc(var(--padding-base) * ___)

📌 Regla que NO se rompe: el valor base se define solo en la base. En la fórmula siempre se utiliza la variable de origen con su multiplicador.`,
          expectedMilestoneArtifact: "Tabla de 3 filas con variables CSS compuestas con calc().",
          constraints: [
            "Ningún valor derivado tiene números absolutos fijos.",
            "Cada fórmula usa var() referenciando su token base.",
          ],
          scaffoldingHints: [
            "Usa var(--spacing-base) dentro de calc().",
            "El multiplicador denso es 0.75.",
          ],
          evaluationRubricSummary: [
            "Uso estricto de variables y calc()",
            "Ausencia de números quemados en las fórmulas",
          ],
        },
      },
      model_mimo: {
        id: "model_mimo",
        name: "Xiaomi MiMo v2.6 Pro (mimo-v2.6-pro)",
        provider: "Xiaomi",
        tier: "fast",
        latencyMs: 310,
        output: {
          blockType: "hands_on_mission",
          challengeStatement: "Configura la derivación de tokens en CSS sin duplicar constantes.",
          workedTemplate: `Plantilla de tokens derivados:

Fila 1: spacing-base, valor ___ px, multiplicador 0.75, derivado spacing-dense, fórmula calc(var(--spacing-base) * ___)

Fila 2: radius-base, valor ___ px, multiplicador 1.25, derivado radius-roomy, fórmula calc(var(--radius-base) * ___)

Fila 3: padding-base, valor ___ px, multiplicador 1.00, derivado padding-comfort, fórmula calc(var(--padding-base) * ___)

📌 Regla que NO se rompe: el valor numérico base solo se escribe UNA sola vez en el origen.`,
          expectedMilestoneArtifact: "Tabla completa con calc() y variables relativas.",
          constraints: [
            "Ningún valor derivado tiene números absolutos fijos.",
            "Cada fórmula usa var() referenciando su token base.",
          ],
          scaffoldingHints: [
            "Usa var(--spacing-base) dentro de calc().",
          ],
          evaluationRubricSummary: [
            "Uso estricto de variables relativas",
          ],
        },
      },
      model_flash: {
        id: "model_flash",
        name: "Gemini 1.5 Flash",
        provider: "Google",
        tier: "fast",
        latencyMs: 420,
        output: {
          blockType: "hands_on_mission",
          challengeStatement: "Crea la escala de tokens derivados multiplicando la base.",
          workedTemplate: `Tabla de tokens — completa los huecos:
Fila 1: spacing-base, valor ___ px; Fila 2: radius-base, valor ___ px; Fila 3: padding-base, valor ___ px; Regla: usar var()`,
          expectedMilestoneArtifact: "Tabla de 3 filas con variables CSS.",
          constraints: [],
          scaffoldingHints: [],
          evaluationRubricSummary: [],
        },
        repairedOutput: {
          blockType: "hands_on_mission",
          challengeStatement: "Crea la escala de tokens derivados multiplicando la base sin tocar el token origen.",
          workedTemplate: `Tabla de tokens de densidad — completa los huecos:

Fila 1: spacing-base, valor ___ px, fórmula calc(var(--spacing-base) * ___)

Fila 2: radius-base, valor ___ px, fórmula calc(var(--radius-base) * ___)

Fila 3: padding-base, valor ___ px, fórmula calc(var(--padding-base) * ___)

📌 Regla que NO se rompe: el valor base se define solo en la base. En la fórmula siempre se utiliza la variable de origen con su multiplicador.`,
          expectedMilestoneArtifact: "Tabla de 3 filas con variables CSS compuestas con calc().",
          constraints: [
            "Ningún valor derivado tiene números absolutos fijos.",
          ],
          scaffoldingHints: [
            "Usa var(--spacing-base) dentro de calc().",
          ],
          evaluationRubricSummary: [
            "Uso estricto de variables relativas",
          ],
        },
      },
      model_pro: {
        id: "model_pro",
        name: "Gemini 1.5 Pro",
        provider: "Google",
        tier: "flagship",
        latencyMs: 1100,
        output: {
          blockType: "hands_on_mission",
          challengeStatement: "Diseña la escala de tokens multiplicativa para diseño responsivo.",
          workedTemplate: `Tabla de tokens de densidad — completa los huecos:

Fila 1: spacing-base, valor ___ px, fórmula calc(var(--spacing-base) * ___)

Fila 2: radius-base, valor ___ px, fórmula calc(var(--radius-base) * ___)

Fila 3: padding-base, valor ___ px, fórmula calc(var(--padding-base) * ___)

📌 Regla que NO se rompe: el valor base se define solo en la base. En la fórmula siempre se utiliza la variable de origen con su multiplicador.`,
          expectedMilestoneArtifact: "Tabla de 3 filas con variables CSS compuestas con calc().",
          constraints: [
            "Ningún valor derivado tiene números absolutos fijos.",
            "Cada fórmula usa var() referenciando su token base.",
          ],
          scaffoldingHints: [
            "Usa var(--spacing-base) dentro de calc().",
          ],
          evaluationRubricSummary: [
            "Uso estricto de variables relativas",
          ],
        },
      },
      model_sonnet: {
        id: "model_sonnet",
        name: "Claude 3.5 Sonnet",
        provider: "Anthropic",
        tier: "flagship",
        latencyMs: 910,
        output: {
          blockType: "hands_on_mission",
          challengeStatement: "Configura la derivación de tokens en CSS sin duplicar constantes.",
          workedTemplate: `Plantilla de tokens derivados:

Fila 1: spacing-base, valor ___ px, multiplicador 0.75, derivado spacing-dense, fórmula calc(var(--spacing-base) * ___)

Fila 2: radius-base, valor ___ px, multiplicador 1.25, derivado radius-roomy, fórmula calc(var(--radius-base) * ___)

Fila 3: padding-base, valor ___ px, multiplicador 1.00, derivado padding-comfort, fórmula calc(var(--padding-base) * ___)

📌 Regla que NO se rompe: el valor numérico base solo se escribe UNA sola vez en el origen.`,
          expectedMilestoneArtifact: "Tabla completa con calc() y variables relativas.",
          constraints: [
            "Ningún valor derivado tiene números absolutos fijos.",
          ],
          scaffoldingHints: [
            "Usa var(--spacing-base) dentro de calc().",
          ],
          evaluationRubricSummary: [
            "Uso estricto de variables relativas",
          ],
        },
      },
      model_8b: {
        id: "model_8b",
        name: "Llama 3.1 8B (Local)",
        provider: "Ollama / Local",
        tier: "local_8b",
        latencyMs: 1400,
        output: {
          blockType: "hands_on_mission",
          challengeStatement: "Rellena los valores de tokens.",
          workedTemplate: `Tokens: spacing 12px, radius 8px, padding 16px.`,
          expectedMilestoneArtifact: "Completar la lista.",
          constraints: [],
          scaffoldingHints: [],
          evaluationRubricSummary: [],
        },
      },
    },
  },
  {
    id: "anchored_micro_theory",
    title: "Teoría Anclada — Cobertura de 4 Capas Pedagógicas",
    description: "Evaluar si el modelo incluye las 4 capas: Gancho Intuitivo, Límite de Analogía, Regla Invariante y Error Frecuente.",
    evaluationFocus: "Presencia completa de las 4 capas sin párrafos abstractos.",
    models: {
      model_deepseek: {
        id: "model_deepseek",
        name: "DeepSeek V4 / Chat (deepseek-chat)",
        provider: "DeepSeek",
        tier: "flagship",
        latencyMs: 430,
        output: {
          blockType: "anchored_micro_theory",
          title: "Derivación multiplicativa de densidad en UI",
          intuitiveHook: "Imagina un zoom de cámara: no necesitas rediseñar cada mueble de la habitación para alejarte, solo cambiar el factor de aumento general.",
          analogyBoundary: "El zoom cambia el tamaño de todo uniformemente; en UI, ciertos elementos (como bordes o contraste de texto) deben permanecer fijos para mantener legibilidad.",
          systemRule: "Cada token de densidad secundario DEBE calcularse multiplicando el token base mediante calc(var(--spacing-base) * multiplier). El valor numérico base solo se declara UNA sola vez.",
          frequentError: "Copiar y pegar valores absolutos calculados (ej. 12px directamente). Si la base cambia de 16px a 14px, los tokens quedarán desincronizados.",
        },
      },
      model_mimo: {
        id: "model_mimo",
        name: "Xiaomi MiMo v2.6 Pro (mimo-v2.6-pro)",
        provider: "Xiaomi",
        tier: "fast",
        latencyMs: 280,
        output: {
          blockType: "anchored_micro_theory",
          title: "Escala relativa de tokens en Design Systems",
          intuitiveHook: "Un juego de engranajes donde girar la rueda maestra mueve armónicamente todas las ruedas secundarias según su proporción de dientes.",
          analogyBoundary: "Los engranajes son rígidos y mecánicos; en CSS, el redondeo de subpíxeles puede causar desfases de 0.5px si no se usan múltiplos enteros.",
          systemRule: "1. Declarar :root { --base: 16px; }. 2. Derivar :root { --dense: calc(var(--base) * 0.75); }. 3. Consumir siempre la variable derivada en componentes.",
          frequentError: "Sustitución en vez de derivación: sobreescribir la variable en clases locales con números crudos fijos.",
        },
      },
      model_flash: {
        id: "model_flash",
        name: "Gemini 1.5 Flash",
        provider: "Google",
        tier: "fast",
        latencyMs: 390,
        output: {
          blockType: "anchored_micro_theory",
          title: "Derivación de tokens",
          intuitiveHook: "Como un zoom en un mapa digital.",
          analogyBoundary: null,
          systemRule: "Multiplica la variable base por el porcentaje de escala deseado en calc().",
          frequentError: "Escribir valores directos en los componentes.",
        },
      },
      model_pro: {
        id: "model_pro",
        name: "Gemini 1.5 Pro",
        provider: "Google",
        tier: "flagship",
        latencyMs: 950,
        output: {
          blockType: "anchored_micro_theory",
          title: "Derivación multiplicativa de densidad",
          intuitiveHook: "Como ajustar el tamaño de fuente base de un documento donde los encabezados escalan en puntos tipográficos relativos.",
          analogyBoundary: "La tipografía escala en múltiplos no lineales de lectura, mientras que el espaciado de layout requiere una cuadrícula modular fija.",
          systemRule: "El token base define la unidad fundamental. Todo token derivado debe ser una función pura de esa base.",
          frequentError: "Romper la cadena de derivación introduciendo constantes arbitrarias no declaradas en la escala.",
        },
      },
    },
  },
];

export function ModelEvaluatorTab() {
  const [selectedTaskId, setSelectedTaskId] = useState<string>("diagnostic_battery");
  const task = BENCHMARK_TASKS.find((t) => t.id === selectedTaskId) ?? BENCHMARK_TASKS[0]!;

  // Default head-to-head comparison: DeepSeek vs MiMo!
  const [modelAId, setModelAId] = useState<string>("model_deepseek");
  const [modelBId, setModelBId] = useState<string>("model_mimo");

  const [repairActiveA, setRepairActiveA] = useState<boolean>(false);
  const [repairActiveB, setRepairActiveB] = useState<boolean>(false);

  // Load configured providers from Tauri to show real status
  const [configuredProviders, setConfiguredProviders] = useState<ProviderWithStatus[]>([]);
  useEffect(() => {
    listProviders()
      .then((providers) => setConfiguredProviders(providers))
      .catch(() => {});
  }, []);

  const hasDeepSeekConfigured = configuredProviders.some((p) => p.provider === "deepseek" && p.configured);
  const hasXiaomiConfigured = configuredProviders.some((p) => p.provider === "xiaomi" && p.configured);

  const modelA = task.models[modelAId] ?? Object.values(task.models)[0]!;
  const modelB = task.models[modelBId] ?? Object.values(task.models)[1] ?? Object.values(task.models)[0]!;

  const activeOutputA = repairActiveA && modelA.repairedOutput ? modelA.repairedOutput : modelA.output;
  const activeOutputB = repairActiveB && modelB.repairedOutput ? modelB.repairedOutput : modelB.output;

  // Run evaluation reports
  const evaluate = (out: Record<string, unknown>): GroundingEvaluationReport => {
    if (task.id === "diagnostic_battery") {
      return evaluateDiagnosticQuestion(out as { stem: string; options: string[] });
    } else if (task.id === "hands_on_mission") {
      return evaluateHandsOnMissionBlock(out);
    } else {
      return evaluateAnchoredTheoryBlock(out);
    }
  };

  const reportA = evaluate(activeOutputA);
  const reportB = evaluate(activeOutputB);

  return (
    <div className="evaluator-layout">
      {/* Control Top Bar */}
      <section className="evaluator-controls card">
        <div className="evaluator-controls-header">
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
              <h3>Banco de evaluación entre modelos (DeepSeek vs MiMo Arena)</h3>
              <span className="badge" style={{ background: "rgba(127, 176, 216, 0.2)", color: "var(--blue-chalk)" }}>
                DeepSeek vs MiMo Activo
              </span>
            </div>
            <p className="muted text-xs">
              Compara cómo DeepSeek, Xiaomi MiMo y otros modelos ejecutan aserciones de contenido, sesgo de longitud (&lt; 15%) y esquemas Zod.
            </p>
          </div>
          <div className="evaluator-task-picker">
            <label className="text-xs font-semibold">Tarea de evaluación:</label>
            <select
              value={selectedTaskId}
              onChange={(e) => {
                setSelectedTaskId(e.target.value);
                setRepairActiveA(false);
                setRepairActiveB(false);
              }}
              className="evaluator-select"
            >
              {BENCHMARK_TASKS.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.title}
                </option>
              ))}
            </select>
          </div>
        </div>

        {/* Status badges for DeepSeek & Xiaomi configured in LearnKit */}
        <div style={{ display: "flex", alignItems: "center", gap: "12px", flexWrap: "wrap" }}>
          <div className="evaluator-brief-pill" style={{ flex: 1 }}>
            <strong>🎯 Criterio heurístico clave:</strong> {task.evaluationFocus}
          </div>
          <div style={{ display: "flex", gap: "8px", fontSize: "12px" }}>
            <span className={`badge ${hasDeepSeekConfigured ? "ok" : ""}`}>
              DeepSeek: {hasDeepSeekConfigured ? "Configurado en Proveedores" : "Preset de Referencia"}
            </span>
            <span className={`badge ${hasXiaomiConfigured ? "ok" : ""}`}>
              Xiaomi MiMo: {hasXiaomiConfigured ? "Configurado en Proveedores" : "Preset de Referencia"}
            </span>
          </div>
        </div>

        {/* Model Selectors */}
        <div className="evaluator-selectors-row">
          <div className="evaluator-model-select">
            <span className="badge" style={{ background: "var(--paper-deep)" }}>Modelo A</span>
            <select
              value={modelAId}
              onChange={(e) => {
                setModelAId(e.target.value);
                setRepairActiveA(false);
              }}
              className="evaluator-select"
            >
              {Object.values(task.models).map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name} ({m.provider})
                </option>
              ))}
            </select>
          </div>

          <div className="evaluator-vs-badge">VS</div>

          <div className="evaluator-model-select">
            <span className="badge" style={{ background: "var(--paper-deep)" }}>Modelo B</span>
            <select
              value={modelBId}
              onChange={(e) => {
                setModelBId(e.target.value);
                setRepairActiveB(false);
              }}
              className="evaluator-select"
            >
              {Object.values(task.models).map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name} ({m.provider})
                </option>
              ))}
            </select>
          </div>
        </div>
      </section>

      {/* Side by Side Arena Grid */}
      <div className="evaluator-arena-grid">
        {/* Model A Card */}
        <div className="evaluator-column card">
          <header className="evaluator-col-head">
            <div>
              <h4>{modelA.name}</h4>
              <span className="muted text-xs">
                {modelA.provider} · {modelA.latencyMs} ms
              </span>
            </div>
            <div className="evaluator-score-pill">
              <span className="evaluator-score-val">{reportA.overallScore}/100</span>
              <span className={`evaluator-status-tag ${reportA.passed ? "ok" : "fail"}`}>
                {reportA.passed ? "APROBADO" : "RECHAZADO"}
              </span>
            </div>
          </header>

          {/* Metric checks */}
          <div className="evaluator-metrics-list">
            {reportA.checks.map((chk) => (
              <div key={chk.id} className={`evaluator-metric-row ${chk.passed ? "pass" : "fail"}`}>
                <div className="evaluator-metric-label">
                  <span>{chk.passed ? "✅" : "⚠️"}</span>
                  <strong>{chk.label}</strong>
                </div>
                <div className="evaluator-metric-detail">{chk.detail}</div>
              </div>
            ))}
          </div>

          {/* Length disparity detail bar if applicable */}
          {reportA.lengthDisparity && (
            <div className="evaluator-disparity-box">
              <div className="evaluator-disparity-title">
                <span>Sesgo de longitud:</span>
                <strong>{reportA.lengthDisparity.variationPercent}% de variación</strong>
              </div>
              <div className="evaluator-disparity-bar-bg">
                <div
                  className={`evaluator-disparity-bar-fill ${reportA.lengthDisparity.status}`}
                  style={{ width: `${Math.min(100, reportA.lengthDisparity.variationPercent * 2.5)}%` }}
                />
              </div>
              <div className="evaluator-disparity-legend">
                <span>Mín: {reportA.lengthDisparity.minChars} car.</span>
                <span>Máx: {reportA.lengthDisparity.maxChars} car.</span>
                <span>Objetivo: &lt; 15%</span>
              </div>
            </div>
          )}

          {/* Repair Loop Trigger */}
          {modelA.repairedOutput && (
            <div className="evaluator-repair-box">
              <button
                type="button"
                className={`btn-quiet ${repairActiveA ? "active" : ""}`}
                onClick={() => setRepairActiveA(!repairActiveA)}
              >
                {repairActiveA ? "↩ Restaurar salida inicial" : "⚡ Simular The Repair Loop (Targeted Patch)"}
              </button>
              {repairActiveA && (
                <p className="hint text-xs">
                  ✅ Salida re-generada tras el patch prompt: el modelo corrigió las longitudes y la sintaxis.
                </p>
              )}
            </div>
          )}

          {/* Rendered Output Preview */}
          <div className="evaluator-output-preview">
            <h5>Salida del modelo:</h5>
            <pre className="dev-pre text-xs">{JSON.stringify(activeOutputA, null, 2)}</pre>
          </div>
        </div>

        {/* Model B Card */}
        <div className="evaluator-column card">
          <header className="evaluator-col-head">
            <div>
              <h4>{modelB.name}</h4>
              <span className="muted text-xs">
                {modelB.provider} · {modelB.latencyMs} ms
              </span>
            </div>
            <div className="evaluator-score-pill">
              <span className="evaluator-score-val">{reportB.overallScore}/100</span>
              <span className={`evaluator-status-tag ${reportB.passed ? "ok" : "fail"}`}>
                {reportB.passed ? "APROBADO" : "RECHAZADO"}
              </span>
            </div>
          </header>

          {/* Metric checks */}
          <div className="evaluator-metrics-list">
            {reportB.checks.map((chk) => (
              <div key={chk.id} className={`evaluator-metric-row ${chk.passed ? "pass" : "fail"}`}>
                <div className="evaluator-metric-label">
                  <span>{chk.passed ? "✅" : "⚠️"}</span>
                  <strong>{chk.label}</strong>
                </div>
                <div className="evaluator-metric-detail">{chk.detail}</div>
              </div>
            ))}
          </div>

          {/* Length disparity detail bar if applicable */}
          {reportB.lengthDisparity && (
            <div className="evaluator-disparity-box">
              <div className="evaluator-disparity-title">
                <span>Sesgo de longitud:</span>
                <strong>{reportB.lengthDisparity.variationPercent}% de variación</strong>
              </div>
              <div className="evaluator-disparity-bar-bg">
                <div
                  className={`evaluator-disparity-bar-fill ${reportB.lengthDisparity.status}`}
                  style={{ width: `${Math.min(100, reportB.lengthDisparity.variationPercent * 2.5)}%` }}
                />
              </div>
              <div className="evaluator-disparity-legend">
                <span>Mín: {reportB.lengthDisparity.minChars} car.</span>
                <span>Máx: {reportB.lengthDisparity.maxChars} car.</span>
                <span>Objetivo: &lt; 15%</span>
              </div>
            </div>
          )}

          {/* Repair Loop Trigger */}
          {modelB.repairedOutput && (
            <div className="evaluator-repair-box">
              <button
                type="button"
                className={`btn-quiet ${repairActiveB ? "active" : ""}`}
                onClick={() => setRepairActiveB(!repairActiveB)}
              >
                {repairActiveB ? "↩ Restaurar salida inicial" : "⚡ Simular The Repair Loop (Targeted Patch)"}
              </button>
              {repairActiveB && (
                <p className="hint text-xs">
                  ✅ Salida re-generada tras el patch prompt: el modelo corrigió las longitudes y la sintaxis.
                </p>
              )}
            </div>
          )}

          {/* Rendered Output Preview */}
          <div className="evaluator-output-preview">
            <h5>Salida del modelo:</h5>
            <pre className="dev-pre text-xs">{JSON.stringify(activeOutputB, null, 2)}</pre>
          </div>
        </div>
      </div>

      {/* Grounding Architecture Summary Card */}
      <section className="card evaluator-insights-card">
        <h4>🛡 Arquitectura de Alineación (LearnKit Grounding & Harness)</h4>
        <div className="evaluator-insights-grid">
          <div>
            <strong>1. DeepSeek (Inferencia con Razonamiento):</strong>
            <p className="text-xs muted">
              Sobresale en estructuración lógica y reglas sintácticas estrictas, generando salidas equilibradas con alta adherencia a Zod.
            </p>
          </div>
          <div>
            <strong>2. Xiaomi MiMo (Velocidad y Eficiencia):</strong>
            <p className="text-xs muted">
              Optimizado para latencia ultrabaja (&lt; 350 ms). Ideal para asistentes léxicos, feedback interactivo y turnos de generación rápida.
            </p>
          </div>
          <div>
            <strong>3. The Repair Loop (Autocorrección Quirúrgica):</strong>
            <p className="text-xs muted">
              Cuando un modelo falla una aserción heurística (&gt; 15% de sesgo de longitud), un sub-turno de parche numérico rescata la salida sin reiniciar la sesión.
            </p>
          </div>
        </div>
      </section>
    </div>
  );
}
