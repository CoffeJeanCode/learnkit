import { renderToStaticMarkup } from "react-dom/server";
import { DynamicSectionBlockSchema, GateSubmissionSchema } from "../src/lib/schemas";
import type { GateResult, NotebookBlock } from "../src/lib/schemas";
import { DeclarativeVisualDiagramBlock } from "../src/features/notebook/blocks/DeclarativeVisualDiagramBlock";
import { HandsOnMissionBlock } from "../src/features/notebook/blocks/HandsOnMissionBlock";
import { HeuristicErrorAuditBlock } from "../src/features/notebook/blocks/HeuristicErrorAuditBlock";
import { InteractivePredictionGateBlock } from "../src/features/notebook/blocks/InteractivePredictionGateBlock";
import { BranchingScenarioChallengeBlock } from "../src/features/notebook/blocks/BranchingScenarioChallengeBlock";
import { moveItem, splitSnippetLines, tokenMatches } from "../src/features/notebook/blocks/interaction";
import { InlineText, RichText } from "../src/features/notebook/blocks/RichText";

const question = [
  "Lee este programa antes de ejecutarlo mentalmente:",
  "",
  "```rust",
  "fn guardar(nota: String) {",
  "    // usa `nota` aquí dentro",
  "}",
  "",
  "fn main() {",
  '    let nota = String::from("ideas para la base de conocimiento");',
  "    guardar(nota);",
  '    println!("{}", nota);',
  "}",
  "```",
  "",
  "¿Qué ocurre cuando este código se compila?",
].join("\n");

const html = renderToStaticMarkup(<RichText className="block-question" text={question} />);
console.log("--- RichText ---");
console.log(html);

const checks: Record<string, boolean> = {
  "one fenced block only": (html.match(/<pre/g) ?? []).length === 1,
  "block keeps language": html.includes('class="language-rust"'),
  "block content intact": html.includes("fn guardar(nota: String)"),
  "prose outside the block": html.includes("<p class=\"rich-text-p\">") && html.includes("¿Qué ocurre"),
  "fences are gone": !html.includes("```"),
};

const inline = renderToStaticMarkup(
  <p>
    <InlineText text={"Suponer que `let b = a` copia, como en Python."} />
  </p>,
);
console.log("--- InlineText ---");
console.log(inline);
checks["inline stays inline"] = inline.includes('<code class="inline-code">let b = a</code>');
checks["inline has no <pre>"] = !inline.includes("<pre");

// InlineText goes inside flex containers (.theory-part is display:flex):
// without ONE wrapper element each text run / <code> becomes its own flex
// item and the sentence shatters into islands separated by `gap`.
const wrapped = renderToStaticMarkup(<InlineText text={"a `b` c"} />);
checks["inline parts share one wrapper"] = wrapped.startsWith('<span class="inline-text">') && wrapped.endsWith("</span>");
checks["inline wrapper holds both text and code"] =
  wrapped === '<span class="inline-text">a <code class="inline-code">b</code> c</span>';

const plain = renderToStaticMarkup(<p><InlineText text={"sin backticks, sin cambios"} /></p>);
checks["plain text keeps the wrapper, no markup"] =
  plain === '<p><span class="inline-text">sin backticks, sin cambios</span></p>';

// The stored row that read "texto cortado, sin bloques de código": its
// escapes arrived double, so the string was one line of literal \n and the
// inline-code regex ate half the text instead of the fence becoming a <pre>.
const escapedQuestion =
  "Lee este programa antes de ejecutarlo mentalmente:\\n\\n```rust\\nfn guardar(nota: String) {\\n    let nota = String::from(\\\"ideas\\\");\\n    guardar(nota);\\n}\\n```\\n\\n¿Qué ocurre cuando este código se compila?";
