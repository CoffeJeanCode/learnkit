# Panel de control: dudas y supuestos

Decisiones que tomé sin preguntar, para que las revises. Cada una indica qué haría distinto si la respuesta cambia.

## Supuestos
1. **Solo el conteo de días viaja al backend.** Los interruptores L-M-X-J-V-S-D son visuales: el plan recibe
   «N días por semana», igual que antes. Si quieres que el plan sepa *qué* días estudia (p. ej. para
   recordatorios), hay que ampliar `ProfileBasics` en Rust y en `schemas.ts`.
2. **Tope del deslizador de horas: 8 h/día.** El modelo acepta hasta 24; el deslizador se detiene en 8 para que
   sea utilizable. Un valor mayor que llegue de una sesión guardada se muestra pero el deslizador lo recorta
   visualmente (no se cambia el dato hasta que el usuario lo mueva).
3. **Tema y meta se escriben directo en el tablero**, sin abrir nada. Semanas y ritmo arrancan con valores por
   defecto (8 semanas, 1 h/día, 5 días) y abren su mini-juego solo para cambiarlos: así quedan solo dos cosas
   que escribir.
4. **Orden de las filas en `localStorage`**, oculto tras el botón «Personalizar» (clave `learnkit.controlPanel.order`). No se guarda por
   sesión ni por usuario: es una preferencia global de este equipo. No se pueden ocultar filas.
5. **La perilla de semanas** tiene zona muerta abajo (270° de recorrido) y además acepta teclado, atajos
   rápidos (4/8/12/24). Quité el campo de texto «otra» por redundante; si hace falta un número fuera de los atajos, se llega con la perilla o las flechas.
6. **Ajustar el plan** reutiliza la perilla y el ritmo; el énfasis pasó de «chips» a cartas.
7. **El paso 2** muestra «No lo sé» como carta propia al final de cada pregunta de opciones, igual que el chip
   anterior; las preguntas de texto siguen siendo un input opcional corto.

## Dudas abiertas
- ¿Quieres que el hub tenga más mosaicos en el futuro (p. ej. «Equipo», «Nivel declarado»)? Hoy esas respuestas
  salen de las preguntas generadas por el modelo (misión 2), no de mosaicos fijos.
- ¿Los ejemplos (`EXAMPLES`) deben seguir rellenando las 4 filas de golpe, o solo tema y meta?
- ¿Sonidos o vibración al encender un día o girar la perilla? No añadí ninguno.
- ¿Un nombre/ilustración propios para cada mosaico (estilo juego) o se mantiene el tono sobrio de pizarra?

## Sin verificar
- No probé la interfaz en la app de Tauri ni en un navegador: solo `tsc`, `vite build`, los checks con bun
  (`scripts/check-control-panel.tsx`, `scripts/check-profile-form.ts`) y render estático. El arrastre de la
  perilla con puntero y el aspecto visual necesitan una prueba a mano (`bun run tauri dev`).

## Revisión crítica de la UI (pasada 2)
Vista en el navegador con los estilos reales. Quité: medidor y puntos de estado (dos indicadores del mismo
progreso), flechas de mover siempre visibles, fichas de ejemplo de colores, el vocabulario «misión/mosaico», la
caja «otra» de la perilla y los títulos centrados. Pasó de ~17 elementos iniciales a 4 filas + 1 línea de
ejemplos + 2 botones. Duda abierta: ¿los valores por defecto (8 semanas, 1 h, 5 días) son los correctos o
conviene dejar «Elegir» vacío para obligar a decidir?
