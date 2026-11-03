import { useSessionObserver } from "../src/features/notebook/sessionObserver";
import type { NotebookBlock } from "../src/lib/schemas";

console.log("=== Testing Session Observer ===");

const store = useSessionObserver.getState();

// 1. Iniciar sesión
store.initSession("class-test-01", "xiaomi", "mimo-v2.6-pro");
const s1 = useSessionObserver.getState().currentSession;
console.assert(s1 !== null, "Debe haber una sesión activa");
console.assert(s1?.model === "mimo-v2.6-pro", "Modelo debe ser mimo-v2.6-pro");
console.log("✓ Sesión inicializada correctamente");

// 2. Registrar generating para bloque 0
store.recordGenerating(0, 1);
let cur = useSessionObserver.getState().currentSession!;
console.assert(cur.blocks.length === 1, "Debe tener 1 bloque en telemetría");
console.assert(cur.blocks[0].ttftMs === null, "TTFT debe ser null inicialmente");
console.log("✓ Bloque 0 en estado generating registrado");

// 3. Simular primer delta tras 10ms
setTimeout(() => {
  store.recordFirstDelta();
  cur = useSessionObserver.getState().currentSession!;
  console.assert(cur.blocks[0].ttftMs !== null && cur.blocks[0].ttftMs >= 0, "TTFT debe haber sido registrado");
  console.log(`✓ Primer delta registrado (TTFT = ${cur.blocks[0].ttftMs}ms)`);

  // 4. Registrar tokens y bloque ready
  store.recordTokenUsage({ input: 1200, cached: 800, reasoning: 0, output: 140 });

  const mockBlock: NotebookBlock = {
    id: "block-test-0",
    document_id: "doc-1",
    order_index: 0,
    block_type: "anchored_micro_theory",
    content_json: { title: "Test Concept" },
    status: "ready",
    attempt_count: 0,
    last_feedback: null,
    annotations: null,
  };

  store.recordReady(mockBlock);
  cur = useSessionObserver.getState().currentSession!;
  console.assert(cur.blocks[0].totalDurationMs !== null, "Duración total debe estar registrada");
  console.assert(cur.blocks[0].blockType === "anchored_micro_theory", "Tipo de bloque debe ser anchored_micro_theory");
  console.assert(cur.blocks[0].tokens.reasoning === 0, "Tokens de reasoning deben ser 0");
  console.log(`✓ Bloque ready registrado (Duración = ${cur.blocks[0].totalDurationMs}ms, Out = ${cur.blocks[0].tokens.output})`);

  // 5. Exportar reporte markdown
  const md = store.exportMarkdown();
  console.assert(md.includes("mimo-v2.6-pro"), "Markdown debe incluir el modelo");
  console.assert(md.includes("anchored_micro_theory"), "Markdown debe incluir el bloque");
  console.log("✓ Reporte Markdown generado exitosamente:\n");
  console.log(md);
  console.log("=== All Session Observer tests PASSED ===");
}, 20);