const escapedHtml = renderToStaticMarkup(<RichText text={escapedQuestion} />);
console.log("--- RichText (double-escaped input) ---");
console.log(escapedHtml);
checks["escaped input becomes a block"] = escapedHtml.includes("<pre") && escapedHtml.includes("language-rust");
checks["escaped input: exactly one block"] = (escapedHtml.match(/<pre/g) ?? []).length === 1;
checks["escaped input: no literal \\n shown"] = !escapedHtml.includes("\\n");
checks["escaped input: quotes restored"] = escapedHtml.includes("String::from(&quot;ideas&quot;)");
checks["escaped input: prose still a paragraph"] = escapedHtml.includes(
  '<p class="rich-text-p"><span class="inline-text">¿Qué ocurre',
);
checks["escaped input: no mega inline span"] =
  !escapedHtml.includes('inline-code">rust') && !escapedHtml.includes('inline-code">Lee');

// Markdown the assistant's answer actually contains (see the 3-layer model
// answer in lexical_assistant_agent.rs): `**bold**`, `1. `/`- ` lists and
// `#`-prefixed headings — hand-rolled, still no markdown dependency.
const boldInline = renderToStaticMarkup(<InlineText text={"La capa **mecanismo** cierra."} />);
console.log("--- InlineText (bold) ---");
console.log(boldInline);
checks["bold becomes <strong>"] = boldInline.includes("<strong>mecanismo</strong>");
checks["bold keeps the single wrapper"] = boldInline.startsWith('<span class="inline-text">');

// Code spans are split FIRST, so a `**` inside `code` is never parsed as bold.
const codeAndBold = renderToStaticMarkup(<InlineText text={"sea `let b = a` y **mecanismo** aquí."} />);
console.log("--- InlineText (code + bold) ---");
console.log(codeAndBold);
const codeText = codeAndBold.match(/<code class="inline-code">([^<]*)<\/code>/)?.[1] ?? "";
checks["bold sits next to inline code"] = codeAndBold.includes("</code> y <strong>mecanismo</strong>");
checks["bold never enters <code>"] = codeText.includes("let b = a") && !codeText.includes("**");

const orderedHtml = renderToStaticMarkup(<RichText text={"1. analogía\n2. mecanismo\n3. límite"} />);
console.log("--- RichText (ordered list) ---");
console.log(orderedHtml);
checks["ordered list becomes <ol>"] = orderedHtml.includes("<ol>") && (orderedHtml.match(/<li>/g) ?? []).length === 3;
checks["ordered list keeps every item"] = orderedHtml.includes("analogía") && orderedHtml.includes("límite");
checks["ordered list drops the marker"] = !orderedHtml.includes("1. ");

const unorderedHtml = renderToStaticMarkup(<RichText text={"- una capa\n- otra capa"} />);
console.log("--- RichText (unordered list) ---");
console.log(unorderedHtml);
checks["unordered list becomes <ul>"] = unorderedHtml.includes("<ul>") && (unorderedHtml.match(/<li>/g) ?? []).length === 2;
checks["unordered list drops the marker"] = !unorderedHtml.includes("- ");
checks["lists are not paragraphs"] = !unorderedHtml.includes("<p") && !orderedHtml.includes("<p");

const headingHtml = renderToStaticMarkup(<RichText text={"### Tres capas de respuesta"} />);
console.log("--- RichText (heading) ---");
console.log(headingHtml);
checks["heading marker stripped"] = !headingHtml.includes("#");
checks["heading renders a bold paragraph"] = headingHtml.includes(
  '<p class="rich-text-p"><strong><span class="inline-text">Tres capas de respuesta</span></strong></p>',
);

// Regression: the new list/heading/bold parsing must not touch fenced code.
const fencedAfter = renderToStaticMarkup(<RichText text={"intro\n\n```js\nconst a = **x**;\n```\n\ndespués"} />);
console.log("--- RichText (fence, after markdown additions) ---");
console.log(fencedAfter);
checks["fence still renders <pre><code>"] = fencedAfter.includes(
  '<pre class="code-block"><code class="language-js">',
);
checks["fence content stays literal"] = fencedAfter.includes("const a = **x**;") && !fencedAfter.includes("<strong>");
checks["fence prose stays a paragraph"] = fencedAfter.includes('<span class="inline-text">intro</span>');


