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
];
