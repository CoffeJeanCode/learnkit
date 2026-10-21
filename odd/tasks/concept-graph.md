# Grafo de ideas del estudiante (reemplaza Memoria y Capacidades)

## Objective
Mostrar un grafo en el que el **estudiante** crea los conceptos y las relaciones (con sus propias palabras), como paso adicional que ayuda a construir conocimiento. Quitar los botones «🧠 Memoria» y «🗺 Capacidades» del header.

## Why
User request (2026-10-21, es): 1) mostrar el grafo y quitar los botones de memoria y capacidades; 2) las relaciones las hace el usuario, permitiendo metacognición; 3) la IA no hace relaciones: es un paso adicional para crear conocimiento.

Decisiones tomadas con el usuario: nodos = clases + conceptos que escribe el estudiante; botón «🕸 Grafo» en el header; se borran las vistas viejas y se conserva el backend (memoria cognitiva y evidencias siguen alimentando generación y repaso).

## Design (settled)
- Dos tablas aditivas en SQLite (`concepts`, `concept_links`, `CREATE TABLE IF NOT EXISTS`). Los extremos de una relación son un id de clase o de concepto del MISMO curso (validado en el servicio). Borrar un concepto borra sus relaciones; borrar el curso limpia todo por cascada.
- La relación exige texto del estudiante (≤ 120 caracteres): tener que decir *por qué* es la parte metacognitiva. Los chips («causa», «es parte de»…) solo precargan el campo.
- **La IA no participa**: ningún comando llama a un modelo y nada del grafo entra a `render.rs`, `generation.rs` ni `grading.rs` (hay un test que lo vigila).
- Vista `graph` con SVG propio (sin dependencias nuevas), layout determinista por semana (`features/graph/graph.ts`), lista de relaciones accesible bajo el dibujo.
- Paso opcional tras un cierre aprobado (`ConnectStep`): escribir 1-3 ideas y conectarlas con clases/conceptos ANTERIORES. No bloquea nada ni cuenta como evidencia.

## Tasks
- [x] T1 — Backend: tablas, tipos, store, servicio, 5 comandos (`get_concept_graph`, `save_concept`, `delete_concept`, `save_concept_link`, `delete_concept_link`) + tests de validación, cascada y «el grafo nunca llega a un prompt».
- [x] T2 — Frontend: esquemas Zod, wrappers, store `graph`, `GraphView`/`GraphParts`, layout puro.
- [x] T3 — Paso `ConnectStep` en `MetacognitiveClosureBlock`.
- [x] T4 — Limpieza: botones Memoria/Capacidades, vistas, flag `showCapabilityMap`, `check-capability-map.ts`, CSS `.cap-*`, tipos huérfanos de `tauri.ts`, fila de `docs/estudio-ab.md`.
- [x] T5 — Tests: `scripts/check-concept-graph.ts` (en Makefile y CI), ampliación de `check-rendering.tsx`, arnés temporal en Chromium con backend simulado.

## Notes
- Se conservan los comandos Rust de memoria/capacidades y `StudyEventKind::CapabilityMapOpened` (compatibilidad de exportaciones); este último ya no se emite.
- Las relaciones se guardan por curso: si se borra la sesión, se borran con él.
