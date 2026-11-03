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
4. **Las filas del panel tienen orden fijo** (se quitó la personalización del orden: no aportaba nada en un formulario corto).
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

## Interacciones tipo TouchDesigner en las compuertas (pasada 3)
Referencia: editor de nodos. Hecho:
- **Orden causal**: cada eslabón es un nodo con puertos y un cable corto; se arrastra y los vecinos se apartan
  para abrir el hueco (reflow en vivo). Teclado: foco en el nodo + flechas ↑/↓.
- **Casos límite**: cada caso es un nodo con un puerto de salida; se arrastra un cable hasta «Cumple» o «Rompe»
  (se imanta a 44 px; soltar lejos cancela; clic en un cable lo desconecta). Teclado/lector de pantalla: el par
  de botones de siempre, visible solo con foco de teclado.
- **Horas por día** (panel del plan): campo numérico que se arrastra de lado (Shift = más fino, doble clic o Enter
  para escribir), como los parámetros de TouchDesigner.
Los cables y puertos son siempre neutros: el color nunca insinúa la respuesta (la califica el servidor).

Dudas abiertas:
- Hoy el bloque `heuristic_error_audit` (elegir la línea defectuosa) y las `interactiveSteps` de la misión práctica
  siguen con clic/teclado. ¿Quieres llevarlos también al lenguaje de nodos (p. ej. arrastrar una etiqueta de causa
  raíz hasta la línea)? No lo hice: cambia la mecánica de calificación.
- El orden causal es una columna vertical. TouchDesigner suele ser horizontal; en pantallas anchas podría
  mostrarse como cadena horizontal. Decisión pendiente.
- Sin pruebas con pantalla táctil real (usa eventos de puntero con `touch-action: none`, pero no lo vi en un dispositivo).

## Entradas tipo CHOP (pasada 4)
Más allá de cómo se conectan los nodos: el vocabulario de *entradas* con que se construyen programas.
Traducción a lo que la app tiene hoy (sin tocar el backend):
| TouchDesigner | En LearnKit |
|---|---|
| Visor de un CHOP (scope, autoescala) | `ChannelScope` en «Tu ritmo»: tu semana como señal, un pulso por día con sus horas, escala redonda (1/2/4/8 h) y el total bajo la traza |
| Parámetro numérico que se arrastra | `ScrubField` (horas por día) |
| Botón/Toggle (pulso por día) | los 7 interruptores de día |
| Paleta de operadores → socket del nodo | práctica guiada: las opciones del hueco son operadores que se arrastran al socket (clic y teclado siguen valiendo) |
| Cable de salida → entrada | casos límite |

Decisiones y dudas:
- **No añadí un bloque nuevo de «pipeline de operadores»** (que el estudiante arme un programa con CHOPs
  —constante, LFO, Math, Limit, Lag, Trigger— y compare su señal con una objetivo). Sería la traducción más
  fiel, pero necesita un tipo de bloque nuevo en el esquema de Rust y en la generación/calificación. ¿Lo quieres?
  Si sí, lo siguiente sería: `signal_pipeline_challenge` (operadores permitidos, señal objetivo, parámetros con
  rangos) calificado de forma determinista en Rust (muestreando la señal), sin LLM.
- El visor de canal es solo lectura. Hacerlo editable (arrastrar la altura de cada día) obligaría a que el plan
  acepte horas distintas por día; hoy solo guarda horas/semana y días/semana.
- Un pad XY (semanas × horas/día con curvas de total constante) sería otra entrada propia de TouchDesigner; no lo
  añadí para no sumar controles al panel (veníamos de reducir la carga visual).

## Mesa de piezas (recompensas con física) y «volver donde lo dejé» (pasada 5)
**Mesa de piezas.** El feedback al aprobar ya no es confeti: es una pieza física que cae en una mesa junto al notebook
(`matter-js`), se apila y se arrastra con ratón o dedo. La pieza *informa*: ladrillo dorado 2x2 = compuerta a la primera,
azul 2x1 = al segundo intento, verde 1x1 = tras 3 o más intentos (constancia), sticker troquelado (estrella, rayo, corazón,
llama, en ciclo) = clase cerrada. Al pasar el cursor, el pie de la mesa dice cómo se ganó. Cada compuerta paga una sola vez.
La disposición se recuerda por notebook cuando las piezas quedan quietas. Respeta `prefers-reduced-motion` (se muestran ya
asentadas) y se pausa con la pestaña oculta. No aparece nada hasta ganar la primera pieza.
- **Se guarda solo en este equipo** (`localStorage`, clave `learnkit.pieces.<notebook>`), no en la base de datos: no viaja
  con la sesión ni entre dispositivos. Pasarlo al backend exigiría un comando y una tabla nuevos. ¿Lo quieres?
