import { Fragment } from "react";

// Model-authored prose is markdown-ish, but we only render the two constructs
// it actually emits: fenced code blocks (```rust ... ```) and inline `code`.
// Deliberately NOT a markdown library — no headings, lists, links or raw HTML —
// so a generated notebook can never inject markup: React escapes every string
// passed through here and the only elements created are <p>, <pre> and <code>.
//
// Before this existed a fenced snippet rendered as literal backticks inside a
// <p> (the model writes ```rust fences in `question`, `instruction`, etc.),
// which is both unreadable and wrong for anything the student must copy.

const FENCE = /```([A-Za-z0-9_+#.-]*)[ \t]*\r?\n([\s\S]*?)```/g;

// A block can arrive with double-escaped sequences ("\\n" instead of a
// newline) — the model occasionally writes its JSON strings escaped twice.
// Once decoded, the field is ONE line full of literal backslashes: the
// student sees raw `\n`, the fence below never matches (no real newline),
// and the inline-code regex — which only excludes real newlines — builds a
// single <code> span across half the block. That is exactly the "text looks
// cut, no code blocks" symptom.
//
// Guard is deliberately narrow: only strings with NO real newline AND a
// telltale (a ``` fence or an escaped quote) are unflattened, so ordinary
// prose, Windows paths and legitimate backslashes pass through untouched.
// New rows are repaired at the source too (tools/notebook_tools.rs); this
// is the safety net for rows stored before that fix.
function unflattenEscapedText(text: string): string {
  if (!text.includes("\\")) return text;
  if (/[\r\n]/.test(text)) return text;
  if (!text.includes("```") && !text.includes('\\"')) return text;
  return text.replace(/\\r\\n/g, "\n").replace(/\\n/g, "\n").replace(/\\"/g, '"');
}

type Segment = { kind: "prose"; text: string } | { kind: "code"; lang: string; code: string };

function splitSegments(text: string): Segment[] {
  const out: Segment[] = [];
  // Fresh regex each call: a shared /g literal carries lastIndex between calls.
  const fence = new RegExp(FENCE.source, "g");
  let cursor = 0;
  let match: RegExpExecArray | null;
  while ((match = fence.exec(text)) !== null) {
    if (match.index > cursor) out.push({ kind: "prose", text: text.slice(cursor, match.index) });
    out.push({ kind: "code", lang: match[1] ?? "", code: match[2].replace(/\s+$/, "") });
    cursor = fence.lastIndex;
  }
  if (cursor < text.length) out.push({ kind: "prose", text: text.slice(cursor) });
  return out;
}

/** Inline `code` spans only — safe anywhere (inside `<p>`, `<li>`, `<button>`).
 *  Unmatched backticks are left as literal text, never turned into markup. */
export function InlineText({ text }: { text: string }) {
  const parts = unflattenEscapedText(text).split(/(`[^`\n]+`)/g);
  return (
    <>
      {parts.map((part, i) =>
        part.length > 2 && part.startsWith("`") && part.endsWith("`") ? (
          <code className="inline-code" key={i}>
            {part.slice(1, -1)}
          </code>
        ) : (
          <Fragment key={i}>{part}</Fragment>
        ),
      )}
    </>
  );
}

/** Block-level prose: paragraphs (blank-line separated, single newlines kept)
 *  plus fenced code blocks. Never nest this inside a `<p>`/`<li>` — the `<pre>`
 *  would be invalid there; use `InlineText` in those spots instead. */
export function RichText({ text, className }: { text: string; className?: string }) {
  const segments = splitSegments(unflattenEscapedText(text));
  return (
    <div className={className ? `rich-text ${className}` : "rich-text"}>
      {segments.map((segment, i) => {
        if (segment.kind === "code") {
          return (
            <pre className="code-block" key={`s${i}`}>
              <code className={segment.lang ? `language-${segment.lang}` : undefined}>{segment.code}</code>
            </pre>
          );
        }
        return segment.text
          .split(/\n{2,}/)
          .map((raw) => raw.replace(/\r/g, "").trim())
          .filter(Boolean)
          .map((paragraph, j) => (
            <p className="rich-text-p" key={`s${i}p${j}`}>
              <InlineText text={paragraph} />
            </p>
          ));
      })}
    </div>
  );
}