// --- Notebook interaction mechanics -----------------------------------------
// Static markup shows each component's FIRST paint (initial state), which is
// where the scaffolding promises live: a pre-filled editor instead of a blank
// one, hints hidden until asked for, numbered lines to click, a one-step-at-
// a-time walkthrough, and one control per interaction mode.

const noopSubmit = async (): Promise<GateResult> => {
  throw new Error("not used in static rendering");
};

function fakeBlock(block_type: NotebookBlock["block_type"], content: unknown, status: NotebookBlock["status"] = "ready"): NotebookBlock {
  return {
    id: "b1",
    document_id: "d1",
    block_type,
    content_json: content as Record<string, unknown>,
    order_index: 0,
    status,
    attempt_count: 0,
    last_feedback: null,
  } as NotebookBlock;
}

function parseBlock<T extends string>(content: Record<string, unknown>, type: T) {
  const parsed = DynamicSectionBlockSchema.parse(content);
  if (parsed.blockType !== type) throw new Error(`expected ${type}`);
  return parsed as Extract<typeof parsed, { blockType: T }>;
}

// 1) hands_on_mission: backward compatibility, then the scaffolded variants.
const oldMission = {
  blockType: "hands_on_mission",
  challengeStatement: "Resuelve el caso",
  expectedMilestoneArtifact: "Una función",
  constraints: ["Sin librerías"],
  scaffoldingHints: ["Pista general", "Pista concreta", "Casi la respuesta"],
  evaluationRubricSummary: ["Cubre el caso base"],
};
const legacy = parseBlock(oldMission, "hands_on_mission");
checks["old mission (no scaffolding fields) still parses"] = !legacy.supportLevel && !legacy.workedTemplate && !legacy.interactiveSteps;

const legacyHtml = renderToStaticMarkup(
  <HandsOnMissionBlock block={fakeBlock("hands_on_mission", oldMission)} content={legacy} isActive onSubmitGate={noopSubmit} />,
);
checks["hints start hidden behind a counter button"] = legacyHtml.includes("Pedir pista") && legacyHtml.includes("(0/3)");
checks["no hint text is shown up front"] = !legacyHtml.includes("Pista general") && !legacyHtml.includes("Casi la respuesta");
checks["no de-escalation button without a handler"] = !legacyHtml.includes("paso previo más guiado");
checks["old mission opens on an empty editor"] = /<textarea[^>]*><\/textarea>/.test(legacyHtml);

const scaffolded = parseBlock(
  {
    ...oldMission,
    supportLevel: "faded",
    workedTemplate: "function suma(a, b) {\n  return ___;\n}",
    interactiveSteps: [
      { prompt: "¿Qué operador suma?", expectedToken: "+", options: ["+", "-"] },
      { prompt: "Nombre del parámetro", expectedToken: "a" },
    ],
  },
  "hands_on_mission",
);
const noSteps = { ...scaffolded, interactiveSteps: null };
const templateHtml = renderToStaticMarkup(
  <HandsOnMissionBlock
    block={fakeBlock("hands_on_mission", scaffolded)}
    content={noSteps}
    isActive
    onSubmitGate={noopSubmit}
    onRequestEasierStep={noopSubmit}
  />,
);
checks["worked template pre-fills the editor"] = templateHtml.includes("return ___;");
checks["template editor is the monospace one"] = templateHtml.includes("mission-editor-code");
checks["restore-template button is offered"] = templateHtml.includes("Restaurar plantilla");
checks["de-escalation button next to submit"] = templateHtml.includes("Necesito un paso previo más guiado") && templateHtml.includes("Enviar mi solución");
checks["unchanged template can't be sent"] = /<button[^>]*disabled[^>]*>Enviar mi solución/.test(templateHtml);
checks["support level is labelled"] = templateHtml.includes("Con menos apoyo");

