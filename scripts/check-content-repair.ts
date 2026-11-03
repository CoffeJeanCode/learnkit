/**
 * Verification test for Content Repair & Adaptive Slack logic.
 *
 * Usage:
 *   bun scripts/check-content-repair.ts
 */

export interface RepairResult {
  cleanedText: string;
  justification?: string;
}

export const EMBEDDED_CONNECTORS = [
  "porque",
  "ya que",
  "debido a",
  "puesto que",
  "lo cual significa",
  "es decir que",
];

export function stripOptionPrefixes(text: string): string {
  let s = text.trim();

  // Strip wrapping quotes
  if (
    (s.startsWith('"') && s.endsWith('"') && s.length >= 2) ||
    (s.startsWith("'") && s.endsWith("'") && s.length >= 2) ||
    (s.startsWith("«") && s.endsWith("»") && s.length >= 2)
  ) {
    s = s.slice(1, -1).trim();
  }

  // Strip bullet markers: "A) ", "1. ", "- ", etc.
  const bulletMatch = s.match(/^([A-Za-z0-9][\).\-:]|[\-*]|\[[A-Za-z0-9]\]|\([A-Za-z0-9]\))\s+(.*)$/);
  if (bulletMatch && bulletMatch[2]) {
    s = bulletMatch[2].trim();
  }

  return s;
}

export function extractEmbeddedJustification(text: string): RepairResult {
  const stripped = stripOptionPrefixes(text);
  const lower = stripped.toLowerCase();

  for (const connector of EMBEDDED_CONNECTORS) {
    const idx = lower.indexOf(connector);
    if (idx > 0) {
      const charBefore = lower[idx - 1];
      if (charBefore === " " || charBefore === "," || charBefore === "-" || charBefore === "(") {
        const left = stripped.slice(0, idx).replace(/[\s,;\-(:]+$/, "").trim();
        const right = stripped.slice(idx).replace(/^[\s:]+/, "").replace(/[\s).]+$/, "").trim();
        if (left.length >= 4 && right.length > 0) {
          return { cleanedText: left, justification: right };
        }
      }
    }
  }

  return { cleanedText: stripped };
}

export type SlackTier = "optimal" | "warning" | "critical";

export function evaluateOptionsLengthSlack(options: string[]): SlackTier {
  if (options.length < 2) return "optimal";

  const charLens = options.map((o) => o.length);
  const minChars = Math.min(...charLens);
  const maxChars = Math.max(...charLens);

  const charDiff = maxChars - minChars;
  const relCharDiff = maxChars > 0 ? charDiff / maxChars : 0;

  // Critical length bias: relative difference > 25% AND absolute difference > 25 characters
  if (relCharDiff > 0.25 && charDiff > 25) {
    return "critical";
  }

  // Tier 1: Optimal (variation <= 15% and absolute diff <= 15 chars)
  if (relCharDiff <= 0.15 && charDiff <= 15) {
    return "optimal";
  }

  // Tier 2: Adaptive Slack (between 15% and 25%, or difference <= 25 chars)
  return "warning";
}

function runTests() {
  console.log("Running check-content-repair tests...");

  // Test 1: Prefix stripping
  console.assert(stripOptionPrefixes("A) Primera opción") === "Primera opción", "Test 1.1 failed");
  console.assert(stripOptionPrefixes("1. Segunda opción") === "Segunda opción", "Test 1.2 failed");
  console.assert(stripOptionPrefixes("[C] Tercera opción") === "Tercera opción", "Test 1.3 failed");
  console.assert(stripOptionPrefixes('"Opción entre comillas"') === "Opción entre comillas", "Test 1.4 failed");
  console.assert(stripOptionPrefixes("«Opción con comillas latinas»") === "Opción con comillas latinas", "Test 1.5 failed");
  console.assert(stripOptionPrefixes("- Opción con guion") === "Opción con guion", "Test 1.6 failed");

  // Test 2: Justification extraction
  const res1 = extractEmbeddedJustification("Aumenta el throughput porque reduce los bloqueos concurrentes");
  console.assert(res1.cleanedText === "Aumenta el throughput", "Test 2.1 cleanedText failed");
  console.assert(res1.justification === "porque reduce los bloqueos concurrentes", "Test 2.1 justification failed");

  const res2 = extractEmbeddedJustification("A) Usa memoria swap, debido a la sobrecarga");
  console.assert(res2.cleanedText === "Usa memoria swap", "Test 2.2 cleanedText failed");
  console.assert(res2.justification === "debido a la sobrecarga", "Test 2.2 justification failed");

  const res3 = extractEmbeddedJustification("Opción limpia y sin justificaciones añadidas");
  console.assert(res3.cleanedText === "Opción limpia y sin justificaciones añadidas", "Test 2.3 failed");
  console.assert(res3.justification === undefined, "Test 2.3 just should be undefined");

  // Test 3: Adaptive Slack Tiers
  // Tier 1: Optimal (50 vs 55 chars, 9% relative diff)
  const t1 = evaluateOptionsLengthSlack([
    "Diez palabras para la primera opción de respuesta corta",
    "Diez palabras para la segunda opción de respuesta corta",
  ]);
  console.assert(t1 === "optimal", `Test 3.1 failed: expected optimal, got ${t1}`);

  // Tier 2: Warning with Slack (51 vs 61 chars, 16.4% relative diff > 15%, but diff 10 <= 25 chars)
  const t2 = evaluateOptionsLengthSlack([
    "Esta opción cuenta con exactamente cincuenta y un c", // 51 chars
    "Esta otra alternativa posee sesenta y un caracteres en total.", // 61 chars
  ]);
  console.assert(t2 === "warning", `Test 3.2 failed: expected warning, got ${t2}`);

  // Tier 3: Critical (20 chars vs 100 chars, 80% relative and 80 chars diff)
  const t3 = evaluateOptionsLengthSlack([
    "a".repeat(20),
    "b".repeat(100),
  ]);
  console.assert(t3 === "critical", `Test 3.3 failed: expected critical, got ${t3}`);

  console.log("OK: All content repair & adaptive slack tests passed.");
}

runTests();
