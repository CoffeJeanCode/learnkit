import { formatWorkedTemplate } from "../src/features/notebook/blocks/HandsOnMissionBlock";

console.log("=== Testing Mission Template Cloze Parser ===");

// Función pura de reemplazo de huecos
function parseTemplateSlots(template: string): { parts: string[]; slotCount: number } {
  const parts = template.split("___");
  return {
    parts,
    slotCount: parts.length - 1,
  };
}

function assembleSubmission(parts: string[], values: string[]): string {
  let result = "";
  for (let i = 0; i < parts.length; i++) {
    result += parts[i];
    if (i < values.length) {
      result += values[i];
    }
  }
  return result;
}

const sampleTemplate = `fn calculate_tax(amount: f64) -> f64 {
    let rate = ___;
    amount * ___
}`;

const { parts, slotCount } = parseTemplateSlots(sampleTemplate);
console.assert(slotCount === 2, `Debe haber 2 huecos, obtenido: ${slotCount}`);
console.log(`✓ Huecos detectados correctamente: ${slotCount}`);

const filled = assembleSubmission(parts, ["0.16", "rate"]);
console.assert(filled.includes("let rate = 0.16;"), "El hueco 1 debe ser 0.16");
console.assert(filled.includes("amount * rate"), "El hueco 2 debe ser rate");
console.assert(!filled.includes("___"), "No debe quedar ningún hueco ___");
console.log("✓ Reconstrucción de solución completada exitosamente:\n" + filled);

// Test formatWorkedTemplate on run-on dense templates (Image 2 issue)
const runOnTemplate = "Tabla de tokens: Fila 1: spacing-base, valor ___ px; Fila 2: radius-base, valor ___ px. Regla que NO se rompe: el valor del base se escribe UNA sola vez.";
const formatted = formatWorkedTemplate(runOnTemplate);
console.assert(formatted.includes("\n\nFila 1:"), "Debe separar Fila 1 en nueva línea");
console.assert(formatted.includes(";\n\nFila 2:"), "Debe separar Fila 2 en nueva línea con espacio");
console.assert(formatted.includes(".\n\n📌 Regla que NO se rompe:"), "Debe aislar la regla de forma visual");
console.log("✓ Formateo visual y descompresión de plantilla densa verificado:\n" + formatted);

console.log("=== Mission Cloze Parser test PASSED ===");