const stepsHtml = renderToStaticMarkup(
  <HandsOnMissionBlock block={fakeBlock("hands_on_mission", scaffolded)} content={scaffolded} isActive onSubmitGate={noopSubmit} />,
);
checks["cloze micro-steps render before the editor"] = stepsHtml.includes("¿Qué operador suma?") && stepsHtml.includes("cloze-chips");
checks["editor stays closed until the steps are solved"] = !stepsHtml.includes("<textarea") && stepsHtml.includes("Resuelve los micro-pasos");
checks["later steps are locked"] = (stepsHtml.match(/is-locked/g) ?? []).length === 1;

checks["cloze token matching ignores case and spacing"] = tokenMatches("  Return   X ", "return x") && !tokenMatches("a", "b");

// 2) heuristic_error_audit: bug bounty.
const audit = parseBlock(
  {
    blockType: "heuristic_error_audit",
    instruction: "Audita este código",
    flawedRepresentation: { context: "Suma", buggySnippetOrDiagram: "let a = 1;\nlet b = a +;\nreturn b;", errorType: "syntax" },
    guidingQuestions: ["¿Qué falta?"],
  },
  "heuristic_error_audit",
);
const auditHtml = renderToStaticMarkup(
  <HeuristicErrorAuditBlock block={fakeBlock("heuristic_error_audit", audit)} content={audit} isActive onSubmitGate={noopSubmit} />,
);
checks["audit renders one clickable row per line"] = (auditHtml.match(/class="bug-line"/g) ?? []).length === 3;
checks["audit rows are numbered"] = auditHtml.includes('bug-line-no">2<');
checks["audit asks to point at a line first"] = auditHtml.includes("Señala la línea");
checks["audit blocks sending until a line is picked"] = /<button[^>]*disabled[^>]*>Enviar diagnóstico/.test(auditHtml);

const oneLiner = { ...audit, flawedRepresentation: { ...audit.flawedRepresentation, buggySnippetOrDiagram: "x = x + 1" } };
const oneLinerHtml = renderToStaticMarkup(
  <HeuristicErrorAuditBlock block={fakeBlock("heuristic_error_audit", oneLiner)} content={oneLiner} isActive onSubmitGate={noopSubmit} />,
);
checks["a one-line snippet keeps the plain flow"] = oneLinerHtml.includes("flawed-snippet") && !oneLinerHtml.includes("bug-line");
checks["snippet lines ignore a trailing newline"] = splitSnippetLines("a\nb\n").length === 2 && splitSnippetLines("solo").length === 1;

const faultyKey = DynamicSectionBlockSchema.safeParse({ ...audit, faultyLineNumber: 2 });
checks["faultyLineNumber is accepted (and optional)"] = faultyKey.success && DynamicSectionBlockSchema.safeParse(audit).success;
const passedAuditHtml = renderToStaticMarkup(
  <HeuristicErrorAuditBlock
    block={fakeBlock("heuristic_error_audit", audit, "passed")}
    content={{ ...audit, faultyLineNumber: 2, modelSolution: "Falta el operando" }}
    isActive
    onSubmitGate={noopSubmit}
  />,
);
checks["the real faulty line is highlighted once passed"] = /bug-line is-faulty"[^>]*aria-label="Línea 2"/.test(passedAuditHtml);

