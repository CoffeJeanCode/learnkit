# Remediación de brechas — auditoría tecnopedagógica

File locator: `odd/tasks/pedagogical-audit-remediation.md` (repo-relative, C:\bi\learnkit)

## Objective
Implementar las 7 brechas reales identificadas en la auditoría tecnopedagógica de las 4 fases de LearnKit (diagnóstico → roadmap → syllabus → notebook), entregada previamente como informe + propuesta (sin tocar código). El usuario aprobó implementar ahora.

## Why
Auditoría solicitada por el usuario 2026-09-30. La mayoría de los "vicios genéricos de LLM" del brief ya estaban erradicados en el codebase real (gating en máquina de estados Rust, atajo nivel-cero, paridad de opciones server-side, composición dinámica de bloques, buffering progresivo, etc. — ver engram obs #825). Quedan 7 brechas puntuales, documentadas con cita `file:line`.

## Scope
Gap #7 (validación de sintaxis matemática antes de renderizar) queda **fuera de alcance**: no fue confirmado como brecha real (ninguno de los 4 exploradores lo auditó), solo se declaró como hueco de cobertura. No se implementa nada para un hallazgo no confirmado.

## Constraints
- UI copy en español, comentarios en código en inglés (convención del repo).
- Conventional commits, sin co-author.
- No hay TDD configurado (no hay test runner TS; Rust usa `#[test]` colocado / `tests.rs` por módulo — se sigue ese patrón existente para lógica nueva no trivial, sin RED-first estricto).
- RDD (receipt-driven development) está `on` por default en este repo — se corre `gentle-ai review assess` tras cada commit de work-unit.
- Rama nueva `fix/pedagogical-audit-gaps` creada desde `main` (trabajo no relacionado con `feature/theory-selection-popover`).

## Tasks

- [x] **T1 — Fase 1: paridad de opciones también en TS** (gap 1)
  Ruta: directa inline (1 archivo, mecánico).
  Commit: `b9959bd` — `feat(schemas): mirror option-length-parity guardrail in TS`.
  Aceptación: `tsc --noEmit` limpio; el refine rechaza opciones con variación >15%. Revisado y reconocido (lineage `review-f6c544c74546cedb`, aprobado, 3 hallazgos informativos no bloqueantes: empty-options edge case, sin test runtime, parity no verificable en el mismo diff).

- [x] **T2 — Fase 2: campo capstone/proyecto terminal** (gap 2)
  Ruta: delegada (11 archivos, incluyendo fixtures de test).
  Commit: `c9292f3` — `feat(roadmap): require explicit capstone project per roadmap`.
  Aceptación cumplida: `capstoneProject` obligatorio en `RoadmapSyllabusPackage`, exigido en el prompt Y en el JSON schema real del tool call (`roadmap_tools.rs`), validado por `capstone_project_violations`; `cargo build`+`cargo test` (177 passed) limpios; `tsc --noEmit` limpio.

- [x] **T3 — Fase 2: persistencia antes del árbol de hitos** (gap 3)
  Ruta: delegada.
  Commit: `375a177` — `fix(roadmap): persist learner profile card before generating milestones`.
  Aceptación cumplida: write intermedio en la rama `absolute_zero` justo antes de la llamada encadenada a Gate 3a; test de inyección de fallos (`SucceedOnceThenFailRunner`) prueba que `learner_profile_card` sobrevive si Gate 3a muere; 179 passed.

- [x] **T4 — Fase 3: deliverable tipado + unificación de nombres + fix comentario obsoleto** (gaps 4 + 5)
  Ruta: delegada (15 archivos).
  Commit: `2014406` — `feat(roadmap): type micromodule deliverables and unify milestone field naming`.
  Aceptación cumplida: `Micromodule.deliverable` → `{artifactType, description}` con validación estructural (denylist reemplazado); `deliverable_goal` → `deliverable` unificado en `SyllabusMilestone`/`SyllabusMilestoneSchema`; comentario obsoleto `≤5h` → `≤4h` corregido; `Milestone.deliverable` (rollup semanal) dejado intacto a propósito; 180 passed.
  **Corrección post-revisión** (commit `9f526d3` — `fix(roadmap): keep pre-upgrade string deliverables deserializable`): la revisión nativa detectó CRITICAL — el cambio `String`→struct no tenía ruta de deserialización compatible hacia atrás (a diferencia de `CapstoneProject`, que sí la recibió), rompiendo la carga de sesiones ya persistidas. Se añadió un `Deserialize` manual que acepta ambas formas. 189 passed tras el fix.

- [x] **T5 — Fase 4: tabla de composición tipada** (gap 6)
  Ruta: delegada (4 archivos nuevos/tocados).
  Commit: `5b58361` — `feat(notebook): formalize block composition profiles as typed data`.
  Aceptación cumplida parcialmente: `DisciplineProfile` + `suggested_sequence()` en `domain/lesson_composition.rs` (5 perfiles transcritos del prompt, 7 tests unitarios), mirror TS `DisciplineProfileSchema`. **No wireado** a `notebook_service/grounding.rs` como validación de secuencia completa (decisión deliberada del agente: los perfiles son "disposiciones de partida, no moldes" — un guardrail rígido rechazaría desviación legítima). Hallazgo de revisión (no bloqueante) registra esto como gap de cobertura a futuro, no como defecto.

## Review (RDD)
Todo el trabajo quedó cubierto por una revisión nativa aprobada y reconocida:
- T1 solo: lineage `review-f6c544c74546cedb` — aprobado, reconocido.
- T2+T3+T4 (657 líneas, disparó `slice_budget_reached`): lineage `review-05461d95f2a7cecb` inició, encontró 1 CRITICAL real (ver corrección de T4 arriba), y tras la corrección un commit concurrente de T5 cambió el alcance del target (`scope_changed`) — la lineage se abandonó limpiamente (`gentle-ai review abandon`, razón `operator_disposition`) en vez de forzar una recuperación ambigua.
- T1-T5 consolidados: lineage `review-07f99703a2383079` — aprobado, reconocido. 5 hallazgos no bloqueantes, todos brechas de cobertura de test (no regresiones): render.rs key rename sin test de forma JSON, superRefines de TS sin test runtime, tabla de composición no wireada, capstone legacy-default sin test de migración.

## Follow-ups (no implementados, no bloquean esta entrega)
1. Test runtime TS para los 2 nuevos `superRefine` (paridad de opciones, deliverable "other").
2. Test de deserialización legacy para `RoadmapSyllabusPackage` sin `capstoneProject`.
3. Test que inspeccione el JSON emitido por `render.rs` tras el rename `deliverableGoal`→`deliverable`.
4. Wireo opcional de `lesson_composition` como validación de secuencia en `notebook_service/grounding.rs`.
5. Gap #7 original (sintaxis matemática antes de renderizar) sigue sin auditar.

## Delivery strategy
`ask-on-risk`. Se preguntó una vez al superar el presupuesto de ~400 líneas (slice T2+T3+T4, 657 líneas) — el usuario ya había aprobado revisión candidato-a-candidato vía los consentimientos de RDD, no se pidió estrategia de cadena de PRs porque no se solicitó push/PR en esta sesión.
