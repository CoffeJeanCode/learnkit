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

const plain = renderToStaticMarkup(<p><InlineText text={"sin backticks, sin cambios"} /></p>);
checks["plain text untouched"] = plain === "<p>sin backticks, sin cambios</p>";

const failed = Object.entries(checks).filter(([, ok]) => !ok);
for (const [name, ok] of Object.entries(checks)) console.log(`${ok ? "[ok]" : "[!!]"} ${name}`);
console.log(failed.length === 0 ? "\nPASS" : `\nFAIL (${failed.length})`);
process.exit(failed.length === 0 ? 0 : 1);