// 3) declarative_visual_diagram: stepper + spotlight.
const diagram = parseBlock(
  {
    blockType: "declarative_visual_diagram",
    title: "La célula",
    visualAid: {
      renderEngine: "declarative_svg",
      viewBox: "0 0 100 50",
      groups: [
        { groupId: "membrana", pedagogicalRole: "barrera", elements: [{ tag: "rect", props: { x: 0, y: 0, width: 10, height: 10 } }] },
        { groupId: "nucleo", pedagogicalRole: "control", elements: [{ tag: "circle", props: { cx: 5, cy: 5, r: 3 } }] },
      ],
      caption: "Una célula",
    },
    guidedWalkthrough: [
      { stepNumber: 2, targetVisualElement: "nucleo", pedagogicalInsight: "El núcleo guarda el ADN" },
      { stepNumber: 1, targetVisualElement: "membrana", pedagogicalInsight: "La membrana filtra" },
    ],
  },
  "declarative_visual_diagram",
);
const diagramHtml = renderToStaticMarkup(<DeclarativeVisualDiagramBlock content={diagram} />);
checks["walkthrough starts on step 1 of N, sorted by stepNumber"] = diagramHtml.includes("Paso 1 de 2") && diagramHtml.indexOf("La membrana filtra") < diagramHtml.indexOf("El núcleo guarda");
checks["the active step's group is spotlighted, the rest dimmed"] =
  /data-group-id="membrana"[^>]*class="svg-group is-active"/.test(diagramHtml) && /data-group-id="nucleo"[^>]*class="svg-group is-dim"/.test(diagramHtml);
checks["stepper offers next, and previous is disabled on step 1"] = diagramHtml.includes("Siguiente paso") && /<button[^>]*disabled[^>]*>Paso anterior/.test(diagramHtml);
checks["the full list stays reachable"] = diagramHtml.includes("Ver todos los pasos");

const ghostStep = { ...diagram, guidedWalkthrough: [{ stepNumber: 1, targetVisualElement: "no-existe", pedagogicalInsight: "x" }] };
const ghostHtml = renderToStaticMarkup(<DeclarativeVisualDiagramBlock content={ghostStep} />);
checks["a step naming no group never dims the figure"] = !ghostHtml.includes("is-dim");

// 4) interactive_prediction_gate: one control set per mode, old blocks intact.
const oldGate = parseBlock(
  {
    blockType: "interactive_prediction_gate",
    question: "¿Qué pasa?",
    options: ["A", "B"],
    conceptualFeedbackMap: { A: "porque a", B: "porque b" },
  },
  "interactive_prediction_gate",
);
const oldGateHtml = renderToStaticMarkup(
  <InteractivePredictionGateBlock block={fakeBlock("interactive_prediction_gate", oldGate)} content={oldGate} isActive onSubmitGate={noopSubmit} />,
);
checks["multiple choice is still the default"] = !oldGate.interactionMode && oldGateHtml.includes("prediction-options") && !oldGateHtml.includes("option-discarded");

const chain = parseBlock(
  { blockType: "interactive_prediction_gate", question: "Ordena", interactionMode: "causal_chain_order", shuffledSteps: ["Paso B", "Paso A", "Paso C"] },
  "interactive_prediction_gate",
);
checks["a chain gate parses without options or answer key"] = chain.options.length === 0 && Object.keys(chain.conceptualFeedbackMap).length === 0;
const chainHtml = renderToStaticMarkup(
  <InteractivePredictionGateBlock block={fakeBlock("interactive_prediction_gate", chain)} content={chain} isActive onSubmitGate={noopSubmit} />,
);
checks["chain gate lists the shuffled links with up/down controls"] =
  (chainHtml.match(/chain-item/g) ?? []).length === 3 && chainHtml.includes("↑") && chainHtml.includes("↓") && chainHtml.includes("Comprobar secuencia");
checks["chain gate keeps the shuffled order on first paint"] = chainHtml.indexOf("Paso B") < chainHtml.indexOf("Paso A");
checks["moveItem swaps neighbours and clamps at the edges"] =
  moveItem(["a", "b", "c"], 1, -1).join("") === "bac" && moveItem(["a", "b", "c"], 0, -1).join("") === "abc" && moveItem(["a", "b", "c"], 2, 1).join("") === "abc";

