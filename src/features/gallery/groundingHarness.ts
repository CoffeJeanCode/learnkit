import { DynamicSectionBlockSchema, type DynamicSectionBlock } from "../../lib/schemas";

export interface LengthDisparityResult {
  minChars: number;
  maxChars: number;
  diffChars: number;
  variationPercent: number;
  passedOptimal: boolean; // < 15%
  passedSlack: boolean;   // <= 25%
  status: "optimal" | "slack_warning" | "rejected";
  message: string;
}

/**
 * Computes option length variance: (max - min) / min * 100%.
 * Industry harness rule:
 * - < 15%: Optimal (Green)
 * - 15% - 25%: Adaptive Slack / Warning (Yellow)
 * - > 25%: Rejected for length bias (Red)
 */
export function computeLengthDisparity(options: readonly string[]): LengthDisparityResult {
  if (options.length === 0) {
    return {
      minChars: 0,
      maxChars: 0,
      diffChars: 0,
      variationPercent: 0,
      passedOptimal: true,
      passedSlack: true,
      status: "optimal",
      message: "Sin opciones para evaluar",
    };
  }

  const lengths = options.map((o) => o.trim().length);
  const minChars = Math.min(...lengths);
  const maxChars = Math.max(...lengths);
  const diffChars = maxChars - minChars;
  const variationPercent = minChars > 0 ? Math.round(((maxChars - minChars) / minChars) * 1000) / 10 : 0;

  if (variationPercent < 15) {
    return {
      minChars,
      maxChars,
      diffChars,
      variationPercent,
      passedOptimal: true,
      passedSlack: true,
      status: "optimal",
      message: `Equilibrado: variación de longitud de ${variationPercent}% (< 15% umbral óptimo).`,
    };
  } else if (variationPercent <= 25) {
    return {
      minChars,
      maxChars,
      diffChars,
      variationPercent,
      passedOptimal: false,
      passedSlack: true,
      status: "slack_warning",
      message: `Variación moderada de ${variationPercent}% (entre 15% y 25% — tolerancia adaptativa con penalización leve).`,
    };
  } else {
    return {
      minChars,
      maxChars,
      diffChars,
      variationPercent,
      passedOptimal: false,
      passedSlack: false,
      status: "rejected",
      message: `Sesgo de longitud detectado: variación de ${variationPercent}% (mín ${minChars} car., máx ${maxChars} car. — límite < 15%).`,
    };
  }
}

export interface MetricCheck {
  id: string;
  label: string;
  passed: boolean;
  score: number; // 0 to 100
  detail: string;
}

export interface GroundingEvaluationReport {
  overallScore: number; // 0 - 100
  passed: boolean;
  structuralValid: boolean;
  lengthDisparity?: LengthDisparityResult;
  checks: MetricCheck[];
  suggestedRepairPrompt?: string;
}

/** Evaluates a Diagnostic Battery question payload */
export function evaluateDiagnosticQuestion(q: {
  stem: string;
  options: string[];
  correctIndex?: number;
}): GroundingEvaluationReport {
  const checks: MetricCheck[] = [];
  const options = q.options ?? [];

  // Check 1: Options count
  const countOk = options.length >= 3 && options.length <= 5;
  checks.push({
    id: "option_count",
    label: "Cantidad de distractores (3 a 5 opciones)",
    passed: countOk,
    score: countOk ? 100 : 0,
    detail: `Tiene ${options.length} opciones.`,
  });

  // Check 2: Length disparity
  const disparity = computeLengthDisparity(options);
  const disparityScore = disparity.status === "optimal" ? 100 : disparity.status === "slack_warning" ? 70 : 20;
  checks.push({
    id: "length_disparity",
    label: "Paridad de longitud (< 15% variación)",
    passed: disparity.passedSlack,
    score: disparityScore,
    detail: disparity.message,
  });

  // Check 3: Stem question clarity
  const stem = (q.stem ?? "").trim();
  const stemOk = stem.length >= 15 && (stem.includes("?") || stem.includes("¿") || stem.length >= 25);
  checks.push({
    id: "stem_clarity",
    label: "Claridad del enunciado",
    passed: stemOk,
    score: stemOk ? 100 : 40,
    detail: stemOk ? "Enunciado bien formulado." : "El enunciado es demasiado breve o ambiguo.",
  });

  const overallScore = Math.round(checks.reduce((acc, c) => acc + c.score, 0) / checks.length);
  const passed = checks.every((c) => c.passed);

  let suggestedRepairPrompt: string | undefined;
  if (!disparity.passedOptimal) {
    const avgLen = Math.round(options.reduce((a, b) => a + b.length, 0) / options.length);
    suggestedRepairPrompt = `Tu salida falló la validación heurística de sesgo de longitud:
- Mínimo: ${disparity.minChars} caracteres
- Máximo: ${disparity.maxChars} caracteres (variación ${disparity.variationPercent}% > 15%).
Reescribe ÚNICAMENTE las opciones de esta pregunta para que todas tengan entre ${Math.max(10, avgLen - 4)} y ${avgLen + 4} caracteres exactos. Mantén el significado original de cada opción.`;
  }

  return {
    overallScore,
    passed,
    structuralValid: true,
    lengthDisparity: disparity,
    checks,
    suggestedRepairPrompt,
  };
}

