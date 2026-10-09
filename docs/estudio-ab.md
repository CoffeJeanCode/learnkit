# Estudio A/B: presentación del progreso

Compara dos versiones de LearnKit con **la misma enseñanza, práctica, feedback y evaluación**.
Solo cambia cómo se presenta el progreso.

| | `gamified` | `plain` |
|---|---|---|
| Mapa de capacidades («🗺 Capacidades») | sí | no |
| Marco «Reto de transferencia» en la misión | sí | no |
| Misión de transferencia, repaso con respuesta escrita, rúbrica, scaffolding, feedback | sí | **sí** (idénticos) |

**Invariante:** la versión solo la leen la interfaz (qué mostrar) y el almacén (para marcar filas y
eventos). Nunca entra a la generación de bloques, la calificación, la programación de repasos ni a
ningún prompt. Si algún día lo hiciera, los grupos diferirían en enseñanza y no solo en presentación.

## Asignación
- Por defecto `random`: se sortea **una sola vez** por instalación y se conserva (no se vuelve a sortear
  al cambiar de modo y volver).
- `gamified` / `plain` fuerzan la versión (override manual). Se cambia en **⚙ Proveedores → Versión del estudio**.
- Cada fila de evidencia y cada evento queda marcado con la versión vigente en ese momento. Si la versión
  cambió a mitad del estudio, el informe lo señala en `variantsSeen` (analizar a ese participante aparte).

## Métrica principal
`primaryOutcome`: al menos una habilidad con una **transferencia superada limpia** (primer intento, apoyo
`independent`, sin pistas) **y** un repaso escrito aprobado **al menos 24 h después** de esa transferencia.

## Métricas complementarias
`classesOpened`, `classesCompleted`, `activeDays`, `returnDays`. **No se mide tiempo de uso** (no hay
duración de sesiones; `timeOfUseMeasured: false`).

## Exportación
«Exportar datos del estudio» escribe `study-export-<ts>.json` en la carpeta de datos de la app (un archivo
por participante, con id seudónimo). El análisis agrega los archivos fuera de la app.
