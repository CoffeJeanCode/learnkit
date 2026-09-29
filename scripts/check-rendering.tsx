import { renderToStaticMarkup } from "react-dom/server";
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

const failed = Object.entries(checks).filter(([, ok]) => !ok);
for (const [name, ok] of Object.entries(checks)) console.log(`${ok ? "[ok]" : "[!!]"} ${name}`);
console.log(failed.length === 0 ? "\nPASS" : `\nFAIL (${failed.length})`);
process.exit(failed.length === 0 ? 0 : 1);