/** Evaluates a Hands-On Mission block */
export function evaluateHandsOnMissionBlock(block: Record<string, unknown>): GroundingEvaluationReport {
  const parsed = DynamicSectionBlockSchema.safeParse(block);
  const checks: MetricCheck[] = [];

  const structuralValid = parsed.success && parsed.data.blockType === "hands_on_mission";
  checks.push({
    id: "schema_validation",
    label: "Esquema sintáctico (Zod hands_on_mission)",
    passed: structuralValid,
    score: structuralValid ? 100 : 0,
    detail: structuralValid ? "Cumple el esquema JSON estricto." : "Error de estructura en content_json.",
  });

  if (!structuralValid) {
    return {
      overallScore: 0,
      passed: false,
      structuralValid: false,
      checks,
      suggestedRepairPrompt: "Tu respuesta no cumplió el esquema JSON requerido para 'hands_on_mission'.",
    };
  }

  const content = parsed.data as Extract<DynamicSectionBlock, { blockType: "hands_on_mission" }>;
  const template = content.workedTemplate ?? "";

  // Check cloze gaps syntax
  const gapMatches = [...template.matchAll(/___/g)];
  const hasGaps = gapMatches.length >= 1;
  checks.push({
    id: "cloze_gaps",
    label: "Huecos de práctica (sintaxis '___')",
    passed: hasGaps,
    score: hasGaps ? 100 : 0,
    detail: hasGaps ? `Contiene ${gapMatches.length} huecos para rellenar.` : "No incluye huecos '___'.",
  });

  // Check run-on text formatting
  const isRunOn = template.includes("; Fila ") || (template.length > 200 && !template.includes("\n"));
  checks.push({
    id: "template_formatting",
    label: "Formato y armonía visual (saltos de línea limpios)",
    passed: !isRunOn,
    score: isRunOn ? 40 : 100,
    detail: isRunOn ? "Texto denso sin saltos de línea (concatenado con punto y coma)." : "Estructura espaciada y legible.",
  });

  // Check expectedMilestoneArtifact
  const hasMilestone = Boolean(content.expectedMilestoneArtifact && content.expectedMilestoneArtifact.length > 10);
  checks.push({
    id: "milestone_criterion",
    label: "Criterio de éxito verificable",
    passed: hasMilestone,
    score: hasMilestone ? 100 : 30,
    detail: hasMilestone ? "Criterio de hito claro." : "Falta definición del artefacto esperado.",
  });

  const overallScore = Math.round(checks.reduce((acc, c) => acc + c.score, 0) / checks.length);
  const passed = checks.every((c) => c.passed);

  return {
    overallScore,
    passed,
    structuralValid: true,
    checks,
  };
}

/** Evaluates an Anchored Micro-Theory block */
export function evaluateAnchoredTheoryBlock(block: Record<string, unknown>): GroundingEvaluationReport {
  const parsed = DynamicSectionBlockSchema.safeParse(block);
  const checks: MetricCheck[] = [];

  const structuralValid = parsed.success && parsed.data.blockType === "anchored_micro_theory";
  checks.push({
    id: "schema_validation",
    label: "Esquema sintáctico (Zod anchored_micro_theory)",
    passed: structuralValid,
    score: structuralValid ? 100 : 0,
    detail: structuralValid ? "Cumple el esquema JSON estricto." : "Error de estructura en content_json.",
  });

  if (!structuralValid) {
    return {
      overallScore: 0,
      passed: false,
      structuralValid: false,
      checks,
    };
  }

  const content = parsed.data as Extract<DynamicSectionBlock, { blockType: "anchored_micro_theory" }>;

  // Check the 4 pedagogical layers
  const hasHook = Boolean(content.intuitiveHook?.trim());
  const hasBoundary = Boolean(content.analogyBoundary?.trim());
  const hasRule = Boolean(content.systemRule?.trim());
  const hasError = Boolean(content.frequentError?.trim());

  checks.push({
    id: "layer_hook",
    label: "Capa 1: Gancho intuitivo (Intuitive Hook)",
    passed: hasHook,
    score: hasHook ? 100 : 0,
    detail: hasHook ? "Presente" : "Falta el gancho inicial.",
  });

  checks.push({
    id: "layer_boundary",
    label: "Capa 2: Límite de la analogía (Analogy Boundary)",
    passed: hasBoundary,
    score: hasBoundary ? 100 : 30,
    detail: hasBoundary ? "Presente" : "Opcional pero recomendado para evitar sobregeneralización.",
  });

  checks.push({
    id: "layer_rule",
    label: "Capa 3: Regla del sistema (Invariant Rule)",
    passed: hasRule,
    score: hasRule ? 100 : 0,
    detail: hasRule ? "Presente" : "Falta la regla invariante técnica.",
  });

  checks.push({
    id: "layer_error",
    label: "Capa 4: Error frecuente (Frequent Misconception)",
    passed: hasError,
    score: hasError ? 100 : 0,
    detail: hasError ? "Presente" : "Falta la advertencia de error común.",
  });

  const overallScore = Math.round(checks.reduce((acc, c) => acc + c.score, 0) / checks.length);
  const passed = checks.every((c) => c.passed);

  return {
    overallScore,
    passed,
    structuralValid: true,
    checks,
  };
}
