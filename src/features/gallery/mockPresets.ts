import type { NotebookBlock } from "../../lib/schemas";

export interface BlockPreset {
  id: string;
  name: string;
  description: string;
  domain: string;
  block: NotebookBlock;
}

export interface BlockTypeCategory {
  id: string;
  label: string;
  icon: string;
  pedagogicalRole: string;
  presets: BlockPreset[];
}

export const BLOCK_CATEGORIES: BlockTypeCategory[] = [
  {
    id: "anchored_micro_theory",
    label: "Teoría Anclada",
    icon: "🧠",
    pedagogicalRole: "Construye el modelo mental en 4 capas pedagógicas: gancho intuitivo, límite de analogía, regla invariante y error común.",
    presets: [
      {
        id: "amt_tokens",
        name: "Derivación de Densidad en CSS Tokens",
        description: "Enseña cómo derivar tokens multiplicando una base central sin mutar el valor original.",
        domain: "Design Systems",
        block: {
          id: "gallery-amt-1",
          document_id: "gallery-doc",
          block_type: "anchored_micro_theory",
          status: "ready",
          attempt_count: 0,
          order_index: 1,
          content_json: {
            blockType: "anchored_micro_theory",
            title: "Derivación multiplicativa de densidad",
            intuitiveHook: "Imagina un zoom de cámara: no necesitas rediseñar cada mueble de la habitación para alejarte, solo cambiar el factor de aumento general.",
            analogyBoundary: "El zoom cambia el tamaño percibido de todo uniformemente; en densidad de UI, ciertos elementos (como el grosor de los bordes o el contraste de texto) deben permanecer fijos para no volverse ilegibles.",
            systemRule: "Cada token de densidad secundario DEBE calcularse multiplicando el token base mediante la función var(--spacing-base) * multiplier. El valor numérico base solo se declara UNA sola vez.",
            frequentError: "Copiar y pegar valores absolutos calculados (ej. escribir 12px directamente en vez de derivar). Si mañana la base cambia de 16px a 14px, los tokens derivados quedarán desincronizados.",
          },
        },
      },
      {
        id: "amt_btree",
        name: "Índices B-Tree en Bases de Datos",
        description: "Explica cómo un índice balanceado reduce lecturas de disco de O(N) a O(log N).",
        domain: "Bases de Datos",
        block: {
          id: "gallery-amt-2",
          document_id: "gallery-doc",
          block_type: "anchored_micro_theory",
          status: "ready",
          attempt_count: 0,
          order_index: 1,
          content_json: {
            blockType: "anchored_micro_theory",
            title: "Estructura y navegación en Árboles B+",
            intuitiveHook: "Un índice es como el catálogo temático de una gran biblioteca: en vez de recorrer cada estantería libro por libro, vas a la ficha de la letra inicial y saltas al pasillo exacto.",
            analogyBoundary: "Las fichas de biblioteca no requieren reorganizar los libros físicos al agregar uno nuevo; en bases de datos, las inserciones continuas provocan 'page splits' que consumen E/S.",
            systemRule: "El motor de base de datos lee páginas enteras de 8KB o 16KB. Un índice minimiza las páginas transferidas desde el almacenamiento persistente a la memoria RAM.",
            frequentError: "Crear un índice sobre columnas de muy baja cardinalidad (ej. campos booleanos), obligando al planificador a descartar el índice y preferir un escaneo secuencial.",
          },
        },
      },
      {
        id: "amt_dna_codons",
        name: "ADN leído de 3 en 3 (con figura)",
        description: "Teoría con visualAid sequence_strip: los codones dibujados como se leen y la letra mutada resaltada.",
        domain: "Biología",
        block: {
          id: "gallery-amt-3",
          document_id: "gallery-doc",
          block_type: "anchored_micro_theory",
          status: "ready",
          attempt_count: 0,
          order_index: 1,
          content_json: {
            blockType: "anchored_micro_theory",
            title: "El ADN se lee en palabras de 3 letras",
            intuitiveHook: "Tu ADN es un libro enorme escrito con solo 4 letras: cada 3 letras seguidas forman una palabra con una orden para tu cuerpo.",
            analogyBoundary: "En el libro real no hay autor ni corrector: las letras son moléculas que solo siguen su química.",
            systemRule:
              "1. El ADN usa 4 letras (A, T, G, C); A se empareja siempre con T y G con C.\n2. La célula lee las letras de 3 en 3: cada grupo (un codón) ordena fabricar un aminoácido.\n3. Los aminoácidos seguidos forman una proteína, la herramienta que hace el trabajo.\n\nEjemplo: la tira ATGGGATAA se lee [ATG] [GGA] [TAA]: arranque, glicina y fin.",
            frequentError: "Esto SÍ: ATG GGA TAA guarda 3 palabras. Esto NO: creer que una letra suelta ya es una orden, porque la célula solo lee grupos de 3.",
            visualAid: {
              renderEngine: "sequence_strip",
              rows: [
                {
                  label: "Original",
                  chunks: [
                    { text: "ATG", meaning: "Metionina · inicio", explanation: "El codón de arranque: aquí empieza a leerse la instrucción." },
                    { text: "GGA", meaning: "Glicina", explanation: "Ordena añadir una glicina a la cadena." },
                    { text: "TAA", meaning: "Fin", explanation: "Codón de parada: la proteína se suelta aquí." },
                  ],
                },
                {
                  label: "Con la mutación",
                  chunks: [
                    { text: "ATG", meaning: "Metionina · inicio" },
                    { text: "GAA", meaning: "Ácido glutámico", explanation: "Una sola G pasó a A: el codón ahora pide otro aminoácido." },
                    { text: "TAA", meaning: "Fin" },
                  ],
                },
              ],
              caption: "La célula lee de 3 en 3: cambia una letra y cambia la pieza que se añade.",
            },
          },
        },
      },
      {
        id: "amt_cad_orbit",
        name: "Orbitar la vista en CAD (con pieza 3D)",
        description: "Teoría con visualAid solid_model y showCamera: la cámara gira alrededor de la pieza fija, vista desde fuera.",
        domain: "CAD",
        block: {
          id: "gallery-amt-4",
          document_id: "gallery-doc",
          block_type: "anchored_micro_theory",
          status: "ready",
          attempt_count: 0,
          order_index: 1,
          content_json: {
            blockType: "anchored_micro_theory",
            title: "Orbitar mueve tu cámara, no la pieza",
            intuitiveHook: "Es como caminar alrededor de una escultura en un museo: tú das la vuelta, la escultura sigue en su pedestal.",
            analogyBoundary: "En CAD no caminas: la cámara gira alrededor de un punto y puedes acercarte sin límite.",
            systemRule:
              "1. La pieza vive fija en el origen, con sus ejes X, Y, Z.\n2. Al arrastrar con el botón de órbita, la cámara se desplaza sobre una esfera alrededor de la pieza.\n3. Las vistas estándar (frontal, superior, derecha, isométrica) son posiciones fijas de esa cámara.\n\nEjemplo: arrastras hacia la izquierda y ves el lado derecho de la escuadra; las coordenadas del agujero (15, 0) no cambian.",
            frequentError: "Esto SÍ: orbitar para ver el agujero por detrás. Esto NO: creer que orbitar gira la pieza, porque sus coordenadas y sus ejes siguen iguales.",
            visualIntent: "spatial_shape",
            visualAid: {
              renderEngine: "solid_model",
              title: "Escuadra de fijación",
              units: "mm",
              features: [
                { id: "base", label: "Base 60×30×8", op: "extrude", plane: "xy", profile: { kind: "rect", w: 60, h: 30 }, at: [0, 0], offset: 0, depth: 8, explanation: "Boceto rectangular en el suelo (plano XY) extruido 8 mm hacia arriba: la placa que se atornilla." },
                { id: "pared", label: "Pared 8×30×32", op: "extrude", plane: "xy", profile: { kind: "rect", w: 8, h: 30 }, at: [-26, 0], offset: 8, depth: 32, explanation: "Segundo boceto, dibujado SOBRE la base (offset 8) en su borde izquierdo y extruido 32 mm: forma la L." },
                { id: "agujero", label: "Agujero Ø8 pasante", op: "hole", plane: "xy", profile: { kind: "circle", d: 8 }, at: [15, 0], offset: 8, explanation: "Un círculo en la cara superior de la base que atraviesa toda la pieza: por aquí pasa el tornillo." },
              ],
              showCamera: true,
              startView: "iso",
              caption: "Arrastra la vista: en el recuadro ves tu cámara moverse mientras la pieza y sus ejes no se mueven.",
            },
          },
        },
      },
      {
        id: "amt_gr_geodesics",
        name: "Relatividad: la gravedad como curvatura (superficie 3D)",
        description: "Teoría con visualAid surface_3d: un pozo de espacio-tiempo con slider de masa y geodésicas reales que se desvían.",
        domain: "Física",
        block: {
          id: "gallery-amt-5",
          document_id: "gallery-doc",
          block_type: "anchored_micro_theory",
          status: "ready",
          attempt_count: 0,
          order_index: 1,
          content_json: {
            blockType: "anchored_micro_theory",
            title: "Nada tira de la manzana: el camino se curva",
            intuitiveHook: "Dos hormigas caminan rectas y paralelas sobre una pelota: sin que nada las empuje, acaban acercándose.",
            analogyBoundary: "La superficie es una imagen del espacio, no del tiempo: en la Tierra lo que más se curva es el tiempo.",
            systemRule:
              "1. Una masa curva el espacio-tiempo a su alrededor; más masa, más curvatura.\n2. Un cuerpo libre sigue una geodésica: el camino más recto posible en esa geometría. Nada lo empuja.\n3. Dos geodésicas que empiezan paralelas se desvían hacia la masa: eso es lo que llamamos atracción.\n\nEjemplo: lanza dos trayectorias paralelas a 1,5 unidades a cada lado de la masa. En la superficie se doblan hacia el centro y se cruzan detrás de ella; sube la masa y se doblan más. Ninguna fuerza aparece en el cálculo: solo la forma.",
            frequentError: "Esto SÍ: la manzana cae porque sigue una geodésica. Esto NO: una fuerza invisible tira de ella, porque en caída libre no sientes ninguna fuerza; la que sientes es la del suelo frenándote.",
            visualIntent: "curved_surface",
            visualAid: {
              renderEngine: "surface_3d",
              title: "Espacio curvado por una masa",
              expr: "-m / sqrt(x^2 + y^2 + 0.5)",
              xRange: [-4, 4],
              yRange: [-4, 4],
              params: [{ sym: "m", label: "Masa", unit: "", min: 0.3, max: 3, step: 0.1, init: 1.5 }],
              zLabel: "curvatura",
              paths: [
                { id: "arriba", label: "Geodésica a +1,5", kind: "geodesic", start: [-4, 1.5], direction: [1, 0], explanation: "Entra recta y paralela; al pasar junto a la masa se dobla hacia ella sin que nada la empuje." },
                { id: "abajo", label: "Geodésica a −1,5", kind: "geodesic", start: [-4, -1.5], direction: [1, 0], explanation: "Su gemela del otro lado: se dobla igual, hacia el centro. Por eso se cruzan detrás de la masa." },
                { id: "lejos", label: "Geodésica lejana", kind: "geodesic", start: [-4, 3.2], direction: [1, 0], explanation: "Lejos de la masa la curvatura es pequeña: casi sigue recta." },
              ],
              markers: [{ id: "masa", label: "Masa", at: [0, 0], explanation: "La masa no tira de nada: deforma la geometría a su alrededor." }],
              caption: "Las trayectorias las calcula el sistema como geodésicas de esta superficie: mueve la masa y mira cómo se doblan más.",
            },
          },
        },
      },
      {
        id: "amt_integral_area",
        name: "La integral como área acumulada",
        description: "Teoría con function_plot + area: área que crece al arrastrar el límite, rectángulos de Riemann y A(x).",
        domain: "Cálculo",
        block: {
          id: "gallery-amt-6",
          document_id: "gallery-doc",
          block_type: "anchored_micro_theory",
          status: "ready",
          attempt_count: 0,
          order_index: 1,
          content_json: {
            blockType: "anchored_micro_theory",
            title: "La integral es el área que se va acumulando",
            intuitiveHook: "Como llenar un depósito: el grifo marca cuánto entra por segundo y el depósito guarda todo lo que entró.",
            analogyBoundary: "En el depósito nada sale; bajo el eje x el área cuenta en negativo y la integral puede bajar.",
            systemRule:
              "1. Toma f(x) = x² desde x = 0.\n2. A(b) es el área bajo la curva entre 0 y b.\n3. Aproxímala con rectángulos: con 6 rectángulos hasta b = 2 sumas ≈ 2.65; con más rectángulos te acercas a 8/3 ≈ 2.667.\n4. A(b) crece más rápido donde la curva está alta: su pendiente en b es justo f(b). Ese es el teorema fundamental.\n\nEjemplo: en b = 2, A(2) = 8/3 y la pendiente de A en 2 es f(2) = 4.",
            frequentError: "Esto SÍ: la integral de 0 a 2 de x² es un área, 8/3. Esto NO: confundirla con f(2) = 4, porque f da la altura en un punto y la integral suma todas las alturas.",
            visualIntent: "accumulation",
            visualAid: {
              renderEngine: "interactive_module",
              spec: { module: "function_plot", expr: "x^2", domain: [0, 3], params: [], tangent: false, area: { from: 0, riemann: true, showAccumulated: true } },
              caption: "Arrastra el límite y sube los rectángulos: la suma se acerca al área exacta, y la curva punteada A(x) sube con pendiente f(x).",
            },
          },
        },
      },
    ],
  },
  {
    id: "hands_on_mission",
    label: "Misión Práctica (Cloze)",
    icon: "🛠",
    pedagogicalRole: "Aplica de inmediato la regla técnica con una plantilla de código con huecos interactivos (Cloze) y reglas fijas.",
    presets: [
      {
        id: "hom_tokens",
        name: "Tabla de Densidad con Tokens CSS",
        description: "Completar los multiplicadores y nombres derivados sin romper la regla de derivación.",
        domain: "CSS & Design Tokens",
        block: {
          id: "gallery-hom-1",
          document_id: "gallery-doc",
          block_type: "hands_on_mission",
          status: "ready",
          attempt_count: 0,
          order_index: 2,
          content_json: {
            blockType: "hands_on_mission",
            challengeStatement: "Crea la escala de tokens derivados multiplicando la escala base sin tocar el token origen. Asegura que el valor base se defina una sola vez.",
            workedTemplate: `Tabla de tokens de densidad — completa los huecos:

Fila 1: spacing-base, valor ___ px, multiplicador 0.75, derivado spacing-dense, fórmula calc(var(--spacing-base) * ___)

Fila 2: radius-base, valor ___ px, multiplicador 1.25, derivado radius-roomy, fórmula calc(var(--radius-base) * ___)

Fila 3: padding-section-base, valor ___ px, multiplicador 1.00, derivado padding-section-comfort, fórmula calc(var(--padding-section-base) * ___)

📌 Regla que NO se rompe: el valor base numérico se define solo en la base. En la fórmula siempre se utiliza la variable de origen con su multiplicador.`,
            expectedMilestoneArtifact: "Tabla de 3 filas con variables CSS compuestas con calc() y sin valores crudos fijos.",
            constraints: [
              "Ningún valor derivado tiene números absolutos fijos.",
              "Cada fórmula usa var() referenciando su token base.",
              "Todos los huecos deben completarse.",
            ],
            scaffoldingHints: [
              "Usa var(--spacing-base) dentro de calc().",
              "El multiplicador para denso es 0.75.",
            ],
            evaluationRubricSummary: [
              "Uso estricto de variables y calc()",
              "Ausencia de números quemados en las fórmulas",
            ],
          },
        },
      },
      {
        id: "hom_typography",
        name: "Escala Tipográfica Dinámica (CSS & Tokens)",
        description: "Construir dos bloques en :root con control centralizado mediante --type-density.",
        domain: "CSS & Design Tokens",
        block: {
          id: "gallery-hom-2",
          document_id: "gallery-doc",
          block_type: "hands_on_mission",
          status: "ready",
          attempt_count: 0,
          order_index: 3,
          content_json: {
            blockType: "hands_on_mission",
            challengeStatement: "Ya sabes derivar densidad multiplicando la escala base sin tocar el token origen. Ahora, ¿puedes crear desde cero la tabla de tokens de densidad para tu design system (en CSS y en Figma), donde cada token denso multiplique el base y siga escalando si mañana cambias el valor origen? Al terminar, tu tabla dirá claramente qué token es el origen, cuál es el multiplicador y cómo se ve cada paso en el código.",
            workedTemplate: `:root {
  /* Token base y control único de densidad */
  --font-size-base: 16px;
  --type-density: 1;

  /* Tamaños derivados con fórmula dinámica */
  --font-size-sm: calc(var(--font-size-base) * 0.75 * var(--type-density));
  --font-size-md: calc(var(--font-size-base) * ___ * var(___));
  --font-size-lg: calc(var(--font-size-base) * ___ * var(___));
}`,
            expectedMilestoneArtifact: "CSS con dos bloques en :root: (1) tokens de tipografía — --font-size-base: 16px y --font-size-sm / --font-size-md / --font-size-lg derivados con calc(var(--font-size-base) * multiplicador * var(--type-density)) — y (2) un token único --type-density que gobierna toda la escala. Al cambiar --type-density a 0.85, los tres tamaños derivados se ajustan juntos sin editar ninguna definición.",
            constraints: [
              "El token --type-density (inicial: 1) es el ÚNICO control de densidad de la escala tipográfica: no escribas multiplicadores 0.85 ni 1.2 dentro de las definiciones de tamaño.",
              "--font-size-base se declara como 16px y jamás se deriva ni se modifica, igual que --spacing-base de tu tabla anterior.",
              "Cada tamaño derivado referencia tanto --font-size-base como --type-density vía calc(var(...) * ...); ningún derivado contiene un px escrito a mano como definición.",
              "Usa una escala multiplicadora fija para sm / md / lg distinta de la de espaciado (por ejemplo 0.75 | 1 | 1.5): no reutilices 0.5 | 1 | 1.5 tal cual.",
            ],
            scaffoldingHints: [
              "Multiplica por 1.0 para --font-size-md y por 1.5 para --font-size-lg.",
              "Ambos deben multiplicar también por var(--type-density).",
            ],
            evaluationRubricSummary: [
              "Uso exclusivo de calc() con var(--font-size-base) y var(--type-density)",
              "Ningún px hardcodeado en las definiciones derivadas",
            ],
          },
        },
      },
    ],
  },
  {
    id: "branching_scenario_challenge",
    label: "Decisión Ramificada",
    icon: "🔀",
    pedagogicalRole: "Pone al estudiante frente a un dilema de ingeniería real con trade-offs, consecuencias inmediatas y retroalimentación.",
    presets: [
      {
        id: "bsc_monolith",
        name: "Evolución de Arquitectura: Monolito vs Microservicios",
        description: "Decidir cómo estructurar un sistema en rápido crecimiento con equipo pequeño.",
        domain: "Arquitectura de Software",
        block: {
          id: "gallery-bsc-1",
          document_id: "gallery-doc",
          block_type: "branching_scenario_challenge",
          status: "ready",
          attempt_count: 0,
          order_index: 3,
          content_json: {
            blockType: "branching_scenario_challenge",
            scenario: "Tu startup pasó de 10k a 500k usuarios activos. El monolito actual funciona bien, pero los despliegues de pagos a veces bloquean al equipo de recomendaciones. Tu equipo consta de 4 ingenieros.",
            decisionPoint: "¿Qué estrategia de diseño adoptas para los próximos 6 meses?",
            branches: [
              {
                choice: "Separar inmediatamente en 8 microservicios independientes con Kubernetes y gRPC.",
                consequence: "El equipo pasa el 70% de su tiempo depurando despliegues distribuidos, fallos de red y tracing en lugar de crear valor.",
              },
              {
                choice: "Modularizar el monolito internamente (Monolito Modular) con límites de dominio claros.",
                consequence: "Excelente equilibrio: los equipos trabajan en módulos aislados sin la sobrecarga de infraestructura distribuida.",
              },
              {
                choice: "Ignorar los módulos y añadir más instancias del monolito detrás de un balanceador de carga.",
                consequence: "Resuelve el tráfico por unos meses, pero los conflictos de despliegue entre equipos empeoran drásticamente.",
              },
            ],
          },
        },
      },
    ],
  },
  {
    id: "heuristic_error_audit",
    label: "Encuentra el Error",
    icon: "🔍",
    pedagogicalRole: "Entrena el ojo crítico presentando un fragmento con un error sutil para que el estudiante diagnostique la falla.",
    presets: [
      {
        id: "hea_tokens",
        name: "Antipatrón: Sustitución en vez de Derivación",
        description: "Identificar por qué este bloque CSS rompe la consistencia del sistema de diseño.",
        domain: "CSS & Design Tokens",
        block: {
          id: "gallery-hea-1",
          document_id: "gallery-doc",
          block_type: "heuristic_error_audit",
          status: "ready",
          attempt_count: 0,
          order_index: 4,
          content_json: {
            blockType: "heuristic_error_audit",
            instruction: "Audita el siguiente bloque de tokens. Encuentra la línea que rompe la regla de escalabilidad y explica por qué fallará en producción.",
            flawedRepresentation: {
              context: "Hoja de estilos global :root con escala de espaciado modular",
              buggySnippetOrDiagram: `:root {
  --spacing-base: 16px;
  --spacing-sm: calc(var(--spacing-base) * 0.75); /* 12px */
  --spacing-md: var(--spacing-base);             /* 16px */
  --spacing-lg: 24px;                            /* 24px - ojo aquí */
  --spacing-xl: calc(var(--spacing-base) * 2);   /* 32px */
}`,
              errorType: "conceptual_misunderstanding",
            },
            guidingQuestions: [
              "¿Qué ocurre si el token base cambia de 16px a 20px para pantallas táctiles?",
              "¿--spacing-lg mantendrá la escala relativa o quedará congelada en 24px?",
            ],
            modelSolution: "La variable --spacing-lg está cableada a un número estático ('24px') en vez de usar 'calc(var(--spacing-base) * 1.5)'. Si la escala cambia para pantallas táctiles (--spacing-base: 20px), --spacing-lg quedará congelada en 24px.",
          },
        },
      },
    ],
  },
  {
    id: "interactive_prediction_gate",
    label: "Predicción y Compuertas",
    icon: "🎯",
    pedagogicalRole: "Activa el compromiso cognitivo forzando una hipótesis ANTES de revelar la respuesta o desbloquear el siguiente paso.",
    presets: [
      {
        id: "ipg_react",
        name: "Re-renders en React con dependencias de objeto",
        description: "Predecir si un componente se re-renderizará al pasar un objeto inline a un componente memoizado.",
        domain: "React & Rendimiento",
        block: {
          id: "gallery-ipg-1",
          document_id: "gallery-doc",
          block_type: "interactive_prediction_gate",
          status: "ready",
          attempt_count: 0,
          order_index: 5,
          content_json: {
            blockType: "interactive_prediction_gate",
            question: "¿Qué ocurre cuando el componente padre se re-renderiza y pasa style={{ margin: 10 }} a un hijo envuelto en React.memo?",
            options: [
              "El hijo se re-renderiza de todos modos, porque style recibe una nueva referencia en cada ejecución del padre.",
              "React.memo compara en profundidad (deep equality) las propiedades del objeto style y evita el re-renderizado.",
              "El hijo solo se re-renderiza si el valor numérico 10 cambia en el siguiente turno de estado.",
              "React lanza una advertencia en consola recomendando useMemo para todos los estilos.",
            ],
            boundaryCases: [
              {
                statement: "Si el objeto style se extrae como una constante fuera del componente, React.memo sí evita el re-renderizado.",
                followsRule: true,
              },
            ],
          },
        },
      },
    ],
  },
  {
    id: "declarative_visual_diagram",
    label: "Diagrama & Simulación Visual",
    icon: "📐",
    pedagogicalRole: "Proporciona anclaje espacial e interactividad mediante SVGs reactivos, diagramas Mermaid o módulos sandbox.",
    presets: [
      {
        id: "dvd_mermaid",
        name: "Diagrama de Flujo de Datos y Event Loop",
        description: "Visualización del flujo de microtasks y macrotasks en el navegador.",
        domain: "JavaScript Engine",
        block: {
          id: "gallery-dvd-1",
          document_id: "gallery-doc",
          block_type: "declarative_visual_diagram",
          status: "ready",
          attempt_count: 0,
          order_index: 6,
          content_json: {
            blockType: "declarative_visual_diagram",
            title: "Ciclo de ejecución de Microtasks vs Macrotasks",
            visualAid: {
              renderEngine: "mermaid",
              chartType: "flowchart",
              code: `flowchart TD
  A["Call Stack Vacío"] --> B{"¿Hay Microtasks?"}
  B -- Sí --> C["Ejecutar Microtask (Promise)"]
  C --> B
  B -- No --> D{"¿Toca Render?"}
  D -- Sí --> E["RequestAnimationFrame + Repaint"]
  D -- No --> F["Ejecutar 1 Macrotask"]
  E --> F
  F --> A`,
              caption: "Prioridad del bucle de eventos en motores JavaScript modernos.",
            },
            guidedWalkthrough: [
              {
                stepNumber: 1,
                targetVisualElement: "Call Stack Vacío",
                pedagogicalInsight: "El loop solo atiende colas cuando la pila de ejecución síncrona llega a cero.",
              },
              {
                stepNumber: 2,
                targetVisualElement: "Microtasks",
                pedagogicalInsight: "Las microtasks (Promises y queueMicrotask) tienen prioridad absoluta sobre setTimeout.",
              },
            ],
          },
        },
      },
    ],
  },
  {
    id: "scene_3d",
    label: "Escenas 3D Interactivas (three.js)",
    icon: "🧊",
    pedagogicalRole: "Topologías con mensajes animados paso a paso y modelos 3D explorables por partes, construidos desde un spec JSON cerrado.",
    presets: [
      {
        id: "s3d_http",
        name: "Cómo viaja una petición web (DNS + HTTP)",
        description: "Topología isométrica con paquetes animados y vista de secuencia (renderEngine topology_flow).",
        domain: "Redes",
        block: {
          id: "gallery-s3d-1",
          document_id: "gallery-doc",
          block_type: "declarative_visual_diagram",
          status: "ready",
          attempt_count: 0,
          order_index: 8,
          content_json: {
            blockType: "declarative_visual_diagram",
            title: "De escribir una URL a ver la página",
            visualAid: {
              renderEngine: "topology_flow",
              title: "Petición HTTP",
              nodes: [
                { id: "cliente", label: "Cliente", icon: "laptop", subtext: "192.168.1.10", explanation: "Tu navegador: origina la petición y dibuja la respuesta." },
                { id: "router", label: "Router", icon: "router", subtext: "192.168.1.1", explanation: "Puerta de salida de la red local: reenvía cada paquete hacia Internet." },
                { id: "internet", label: "Internet", icon: "cloud", explanation: "Muchas redes encadenadas; cada salto acerca el paquete a su destino." },
                { id: "dns", label: "Resolutor DNS", icon: "database", subtext: "8.8.8.8", explanation: "Traduce un nombre como www.example.com a una dirección IP." },
                { id: "web", label: "Servidor web", icon: "server", subtext: "203.0.113.10", explanation: "Escucha en el puerto 80 y responde con el HTML de la página." },
              ],
              links: [
                { from: "cliente", to: "router" },
                { from: "router", to: "internet" },
                { from: "internet", to: "dns" },
                { from: "internet", to: "web" },
              ],
              steps: [
                {
                  title: "Resolución DNS",
                  detail: "Los paquetes viajan a direcciones IP, no a nombres: antes de pedir la página hay que averiguar la IP del servidor.",
                  messages: [
                    { from: "cliente", to: "dns", label: "Query A www.example.com", tone: "request" },
                    { from: "dns", to: "cliente", label: "A 203.0.113.10", tone: "response" },
                  ],
                },
                {
                  title: "Conexión TCP",
                  detail: "HTTP necesita un canal fiable. El saludo de tres pasos acuerda números de secuencia en ambos sentidos.",
                  messages: [
                    { from: "cliente", to: "web", label: "SYN", tone: "control" },
                    { from: "web", to: "cliente", label: "SYN, ACK", tone: "control" },
                    { from: "cliente", to: "web", label: "ACK", tone: "control" },
                  ],
                },
                {
                  title: "Intercambio HTTP",
                  detail: "Con el canal abierto, el cliente pide el recurso y el servidor contesta con un código de estado y el cuerpo.",
                  messages: [
                    { from: "cliente", to: "web", label: "GET / HTTP/1.1", tone: "request" },
                    { from: "web", to: "cliente", label: "200 OK (text/html)", tone: "response" },
                  ],
                },
              ],
              caption: "Una sola carga de página son tres conversaciones distintas sobre los mismos cables.",
            },
            guidedWalkthrough: [
              { stepNumber: 1, targetVisualElement: "dns", pedagogicalInsight: "Sin esta consulta el navegador no sabría a qué dirección enviar nada." },
              { stepNumber: 2, targetVisualElement: "router", pedagogicalInsight: "Todo lo que sale de tu red pasa por aquí, sea DNS, TCP o HTTP." },
              { stepNumber: 3, targetVisualElement: "web", pedagogicalInsight: "Solo después del saludo TCP el servidor acepta la petición HTTP." },
            ],
          },
        },
      },
      {
        id: "s3d_cell",
        name: "Célula animal por dentro",
        description: "Modelo 3D esquemático con corte, partes tocables y aislamiento (renderEngine explorable_model).",
        domain: "Biología celular",
        block: {
          id: "gallery-s3d-2",
          document_id: "gallery-doc",
          block_type: "declarative_visual_diagram",
          status: "ready",
          attempt_count: 0,
          order_index: 9,
          content_json: {
            blockType: "declarative_visual_diagram",
            title: "Orgánulos de una célula animal",
            visualAid: {
              renderEngine: "explorable_model",
              title: "Célula animal",
              shell: { label: "Membrana plasmática", shape: "sphere", cutaway: true },
              parts: [
                { id: "nucleo", label: "Núcleo", shape: "sphere", size: "large", count: 1, zone: "center", color: "violet", explanation: "Guarda el ADN y decide qué proteínas se fabrican en cada momento." },
                { id: "mitocondria", label: "Mitocondrias", shape: "capsule", size: "medium", count: 4, zone: "outer", color: "rose", explanation: "Convierten la glucosa y el oxígeno en ATP, la moneda de energía de la célula." },
                { id: "reticulo", label: "Retículo endoplasmático", shape: "tubes", size: "medium", count: 2, zone: "inner", color: "sky", explanation: "Red de tubos pegada al núcleo donde se pliegan proteínas y se fabrican lípidos." },
                { id: "golgi", label: "Aparato de Golgi", shape: "disc_stack", size: "medium", count: 1, zone: "outer", color: "amber", explanation: "Empaqueta y etiqueta las proteínas antes de enviarlas a su destino." },
                { id: "lisosoma", label: "Lisosomas", shape: "blob", size: "small", count: 3, zone: "outer", color: "green", explanation: "Bolsas con enzimas que digieren residuos y orgánulos gastados." },
                { id: "ribosomas", label: "Ribosomas", shape: "grains", size: "small", count: 8, zone: "inner", color: "neutral", explanation: "Leen el ARN mensajero y ensamblan proteínas aminoácido a aminoácido." },
              ],
              caption: "Cada orgánulo es un compartimento con un trabajo propio; juntos mantienen viva a la célula.",
            },
            guidedWalkthrough: [
              { stepNumber: 1, targetVisualElement: "nucleo", pedagogicalInsight: "Todo empieza aquí: la instrucción sale del núcleo en forma de ARN mensajero." },
              { stepNumber: 2, targetVisualElement: "ribosomas", pedagogicalInsight: "Traducen esa instrucción en una cadena de aminoácidos." },
              { stepNumber: 3, targetVisualElement: "golgi", pedagogicalInsight: "La proteína recién hecha se termina y se envía desde aquí." },
              { stepNumber: 4, targetVisualElement: "mitocondria", pedagogicalInsight: "Nada de lo anterior ocurre sin el ATP que producen." },
            ],
          },
        },
      },
      {
        id: "s3d_earth",
        name: "Capas de la Tierra",
        description: "Objeto por capas (shape layer): cáscaras concéntricas abiertas con el corte, ordenadas por zona.",
        domain: "Geología",
        block: {
          id: "gallery-s3d-3",
          document_id: "gallery-doc",
          block_type: "declarative_visual_diagram",
          status: "ready",
          attempt_count: 0,
          order_index: 10,
          content_json: {
            blockType: "declarative_visual_diagram",
            title: "La Tierra por dentro",
            visualAid: {
              renderEngine: "explorable_model",
              title: "Capas de la Tierra",
              parts: [
                { id: "nucleo_interno", label: "Núcleo interno", shape: "layer", size: "medium", count: 1, zone: "center", color: "amber", explanation: "Hierro y níquel sólidos: la presión es tan alta que no se funden pese a ~5 400 °C." },
                { id: "nucleo_externo", label: "Núcleo externo", shape: "layer", size: "medium", count: 1, zone: "inner", color: "rose", explanation: "Metal líquido en movimiento: sus corrientes generan el campo magnético." },
                { id: "manto", label: "Manto", shape: "layer", size: "medium", count: 1, zone: "outer", color: "violet", explanation: "Roca caliente que fluye muy despacio y arrastra las placas de encima." },
                { id: "corteza", label: "Corteza", shape: "layer", size: "medium", count: 1, zone: "surface", color: "green", explanation: "La capa fina y rígida donde vivimos, partida en placas tectónicas." },
              ],
              caption: "Cuatro capas, de la más caliente y densa a la más fina y fría.",
            },
            guidedWalkthrough: [
              { stepNumber: 1, targetVisualElement: "corteza", pedagogicalInsight: "Lo que pisamos es apenas una cáscara: menos del 1 % del radio." },
              { stepNumber: 2, targetVisualElement: "manto", pedagogicalInsight: "Debajo, la roca fluye: ese movimiento lento mueve los continentes." },
              { stepNumber: 3, targetVisualElement: "nucleo_externo", pedagogicalInsight: "El metal líquido en giro es la dinamo del campo magnético." },
              { stepNumber: 4, targetVisualElement: "nucleo_interno", pedagogicalInsight: "En el centro, la presión gana al calor y el metal vuelve a ser sólido." },
            ],
          },
        },
      },
      {
        id: "s3d_cad_bracket",
        name: "Escuadra CAD por operaciones",
        description: "solid_model: boceto → extrusión → agujero, con medidas reales, ejes y vistas estándar.",
        domain: "CAD",
        block: {
          id: "gallery-s3d-4",
          document_id: "gallery-doc",
          block_type: "declarative_visual_diagram",
          status: "ready",
          attempt_count: 0,
          order_index: 11,
          content_json: {
            blockType: "declarative_visual_diagram",
            title: "Una escuadra en tres operaciones",
            visualIntent: "spatial_shape",
            visualAid: {
              renderEngine: "solid_model",
              title: "Escuadra de fijación",
              units: "mm",
              features: [
                { id: "base", label: "Base 60×30×8", op: "extrude", plane: "xy", profile: { kind: "rect", w: 60, h: 30 }, at: [0, 0], offset: 0, depth: 8, explanation: "Boceto rectangular en el suelo (plano XY) extruido 8 mm hacia arriba: la placa que se atornilla." },
                { id: "pared", label: "Pared 8×30×32", op: "extrude", plane: "xy", profile: { kind: "rect", w: 8, h: 30 }, at: [-26, 0], offset: 8, depth: 32, explanation: "Segundo boceto, dibujado SOBRE la base (offset 8) en su borde izquierdo y extruido 32 mm: forma la L." },
                { id: "agujero", label: "Agujero Ø8 pasante", op: "hole", plane: "xy", profile: { kind: "circle", d: 8 }, at: [15, 0], offset: 8, explanation: "Un círculo en la cara superior de la base que atraviesa toda la pieza: por aquí pasa el tornillo." },
              ],
              showCamera: false,
              startView: "iso",
              caption: "Cada operación parte de un boceto en un plano: extruir añade material, el agujero lo quita.",
            },
            guidedWalkthrough: [
              { stepNumber: 1, targetVisualElement: "base", pedagogicalInsight: "Todo empieza con un rectángulo en el plano XY que crece 8 mm hacia arriba." },
              { stepNumber: 2, targetVisualElement: "pared", pedagogicalInsight: "El segundo boceto se apoya en la cara superior de la base: por eso su offset es 8." },
              { stepNumber: 3, targetVisualElement: "front", pedagogicalInsight: "En la vista frontal la L se ve de perfil: así se acota en un plano técnico." },
              { stepNumber: 4, targetVisualElement: "agujero", pedagogicalInsight: "El agujero pasante quita material de arriba abajo: míralo desde la vista superior." },
              { stepNumber: 5, targetVisualElement: "top", pedagogicalInsight: "Desde arriba, el agujero es un círculo de Ø8 a 15 mm del centro." },
            ],
          },
        },
      },
    ],
  },
  {
    id: "spaced_interleaved_retrieval",
    label: "Repaso Espaciado (Active Recall)",
    icon: "⚡",
    pedagogicalRole: "Entrelaza preguntas de sesiones anteriores para consolidar la memoria a largo plazo.",
    presets: [
      {
        id: "sir_css",
        name: "Conceptos Fundamentales de CSS Layout",
        description: "Tarjetas de recuerdo activo sobre box-sizing, stacking context y stacking order.",
        domain: "CSS Moderno",
        block: {
          id: "gallery-sir-1",
          document_id: "gallery-doc",
          block_type: "spaced_interleaved_retrieval",
          status: "ready",
          attempt_count: 0,
          order_index: 7,
          content_json: {
            blockType: "spaced_interleaved_retrieval",
            items: [
              {
                conceptLabel: "box-sizing: border-box",
                prompt: "¿Por qué border-box simplifica el cálculo de anchos en layouts responsivos?",
                expectedAnswer: "Porque incluye el padding y el border dentro del width declarado, evitando que el elemento desborde su contenedor.",
              },
              {
                conceptLabel: "Stacking Context",
                prompt: "¿Nombrar dos propiedades CSS comunes que crean un nuevo contexto de apilamiento sin usar z-index?",
                expectedAnswer: "opacity menor a 1, transform distinto de none, filter distinto de none o contain: paint.",
              },
            ],
          },
        },
      },
    ],
  },
  {
    id: "metacognitive_closure",
    label: "Cierre Metacognitivo",
    icon: "💡",
    pedagogicalRole: "Cierra la clase invitando a contrastar lo que creía antes con lo que sabe ahora, consolidando el aprendizaje.",
    presets: [
      {
        id: "mc_closure",
        name: "Síntesis de Modelo Mental",
        description: "Contraste entre intuición previa y principio técnico aprendido.",
        domain: "Metacognición",
        block: {
          id: "gallery-mc-1",
          document_id: "gallery-doc",
          block_type: "metacognitive_closure",
          status: "ready",
          attempt_count: 0,
          order_index: 8,
          content_json: {
            blockType: "metacognitive_closure",
            synthesisTask: "En una oración de tus propias palabras: explica a un colega junior por qué un design system nunca debe permitir tokens derivados con números fijos escritos a mano.",
            predictionComparison: {
              initialPrediction: "Creíamos que duplicar números en variables era inofensivo y ahorraba cálculos.",
              finalResult: "Comprobamos que rompe la escalabilidad cuando el origen se reconfigura.",
              contrastNarrative: "La derivación centralizada mediante calc() garantiza consistencia instantánea en toda la jerarquía de UI.",
            },
            selfEvaluationChecklist: [
              "Comprendo la diferencia entre sustitución y derivación multiplicativa.",
              "Sé usar calc() con variables base relativas.",
              "Puedo auditar hojas de estilo buscando variables desacopladas.",
            ],
          },
        },
      },
    ],
  },
  {
    id: "domain_kits",
    label: "Kits de dominio (química, álgebra, música, física, código)",
    icon: "🧪",
    pedagogicalRole: "Figuras de dominio que el sistema calcula: el modelo dice QUÉ (átomos, grupo, acordes, piezas) y la app dibuja, comprueba y hace sonar.",
    presets: [
      {
        id: "kit_sn2",
        name: "SN2: ataque por detrás (molécula 3D)",
        description: "molecule_3d: CH₃Br + OH⁻ → CH₃OH + Br⁻ con el enlace que se forma y el que se rompe en el mismo paso.",
        domain: "Química orgánica",
        block: {
          id: "gallery-kit-1",
          document_id: "gallery-doc",
          block_type: "declarative_visual_diagram",
          status: "ready",
          attempt_count: 0,
          order_index: 12,
          content_json: {
            blockType: "declarative_visual_diagram",
            title: "SN2 en un solo paso",
            visualIntent: "molecule",
            visualAid: {
              renderEngine: "molecule_3d",
              title: "Bromometano + hidróxido",
              atoms: [
                { id: "C", element: "C" },
                { id: "H1", element: "H" },
                { id: "H2", element: "H" },
                { id: "H3", element: "H" },
                { id: "Br", element: "Br" },
                { id: "O", element: "O", charge: -1, label: "OH" },
                { id: "Ho", element: "H" },
              ],
              bonds: [
                { a: "C", b: "H1", order: 1 },
                { a: "C", b: "H2", order: 1 },
                { a: "C", b: "H3", order: 1 },
                { a: "C", b: "Br", order: 1 },
                { a: "O", b: "Ho", order: 1 },
              ],
              steps: [
                {
                  title: "Ataque por detrás",
                  detail: "El OH⁻ entra por el lado opuesto al Br: el enlace C–O se forma mientras el C–Br se rompe. El carbono nunca tiene cinco enlaces.",
                  form: [["O", "C"]],
                  break: [["C", "Br"]],
                  charges: { O: 0, Br: -1 },
                  highlight: ["C", "O", "Br"],
                },
              ],
              caption: "Un solo paso, sin intermedio: por eso la SN2 depende de la concentración de los dos reactivos.",
            },
            guidedWalkthrough: [
              { stepNumber: 1, targetVisualElement: "Br", pedagogicalInsight: "El bromo es el grupo saliente: se llevará los electrones del enlace." },
              { stepNumber: 2, targetVisualElement: "O", pedagogicalInsight: "El oxígeno con carga negativa es el nucleófilo: busca el carbono parcialmente positivo." },
              { stepNumber: 3, targetVisualElement: "C", pedagogicalInsight: "El carbono recibe y pierde un enlace a la vez: valencia cuatro en todo momento." },
            ],
          },
        },
      },
      {
        id: "kit_lagrange",
        name: "Lagrange en ℤ₁₂ (estructura algebraica)",
        description: "algebraic_structure: el subgrupo ⟨4⟩ y sus 4 clases laterales coloreadas en la tabla y el grafo de Cayley.",
        domain: "Álgebra abstracta",
        block: {
          id: "gallery-kit-2",
          document_id: "gallery-doc",
          block_type: "declarative_visual_diagram",
          status: "ready",
          attempt_count: 0,
          order_index: 13,
          content_json: {
            blockType: "declarative_visual_diagram",
            title: "Las clases laterales parten el grupo en trozos iguales",
            visualIntent: "algebraic",
            visualAid: {
              renderEngine: "algebraic_structure",
              group: { kind: "cyclic", n: 12 },
              subgroup: ["4"],
              view: "both",
              caption: "H = ⟨4⟩ = {0, 4, 8}: cada color es una clase lateral g + H, todas de tamaño 3. Por eso 12 = 3 × 4.",
            },
            guidedWalkthrough: [
              { stepNumber: 1, targetVisualElement: "0", pedagogicalInsight: "H son los nodos opacos: 0, 4 y 8." },
              { stepNumber: 2, targetVisualElement: "1", pedagogicalInsight: "Sumar 1 a todo H da otra clase lateral, del mismo tamaño." },
              { stepNumber: 3, targetVisualElement: "2", pedagogicalInsight: "Las clases no se solapan y cubren todo: el orden de H divide al de G." },
            ],
          },
        },
      },
      {
        id: "kit_guitar",
        name: "Sol–Do–Re con rasgueo (instrumento)",
        description: "instrument: diagramas de acordes, progresión y rasgueo D-DU-UDU que se escucha con metrónomo.",
        domain: "Guitarra",
        block: {
          id: "gallery-kit-3",
          document_id: "gallery-doc",
          block_type: "declarative_visual_diagram",
          status: "ready",
          attempt_count: 0,
          order_index: 14,
          content_json: {
            blockType: "declarative_visual_diagram",
            title: "Cambiar a tiempo entre Sol, Do y Re",
            visualIntent: "music",
            visualAid: {
              renderEngine: "instrument",
              instrument: "guitar",
              chords: ["G", "C", "D"],
              progression: [
                { chord: "G", beats: 4 },
                { chord: "C", beats: 4 },
                { chord: "G", beats: 4 },
                { chord: "D", beats: 4 },
              ],
              strum: "D-DU-UDU",
              bpm: 76,
              caption: "Escucha primero, luego toca encima. El cambio se prepara en el último ↑ del compás.",
            },
            guidedWalkthrough: [
              { stepNumber: 1, targetVisualElement: "G", pedagogicalInsight: "Sol: el anular y el meñique quedan casi en su sitio para el Re." },
              { stepNumber: 2, targetVisualElement: "C", pedagogicalInsight: "Do: el índice en la primera cuerda B es el ancla." },
              { stepNumber: 3, targetVisualElement: "D", pedagogicalInsight: "Re: solo cuatro cuerdas; no toques la sexta." },
            ],
          },
        },
      },
      {
        id: "kit_atwood",
        name: "Máquina de Atwood (kit de ilustración)",
        description: "illustration_kit: polea, dos bloques, cuerdas y fuerzas — sin coordenadas, en casillas.",
        domain: "Física",
        block: {
          id: "gallery-kit-4",
          document_id: "gallery-doc",
          block_type: "declarative_visual_diagram",
          status: "ready",
          attempt_count: 0,
          order_index: 15,
          content_json: {
            blockType: "declarative_visual_diagram",
            title: "Dos masas, una cuerda, una polea",
            visualIntent: "physical_phenomenon",
            visualAid: {
              renderEngine: "illustration_kit",
              items: [
                { id: "techo", kind: "wall", label: "Techo", slot: [2, 0], explanation: "Sostiene la polea." },
                { id: "polea", kind: "pulley", label: "Polea", slot: [3, 0], explanation: "Cambia la dirección de la tensión sin cambiar su valor (ideal)." },
                { id: "m1", kind: "block", label: "m₁", value: "2 kg", slot: [2, 2], explanation: "La masa ligera: sube." },
                { id: "m2", kind: "block", label: "m₂", value: "3 kg", slot: [4, 3], explanation: "La masa pesada: baja." },
              ],
              links: [
                { from: "m1", to: "polea", kind: "rope" },
                { from: "polea", to: "m2", kind: "rope" },
              ],
              forces: [
                { on: "m1", direction: "down", label: "P₁", magnitude: 2 },
                { on: "m1", direction: "up", label: "T", magnitude: 2 },
                { on: "m2", direction: "down", label: "P₂", magnitude: 3 },
              ],
              caption: "La misma tensión T tira de las dos masas; gana la diferencia de pesos.",
            },
            guidedWalkthrough: [
              { stepNumber: 1, targetVisualElement: "polea", pedagogicalInsight: "La cuerda es una sola: la tensión es la misma a ambos lados." },
              { stepNumber: 2, targetVisualElement: "m2", pedagogicalInsight: "P₂ > T: m₂ acelera hacia abajo." },
              { stepNumber: 3, targetVisualElement: "m1", pedagogicalInsight: "T > P₁: m₁ acelera hacia arriba con el mismo valor." },
            ],
          },
        },
      },
      {
        id: "kit_code_pendulum",
        name: "Péndulo (nivel 2: figura como código)",
        description: "code_figure: lo que escribe el modelo de escalado cuando ningún motor alcanza — corre aislado en un iframe.",
        domain: "Física",
        block: {
          id: "gallery-kit-5",
          document_id: "gallery-doc",
          block_type: "declarative_visual_diagram",
          status: "ready",
          attempt_count: 0,
          order_index: 16,
          content_json: {
            blockType: "declarative_visual_diagram",
            title: "El periodo depende de la longitud, no de la masa",
            visualIntent: "law",
            visualAid: {
              renderEngine: "code_figure",
              height: 320,
              code: "const L = fig.slider({ label: 'Longitud (m)', min: 0.3, max: 2.5, value: 1, step: 0.05 });\nconst g = 9.81;\nfig.onFrame((t) => {\n  fig.clear();\n  const len = L();\n  const T = 2 * Math.PI * Math.sqrt(len / g);\n  const theta = 0.5 * Math.cos((2 * Math.PI * t) / T);\n  const ox = fig.width / 2, oy = 30, px = 110;\n  const bx = ox + Math.sin(theta) * len * px, by = oy + Math.cos(theta) * len * px;\n  const c = fig.ctx;\n  c.strokeStyle = '#94a3b8'; c.lineWidth = 2;\n  c.beginPath(); c.moveTo(ox, oy); c.lineTo(bx, by); c.stroke();\n  c.fillStyle = '#38bdf8'; c.beginPath(); c.arc(bx, by, 14, 0, 2 * Math.PI); c.fill();\n  fig.label('T = 2π√(L/g) = ' + T.toFixed(2) + ' s', 20, fig.height - 20, '#fbbf24');\n});",
              caption: "Mueve la longitud: el periodo cambia como √L; la masa ni aparece en la fórmula.",
            },
            guidedWalkthrough: [{ stepNumber: 1, targetVisualElement: "péndulo", pedagogicalInsight: "Alarga la cuerda y cuenta: cuatro veces más larga, el doble de lento." }],
          },
        },
      },
    ],
  },
];
