# Generative UI de bloques: velocidad, personalización y módulos interactivos

Plan por fases (0 → 4; **sin HTML libre en iframe**). Estado al cierre de la sesión del 2026-11-02.
Origen: revisión del pipeline de bloques + comparación con Google (Generative UI, código libre) y OpenAI
(módulos curados y parametrizables). Principio: el LLM emite un JSON pequeño y validado; el frontend
contiene el modelo matemático, el dibujo y los controles.

## Cómo verificar (Windows)

- **Frontend:** `bun node_modules/typescript/bin/tsc --noEmit` y los `bun scripts/check-*.ts(x)` listados abajo.
  (`npx tsc` NO funciona aquí.)
- **Rust:** el exe de tests muere con `STATUS_ENTRYPOINT_NOT_FOUND` si no lleva el manifest de `comctl32 v6`.
  Solución: compilar el test con `cargo rustc --lib --profile test -- -Clink-arg=/MANIFEST:EMBED -Clink-arg=/MANIFESTINPUT:<test.manifest>`
  (con `CARGO_TARGET_DIR=target-check`) y ejecutar el `.exe` resultante en `target-check\debug\deps\learnkit_lib-*.exe`.
  El manifest solo declara la dependencia `Microsoft.Windows.Common-Controls 6.0.0.0` + `supportedOS`.
- Reporte de tamaño del prompt: filtro `size_report` con `--ignored --nocapture`.
- Heredocs de bash con Python y comillas fallan: escribir el script a un archivo y ejecutarlo.
- Los archivos del repo están en LF con `autocrlf`; los scripts de parche deben leer/escribir con `newline=""`.

Último resultado completo de Rust: **557 passed, 0 failed, 1 ignored (size_report)**.
Frontend: TypeScript 0 errores (`tsc --noEmit`) y los 5 scripts de verificación OK.

## Hecho

### Fase 0: línea base
- [x] 0.1 `src/lib/blockTiming.ts` + `scripts/report-block-timing.ts` + `scripts/check-block-timing.ts`:
  latencia real por bloque desde el dev-trace (`block_generation_timing`). Uso: exportar entradas a JSON y
  `bun scripts/report-block-timing.ts trace.json`.
- [x] `scripts/eval-block-models.ts`: avisa que `--dry-run` **simula** latencia/TTFT/tokens (no es línea base).
- [x] Tamaño de prefijo del generador medido: sistema 45.5 KB (~11.4k tok) + herramienta 21.4 KB (~5.3k tok) =
  ~16.7k tok/llamada (ya con las variantes visuales nuevas). Tras el cambio: completo 47.5 KB.
- [x] 0.2 `src/lib/visualQuality.ts` + `scripts/check-visual-quality.ts`: solapamiento de textos, textos sin
  pill, props kebab-case, colores sin contraste, aristas colgantes.
- [ ] **Pendiente:** capturar una línea base REAL de latencia/reintentos con una sesión viva (necesita API key)
  y guardarla aquí para comparar.

### Fase 1
- [x] 1.1 Esquema de herramienta (`tools/notebook_tools.rs::visual_aid_json_schema`): `interactive_module`,
  `diagram_spec`, `conceptual_matrix`, `declarative_svg`, `mermaid` en orden de preferencia. Prompt con la
  nueva sección `P_VISUALS`. Validación reforzada de `diagram_spec` en `grounding.rs`.
- [x] 1.2 Preview en vivo ampliado (`tools/block_stream.rs`): `PREVIEW_FIELDS` por tipo + `PREVIEW_TEMPLATES`
  para `tpl_*` (antes no mostraban nada). Tests de no-fuga de respuestas. Frontend: `BlockDraftPreview.tsx`.
- [x] 1.3 Etiquetas clicables: `DiagramNodeSpec.explanation` (Rust + zod), panel al tocar un nodo y spotlight del
  paso del walkthrough en `StaticVisual.tsx` (Mermaid). CSS en `styles.css`.

### Fase 2
- [x] 2.1 Prompt dividido en secciones (`agents/block_generator_agent.rs`): `full_prompt()`, `lean_prompt(tipo)`,
  `system_prompt_for(Option<tipo>)`. `orchestrator.run_notebook_block_agent_with(..., override, ...)`.
  Intento 1 con prompt corto si el plan fija el tipo; los reintentos vuelven al completo.
- [x] 2.2 `domain/visual_repair.rs` (nuevo): kebab→camel, pill de fondo para `text`, viewBox, ids de
  `diagram_spec` y aristas, sliders fuera de rango. Se aplica en `generate()` antes de la crítica.
- [x] 2.3 **Prefetch especulativo** del bloque siguiente mientras el estudiante resuelve la compuerta (`generation.rs` con clave `(document_id, gate_block_id)`, guard `insert_block_if_count`, `with_prefetch`, tests en `generation::tests` y `notebook_service::tests`).

### Fase 3
- [x] 3.1 `domain/block_planner.rs` (nuevo): `plan_next` fija tipo/modo/`supportLevel`/`isTransfer` cuando la
  posición está determinada (retrieval, transferencia, cierre, chequeo antes de apilar teoría, regla de oro,
  cadena del perfil, maestría pendiente). `render::composition_suggestion` expuesto. Hint `PLAN_DEL_SISTEMA`
  añadido al input.
- [x] 3.2 `Presentation {density, emphasis, layout}` decidida por el sistema y guardada en
  `content_json.presentation`. Frontend: `presentationOf` (schemas.ts), `data-density/-emphasis/-layout` en
  `blocks/index.tsx`, CSS adaptable. `scripts/check-presentation.ts`.