const boundary = parseBlock(
  {
    blockType: "interactive_prediction_gate",
    question: "¿Cumple la regla?",
    interactionMode: "boundary_case_audit",
    boundaryCases: [{ statement: "Caso uno" }, { statement: "Caso dos" }],
  },
  "interactive_prediction_gate",
);
const boundaryHtml = renderToStaticMarkup(
  <InteractivePredictionGateBlock block={fakeBlock("interactive_prediction_gate", boundary)} content={boundary} isActive onSubmitGate={noopSubmit} />,
);
checks["boundary gate shows both verdict buttons per case"] =
  (boundaryHtml.match(/>Cumple la regla<\/button>/g) ?? []).length === 2 && (boundaryHtml.match(/>Rompe la regla<\/button>/g) ?? []).length === 2;
checks["boundary gate can't be checked before every case is classified"] = /<button[^>]*disabled[^>]*>Comprobar casos/.test(boundaryHtml);

const branching = parseBlock(
  {
    blockType: "branching_scenario_challenge",
    scenario: "Tu equipo se retrasa",
    decisionPoint: "¿Qué haces?",
    branches: [
      { choice: "Hablar", consequence: "Se aclara" },
      { choice: "Ignorar", consequence: "Empeora" },
    ],
  },
  "branching_scenario_challenge",
);
const branchingHtml = renderToStaticMarkup(
  <BranchingScenarioChallengeBlock block={fakeBlock("branching_scenario_challenge", branching)} content={branching} isActive onSubmitGate={noopSubmit} />,
);
checks["branching gate starts with every branch available"] = !branchingHtml.includes("option-discarded") && (branchingHtml.match(/<button/g) ?? []).length === 2;

// 5) Submission contracts (wire shape used by the Tauri commands).
checks["prediction submissions: choice, order or verdicts"] =
  GateSubmissionSchema.safeParse({ blockType: "interactive_prediction_gate", selectedOption: "A" }).success &&
  GateSubmissionSchema.safeParse({ blockType: "interactive_prediction_gate", orderedSteps: ["a", "b"] }).success &&
  GateSubmissionSchema.safeParse({ blockType: "interactive_prediction_gate", caseVerdicts: [true, false] }).success;
checks["mission submission carries hintsShown, audit carries selectedLine"] =
  GateSubmissionSchema.safeParse({ blockType: "hands_on_mission", submissionText: "x", hintsShown: 2 }).success &&
  GateSubmissionSchema.safeParse({ blockType: "heuristic_error_audit", diagnosisText: "x", selectedLine: 3 }).success;
checks["Option<T> fields accept an explicit null (Rust sends null)"] =
  DynamicSectionBlockSchema.safeParse({ ...oldMission, supportLevel: null, workedTemplate: null, interactiveSteps: null }).success &&
  DynamicSectionBlockSchema.safeParse({ ...audit, faultyLineNumber: null }).success;


// 6) Persistence + private notes on questions.
import { BlockNote } from "../src/features/notebook/blocks/persist";
const noopAnnotate = () => {};

const savedGate = parseBlock(
  { ...oldGate, blockType: "interactive_prediction_gate", studentNote: "Ojo con B", studentState: { failedOptions: ["A"], lastOption: "A" } },
  "interactive_prediction_gate",
);
checks["notes and selection state parse on a question"] = savedGate.studentNote === "Ojo con B" && !!savedGate.studentState;
checks["old blocks without notes still parse"] = DynamicSectionBlockSchema.safeParse(oldMission).success && !oldGate.studentNote;
const restoredGate = renderToStaticMarkup(
  <InteractivePredictionGateBlock block={fakeBlock("interactive_prediction_gate", savedGate)} content={savedGate} isActive onSubmitGate={noopSubmit} onAnnotate={noopAnnotate} />,
);
checks["a saved wrong pick comes back discarded, with its reason"] =
  restoredGate.includes("option-discarded") && restoredGate.includes("discarded-list") && restoredGate.includes("porque a");

