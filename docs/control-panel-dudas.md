# Panel de control: dudas y supuestos

Decisiones que tomé sin preguntar, para que las revises. Cada una indica qué haría distinto si la respuesta cambia.

## Supuestos
1. **Solo el conteo de días viaja al backend.** Los interruptores L-M-X-J-V-S-D son visuales: el plan recibe
   «N días por semana», igual que antes. Si quieres que el plan sepa *qué* días estudia (p. ej. para
   recordatorios), hay que ampliar `ProfileBasics` en Rust y en `schemas.ts`.
2. **Tope del deslizador de horas: 8 h/día.** El modelo acepta hasta 24; el deslizador se detiene en 8 para que
   sea utilizable. Un valor mayor que llegue de una sesión guardada se muestra pero el deslizador lo recorta
   visualmente (no se cambia el dato hasta que el usuario lo mueva).
3. **Tema y meta se escriben directo en el tablero**, sin abrir un mosaico. Semanas y ritmo abren su mini-juego
   al tocarlos. Elegí esto porque dijiste que escribir es lo más fácil.
4. **Orden de los mosaicos en `localStorage`** (clave `learnkit.controlPanel.order`). No se guarda por
   sesión ni por usuario: es una preferencia global de este equipo. No se pueden ocultar mosaicos.
5. **La perilla de semanas** tiene zona muerta abajo (270° de recorrido) y además acepta teclado, atajos
   rápidos (4/8/12/24) y un campo para escribir el número. Es redundante a propósito (accesibilidad).
6. **Ajustar el plan** reutiliza la perilla y el ritmo; el énfasis pasó de «chips» a cartas.
7. **Misión 2** muestra «No lo sé» como carta propia al final de cada pregunta de opciones, igual que el chip
   anterior; las preguntas de texto siguen siendo un input opcional corto.

## Dudas abiertas
- ¿Quieres que el hub tenga más mosaicos en el futuro (p. ej. «Equipo», «Nivel declarado»)? Hoy esas respuestas
  salen de las preguntas generadas por el modelo (misión 2), no de mosaicos fijos.
- ¿Los ejemplos (`EXAMPLES`) deben seguir rellenando los 4 mosaicos de golpe, o solo tema y meta?
- ¿Sonidos o vibración al encender un día o girar la perilla? No añadí ninguno.
- ¿Un nombre/ilustración propios para cada mosaico (estilo juego) o se mantiene el tono sobrio de pizarra?

## Sin verificar
- No probé la interfaz en la app de Tauri ni en un navegador: solo `tsc`, `vite build`, los checks con bun
  (`scripts/check-control-panel.tsx`, `scripts/check-profile-form.ts`) y render estático. El arrastre de la
  perilla con puntero y el aspecto visual necesitan una prueba a mano (`bun run tauri dev`).