### Fase 4: módulos interactivos (enfoque OpenAI)
- [x] 4.1 `domain/safe_expr.rs` + `src/lib/safeExpr.ts` (evaluador sin `eval`, misma gramática, tests en ambos).
  `domain/interactive_module.rs` (validación: fórmulas, símbolos, rangos, finitud, retos alcanzables).
  Tipos `InteractiveModuleSpec` en `domain/notebook.rs` y zod en `schemas.ts`.
- [x] 4.2/4.3 `src/features/notebook/blocks/InteractiveModule.tsx`: `relation_explorer`, `function_plot`
  (con tangente), `vector_sum` (punta a cola), `particle_gradient`, `timeline`. `scripts/check-interactive-module.tsx`.
- [x] 4.4 (parcial) Compuerta de predicción **bloquea** el módulo hasta responder (`locked`); `challenge`
  verificado en cliente (feedback, no bloquea).

### Arreglos ajenos al plan (estaban rotos en el árbol de trabajo)
- Tests con `NotebookBlock { .. }` sin `concept_node_id` (6 sitios) y `stream_notebook_block` con 6 argumentos
  (`factory/tests.rs`). Se añadió `concept_node_id: None` y `capture` para que compile la suite.

## Pendiente (en orden)

1. [x] **Compilar y correr todo**: 557 tests de Rust pasando, 0 fallos, 0 errores en frontend `tsc --noEmit` y scripts `check-*`.
2. [x] **Tests de la regla `declarative_svg` solo en `declarative_visual_diagram`** en `grounding.rs`
   (`declarative_svg_is_only_allowed_in_declarative_visual_diagram` y `broken_interactive_module_is_rejected_by_single_block_violations` probados y pasando).
3. [x] **Probar el hook del plan en `generation.rs`** con el orquestador mock
   (`planned_block_uses_lean_prompt_on_first_attempt_and_full_prompt_on_retry` probado y pasando).
4. [x] **`size_report`**: añadidas filas `notebook_generator [lean:<tipo>]`. Ahorro medido en tokens de sistema:
   - `anchored_micro_theory`: 5.118 tok vs 11.882 tok (-56,9%, 43,1% del completo).
   - `interactive_prediction_gate`: 5.909 tok vs 11.882 tok (-50,3%, 49,7% del completo).
   - `heuristic_error_audit`: 5.319 tok vs 11.882 tok (-55,3%, 44,7% del completo).
   - `hands_on_mission`: 5.675 tok vs 11.882 tok (-52,3%, 47,7% del completo).
   - `branching_scenario_challenge`: 5.503 tok vs 11.882 tok (-53,7%, 46,3% del completo).
   - `declarative_visual_diagram`: 6.731 tok vs 11.882 tok (-43,4%, 56,6% del completo).
   - `metacognitive_closure`: 4.687 tok vs 11.882 tok (-60,6%, 39,4% del completo).
   - `spaced_interleaved_retrieval`: 4.635 tok vs 11.882 tok (-61,0%, 39,0% del completo).
5. [x] **2.3 Prefetch especulativo**: generación separada de persistencia (`generate_candidate_block` + `persist_candidate_block`).
   Disparado al mostrar una compuerta si `prefetch` está activo (`with_prefetch` o `LEARNKIT_PREFETCH=1`).
   Candidato en memoria con clave `(document_id, gate_block_id)` y conteo esperado. En `submit_gate_response`, si pasa,
   se inserta de inmediato devolviendo `next_block` en `GateResult`; si falla o caduca, se descarta.
   Tests en `generation::tests` y `notebook_service::tests::speculative_prefetch_serves_next_block_on_gate_pass_and_discards_on_fail`.
6. **4.4 completo (opcional):** calificar una compuerta por el **estado del módulo** (server-side). Requiere
   variante nueva en `GateSubmission` + evaluar la fórmula en Rust (`safe_expr` ya sirve) + redacción del objetivo.
7. **Revisión visual real:** abrir la app (`bun run tauri dev`) y revisar a ojo `InteractiveModule`, las etiquetas
   clicables y la presentación `split`/`spacious` (solo se probó con `renderToStaticMarkup` y tsc).
8. **Calidad real del modelo:** con API key, generar clases de física/biología/cálculo y confirmar que el modelo
   elige `interactive_module` / `diagram_spec` y que ya no cae en `declarative_svg`. Ajustar `P_VISUALS` si no.
9. **Medir:** comparar latencia/intentos por tipo antes y después con `scripts/report-block-timing.ts`.

## Fuera de alcance (decidido)
- HTML libre en `iframe` (Fase 5): no se integra.

## Archivos tocados / nuevos (resumen)
Rust nuevos: `domain/{safe_expr,interactive_module,block_planner,visual_repair}.rs`.
Rust editados: `domain/{notebook,mod,pedagogy_guardrails}.rs`, `application/notebook_service/{grounding,generation,render}.rs`,
`agents/block_generator_agent.rs`, `orchestration/orchestrator.rs`, `tools/{notebook_tools,block_stream}.rs`.
Frontend nuevos: `lib/{safeExpr,blockTiming,visualQuality}.ts`, `features/notebook/blocks/InteractiveModule.tsx`.
Frontend editados: `lib/schemas.ts`, `blocks/{StaticVisual,InteractivePredictionGateBlock,index}.tsx`, `BlockDraftPreview.tsx`, `styles.css`.
Scripts nuevos: `check-{safe-expr,interactive-module,block-timing,visual-quality,presentation}`, `report-block-timing`.

Nota: el árbol de trabajo ya tenía cambios sin confirmar del usuario (lesson_graph, comentarios, session observer, etc.);
nada de lo anterior se ha confirmado (commit) todavía.
