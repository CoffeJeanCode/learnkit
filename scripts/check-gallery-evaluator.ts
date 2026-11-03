import {
  computeLengthDisparity,
  evaluateDiagnosticQuestion,
  evaluateHandsOnMissionBlock,
  evaluateAnchoredTheoryBlock,
} from "../src/features/gallery/groundingHarness";
import {
  findSnapTarget,
  findSupportedBricks,
  isValidPlacement,
  getGridGeometry,
  detectAssembledGridSlot,
} from "../src/features/notebook/rewards/legoSnap";

console.log("=== Testing Grounding Harness ===");

// 1. Length disparity test
const balancedOptions = [
  "Centraliza el origen del cambio para toda la jerarquía de UI.", // 61
  "Permite escalar el valor origen automáticamente en toda la app.", // 63
  "Garantiza que la densidad escale sin mutar el archivo principal.", // 63
];
const balancedResult = computeLengthDisparity(balancedOptions);
console.log("Balanced options variance:", balancedResult.variationPercent, "% -", balancedResult.status);
if (balancedResult.status !== "optimal") throw new Error("Expected optimal variance");

const biasedOptions = [
  "un token CSS directo", // 20
  "declaración inline directa en el atributo style del nodo DOM", // 60
];
const biasedResult = computeLengthDisparity(biasedOptions);
console.log("Biased options variance:", biasedResult.variationPercent, "% -", biasedResult.status);
if (biasedResult.status !== "rejected") throw new Error("Expected rejected variance");

// 2. Diagnostic question evaluation test
const diagReport = evaluateDiagnosticQuestion({
  stem: "¿Por qué un token de densidad derivado debe usar una fórmula multiplicativa?",
  options: balancedOptions,
});
console.log("Diagnostic question report score:", diagReport.overallScore, "passed:", diagReport.passed);
if (!diagReport.passed) throw new Error("Diagnostic report should have passed");

// 3. LEGO Snap and Grid Geometry test
console.log("\n=== Testing LEGO Assembly & Snap System ===");
const geom = getGridGeometry(600, 230);
console.log("Grid geometry:", geom);
if (geom.numCols !== 20) throw new Error("Expected 20 columns for width 600 with BRICK_UNIT 30");

const snap = findSnapTarget(152, 210, { cols: 2, rows: 1 }, 600, 230, []);
console.log("Floor snap target found:", snap);
if (!snap || snap.layerBottom !== 0) throw new Error("Expected floor snap at layer 0");

const stacked = findSnapTarget(snap.x, snap.y - 27, { cols: 2, rows: 1 }, 600, 230, [
  { id: "b1", cols: 2, rows: 1, cStart: snap.cStart, layerBottom: 0 },
]);
console.log("Stack snap target found:", stacked);
if (!stacked || stacked.layerBottom !== 1) throw new Error("Expected stacked snap at layer 1");

// Test support graph
const supported = findSupportedBricks([
  { id: "b1", cols: 2, rows: 1, cStart: 4, layerBottom: 0 },
  { id: "b2", cols: 2, rows: 1, cStart: 4, layerBottom: 1 },
]);
console.log("Supported bricks:", [...supported]);
if (!supported.has("b1") || !supported.has("b2")) throw new Error("Both b1 and b2 should be supported");

console.log("\n✅ ALL TESTS PASSED SUCCESSFULLY!");