- Tope de 60 piezas por notebook (se van las más antiguas).
- (Resuelto en la pasada 6: los stickers van sobre la página y hay sonido; ver abajo.)
- Probado con ratón en una vista previa; la física va a cámara lenta en un navegador que limita los fotogramas, pero la
  simulación usa tiempo real con pasos fijos, así que a 60 fps cae y reposa en pocos segundos. Sin probar en táctil.

**Volver donde lo dejé.** Por sesión se guarda la sección (conversación / plan / notebook), la clase y el último bloque
alcanzado. Al abrir una sesión (al arrancar o desde el panel de sesiones) cae directo ahí y, en el notebook, se desplaza al
bloque. Una sesión aún en construcción abre su conversación; una sellada sin nada guardado abre su plan. La pestaña «Clases»
también vuelve al bloque. Se guarda en este equipo (`learnkit.resume.<sesión>`).
- Si el bloque guardado ya no existe (se regeneró la clase), se queda en la clase sin desplazarse; no inventa otro destino.
- Duda: ¿debería, en una clase **terminada**, abrir la siguiente en vez de la última? Hoy abre donde estabas.

## Stickers con peel (estilo Sticker.js) y sonidos (pasada 6)
**Stickers.** Los de «clase cerrada» (estrella, rayo, corazón, llama) dejaron la mesa de física: ahora están en una
*hoja de stickers* dentro del panel y se **arrastran a la propia página del notebook**, donde se quedan (se recuerda su
posición y su inclinación por notebook), se pueden volver a mover y, con doble clic, volver a la hoja. Los ladrillos
siguen en la mesa con física. El **peel** replica la idea de Sticker.js: al acercar el cursor a un borde, ese borde
se levanta como una pegatina real (el reverso de papel con la imagen tenue, un degradado oscuro hacia el pliegue y la
sombra que proyecta la solapa); al salir vuelve con un ease-out de 0,6 s. Lo hice con geometría propia (no con su
código) dibujada en SVG, para que el peel siga la silueta troquelada de cada sticker y no un cuadrado. El pliegue queda
a mitad de camino entre el borde y el puntero, igual que en la librería.
- Cómo se ve: borde blanco troquelado, brillo en la parte alta y sombra suave; el peel se ve mejor entrando por un lado
  y avanzando hacia dentro.
- Se pega solo sobre la página (no sobre otras vistas). Un sticker pegado queda por encima del contenido: si tapa lo
  que se está leyendo, se mueve o se guarda con doble clic.
- Duda: ¿que el sticker se pueda **elegir** (no solo el que toca por ciclo)? Hoy salen en orden: estrella, rayo, corazón, llama.
- Duda: ¿el peel también al pasar por encima en pantallas táctiles? Usa eventos de puntero, pero no lo probé en táctil.

**Sonidos** (sintetizados con WebAudio: no hay archivos de audio que empaquetar). El sonido informa:
- Al ganar una pieza suena un arpegio ascendente con **más notas cuanto antes la ganaste** (a la primera: 4, al segundo
  intento: 3, con constancia: 2); un sticker es una campanilla de dos tonos.
- Los ladrillos hacen un «clac» de plástico al chocar, tan fuerte como el golpe y más grave cuanto más grande la pieza;
  las piezas en reposo no suenan.
- Un sticker suena a papel al despegarse (una vez por despegue) y da un toque suave al pegarse; recoger una pieza hace un «pop».
- Volumen bajo (0,32), límite de frecuencia por tipo de sonido, y botón «Sonido: sí/no» en el panel de la mesa (se recuerda
  en este equipo). No suena nada hasta que hubo una interacción con la página (política del navegador).
- Duda: ¿sonido por defecto activado? Lo dejé activado, con el botón a mano. Si prefieres silencio por defecto, es un cambio de una línea.
- No pude **oírlo** (la verificación fue con reglas y pruebas de la lógica y viendo que no hay errores de consola); conviene escucharlo una vez.
