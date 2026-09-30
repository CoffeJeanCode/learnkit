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

- [ ] **T1 — Fase 1: paridad de opciones también en TS** (gap 1)
  Ruta: directa inline (1 archivo, mecánico).
  Archivo: `src/lib/schemas.ts` — `superRefine` en `DiagnosticQuestionSchema` replicando la regla `<15%` de `grounding.rs:291-298`.
  Aceptación: `tsc --noEmit` limpio; el refine rechaza opciones con variación >15%.

- [ ] **T2 — Fase 2: campo capstone/proyecto terminal** (gap 2)
  Ruta: delegada (4 archivos no triviales).
  Archivos: `src-tauri/src/domain/roadmap.rs`, `src-tauri/src/agents/roadmap_agent.rs`, `src-tauri/src/application/roadmap_service/grounding.rs`, `src/lib/schemas.ts`.
  Aceptación: `RoadmapSyllabusPackage` tiene `capstone_project` obligatorio; prompt lo exige; grounding lo valida (no vacío); `cargo build` + `cargo test` limpios; `tsc --noEmit` limpio.

- [ ] **T3 — Fase 2: persistencia antes del árbol de hitos** (gap 3)
  Ruta: delegada (cambio de orden en máquina de estados, no trivial).
  Archivo: `src-tauri/src/application/roadmap_service.rs` (`advance()`).
  Aceptación: `LearnerProfileCard`/`DiagnosticSummaryCard` se persisten en un write propio antes de que Gate 3a genere el árbol de hitos; `cargo test` limpio (sin regresión en flujo de sesión).

- [ ] **T4 — Fase 3: deliverable tipado + unificación de nombres + fix comentario obsoleto** (gaps 4 + 5)
  Ruta: delegada (múltiples archivos, cambio de esquema con migración).
  Archivos: `src-tauri/src/domain/roadmap.rs`, `src-tauri/src/domain/notebook.rs`, `src-tauri/src/application/roadmap_service/grounding.rs`, `src-tauri/src/agents/roadmap_agent.rs`, `src/lib/schemas.ts`, `src/components/MicromoduleItem.tsx`.
  Aceptación: `deliverable` pasa de `String` libre a `{artifact_type, description}`; denylist de frases vagas se reemplaza por validación de tipo; naming unificado (`deliverable` en ambos sistemas de tipos, eliminando `deliverable_goal`); comentario obsoleto en `notebook.rs:567` corregido; `cargo build`+`cargo test` limpios; `tsc --noEmit` limpio.

- [ ] **T5 — Fase 4: tabla de composición tipada** (gap 6)
  Ruta: delegada (archivo nuevo + wiring).
  Archivos: `src-tauri/src/domain/lesson_composition.rs` (nuevo), `src-tauri/src/agents/block_generator_agent.rs` (referencia a la tabla), `src-tauri/src/application/notebook_service/grounding.rs` (validar secuencia completa contra la tabla), `src/lib/schemas.ts` (mirror TS).
  Aceptación: perfiles disciplinares y sus secuencias existen como datos testeables, no solo prosa de prompt; `cargo test` cubre al menos un perfil; sin regresión de comportamiento (la selección dinámica ya funcionaba, esto la formaliza).

## Delivery strategy
`ask-on-risk` (default). Forecast inicial ~600-900 líneas autoría (T2-T5 tocan tipos + prompts + validación + mirror TS cada una). Se preguntará por estrategia de cadena de PRs solo si se supera el presupuesto de ~400 líneas por slice.

## Progress
(se actualiza por tarea)