const restoredChain = parseBlock({ ...chain, studentState: { order: ["Paso C", "Paso B", "Paso A"], failedKeys: [] } }, "interactive_prediction_gate");
const restoredChainHtml = renderToStaticMarkup(
  <InteractivePredictionGateBlock block={fakeBlock("interactive_prediction_gate", restoredChain)} content={restoredChain} isActive onSubmitGate={noopSubmit} onAnnotate={noopAnnotate} />,
);
checks["a saved order is restored"] = restoredChainHtml.indexOf("Paso C") < restoredChainHtml.indexOf("Paso B") && restoredChainHtml.indexOf("Paso B") < restoredChainHtml.indexOf("Paso A");
const corruptChain = parseBlock({ ...chain, studentState: { order: ["otro", "paso"] } }, "interactive_prediction_gate");
const corruptHtml = renderToStaticMarkup(
  <InteractivePredictionGateBlock block={fakeBlock("interactive_prediction_gate", corruptChain)} content={corruptChain} isActive onSubmitGate={noopSubmit} onAnnotate={noopAnnotate} />,
);
checks["a stale/corrupt saved order falls back to the shuffled one"] = corruptHtml.indexOf("Paso B") < corruptHtml.indexOf("Paso A");

const restoredAudit = parseBlock({ ...audit, studentState: { selectedLine: 2, diagnosis: "borrador" } }, "heuristic_error_audit");
const restoredAuditHtml = renderToStaticMarkup(
  <HeuristicErrorAuditBlock block={fakeBlock("heuristic_error_audit", restoredAudit)} content={restoredAudit} isActive onSubmitGate={noopSubmit} onAnnotate={noopAnnotate} />,
);
checks["the flagged line and the draft diagnosis are restored"] = /bug-line is-selected"[^>]*aria-label="Línea 2"/.test(restoredAuditHtml) && restoredAuditHtml.includes(">borrador</textarea>");
const outOfRange = parseBlock({ ...audit, studentState: { selectedLine: 99 } }, "heuristic_error_audit");
checks["an out-of-range saved line is ignored"] = !renderToStaticMarkup(
  <HeuristicErrorAuditBlock block={fakeBlock("heuristic_error_audit", outOfRange)} content={outOfRange} isActive onSubmitGate={noopSubmit} onAnnotate={noopAnnotate} />,
).includes("is-selected");

const restoredMission = parseBlock({ ...scaffolded, interactiveSteps: null, studentState: { submission: "mi avance", hintsShown: 2 } }, "hands_on_mission");
const restoredMissionHtml = renderToStaticMarkup(
  <HandsOnMissionBlock block={fakeBlock("hands_on_mission", restoredMission)} content={restoredMission} isActive onSubmitGate={noopSubmit} onAnnotate={noopAnnotate} />,
);
checks["mission draft and opened hints are restored"] = restoredMissionHtml.includes(">mi avance</textarea>") && restoredMissionHtml.includes("(2/3)") && (restoredMissionHtml.match(/hint-revealed"/g) ?? []).length === 2;

const noteHtml = renderToStaticMarkup(<BlockNote blockId="b1" saved="Mi apunte" onAnnotate={noopAnnotate} />);
checks["a saved note is shown, its panel open"] = noteHtml.includes("Mi apunte") && noteHtml.includes("<details") && noteHtml.includes(" open");
checks["an empty note starts collapsed"] = !renderToStaticMarkup(<BlockNote blockId="b1" onAnnotate={noopAnnotate} />).includes(" open");
checks["no notes UI without a save handler"] = renderToStaticMarkup(<BlockNote blockId="b1" saved="x" />) === "";

const failed = Object.entries(checks).filter(([, ok]) => !ok);
for (const [name, ok] of Object.entries(checks)) console.log(`${ok ? "[ok]" : "[!!]"} ${name}`);
console.log(failed.length === 0 ? "\nPASS" : `\nFAIL (${failed.length})`);
process.exit(failed.length === 0 ? 0 : 1);
