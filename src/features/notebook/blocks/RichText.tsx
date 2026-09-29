import { Fragment, type ReactNode } from "react";

// Model-authored prose is markdown-ish, but we only render the handful of
// constructs it actually emits: fenced code blocks (```rust ... ```), inline
// `code`, `**bold**`, contiguous list runs (`- `/`* ` → <ul>, `1. ` → <ol>)
// and `#{1,6} `-prefixed headings (marker stripped, rendered as a bold
// paragraph). Deliberately NOT a markdown library — no links, images,
// tables, nested constructs or raw HTML — so a generated notebook can never
// inject markup: React escapes every string passed through here and the only
// elements created are <p>, <pre>, <code>, <strong>, <ul>, <ol> and <li>.
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

/** `**bold**` inside ONE text part. Callers split inline-code spans FIRST
 *  (see `InlineText`), so a backtick span is never searched for `**` and a
 *  code sample that mentions it stays literal. Unmatched delimiters are left
 *  as literal text — same contract as unmatched backticks. */
function splitBold(text: string, keyBase: string): ReactNode[] {
  return text.split(/(\*\*[^*\n]+\*\*)/g).map((part, i) =>
    part.length > 4 && part.startsWith("**") && part.endsWith("**") ? (
      <strong key={`${keyBase}b${i}`}>{part.slice(2, -2)}</strong>
    ) : (
      <Fragment key={`${keyBase}b${i}`}>{part}</Fragment>
    ),
  );
}

/** Inline `code` spans plus `**bold**` — safe anywhere (inside `<p>`, `<li>`,
 *  `<button>`). Unmatched backticks are left as literal text, never turned
 *  into markup. Code spans are split off before bold, so `**` inside `code`
 *  is never parsed.
 *
 *  Every part is wrapped in ONE span on purpose: prose containing this goes
 *  into flex containers (`.theory-part` is `display: flex`), where each bare
 *  text run or `<code>` would become its own flex item and shatter the
 *  sentence into islands separated by `gap` — "texto mal renderizado". A
 *  single wrapper keeps it one flex item that flows normally inside. */
export function InlineText({ text }: { text: string }) {
  const parts = unflattenEscapedText(text).split(/(`[^`\n]+`)/g);
  return (
    <span className="inline-text">
      {parts.map((part, i) =>
        part.length > 2 && part.startsWith("`") && part.endsWith("`") ? (
          <code className="inline-code" key={i}>
            {part.slice(1, -1)}
          </code>
        ) : (
          <Fragment key={i}>{splitBold(part, String(i))}</Fragment>
        ),
      )}
    </span>
  );
}

// Marker lines. Leading whitespace tolerated (markers may be indented); the
// marker itself is consumed, never rendered.
const HEADING_LINE = /^\s*#{1,6}[ \t]+(.*)$/;
const UNORDERED_LINE = /^\s*[-*][ \t]+(.*)$/;
const ORDERED_LINE = /^\s*\d+\.[ \t]+(.*)$/;

type ProseRun =
  | { kind: "p"; text: string; bold?: boolean }
  | { kind: "ul"; items: string[] }
  | { kind: "ol"; items: string[] };

/** Turns ONE blank-line-separated paragraph into renderable runs: a contiguous
 *  run of marker lines becomes a single `<ul>`/`<ol>`, a heading line becomes
 *  its text as a bold paragraph (marker stripped, no new class), everything
 *  else stays one `<p>` joined with its single newlines (rendered as breaks
 *  by `.rich-text-p`'s `white-space: pre-line`). Blank lines were already
 *  split off by the caller, so a list never spans paragraphs and fenced code
 *  never reaches here (fences are split out first, at segment level). */
function parseProse(paragraph: string): ProseRun[] {
  const runs: ProseRun[] = [];
  let prose: string[] = [];
  let list: { kind: "ul" | "ol"; items: string[] } | null = null;

  const flushProse = () => {
    if (prose.length === 0) return;
    const text = prose.join("\n").trim();
    prose = [];
    if (text) runs.push({ kind: "p", text });
  };
  const flushList = () => {
    if (!list) return;
    const { kind, items } = list;
    list = null;
    if (items.length) runs.push({ kind, items });
  };
  const pushItem = (kind: "ul" | "ol", item: string) => {
    if (list && list.kind !== kind) flushList(); // "- a" then "1. b" → two lists
    if (!list) {
      flushProse(); // prose before the first item ends, the list starts
      list = { kind, items: [] };
    }
    list.items.push(item);
  };

  for (const line of paragraph.split("\n")) {
    const heading = line.match(HEADING_LINE);
    if (heading) {
      flushList();
      flushProse();
      const text = heading[1].trim();
      if (text) runs.push({ kind: "p", text, bold: true });
      continue;
    }
    const unordered = line.match(UNORDERED_LINE);
    if (unordered && unordered[1].trim()) {
      pushItem("ul", unordered[1].trim());
      continue;
    }
    const ordered = line.match(ORDERED_LINE);
    if (ordered && ordered[1].trim()) {
      pushItem("ol", ordered[1].trim());
      continue;
    }
    flushList();
    prose.push(line);
  }
  flushList();
  flushProse();
  return runs;
}

function renderRun(run: ProseRun, key: string): ReactNode {
  if (run.kind === "p") {
    const inner = <InlineText text={run.text} />;
    return (
      <p className="rich-text-p" key={key}>
        {run.bold ? <strong>{inner}</strong> : inner}
      </p>
    );
  }
  const items = run.items.map((item, j) => (
    <li key={j}>
      <InlineText text={item} />
    </li>
  ));
  return run.kind === "ul" ? <ul key={key}>{items}</ul> : <ol key={key}>{items}</ol>;
}

/** Block-level prose: paragraphs (blank-line separated, single newlines kept),
 *  contiguous lists and headings stripped to bold text, plus fenced code
 *  blocks. Never nest this inside a `<p>`/`<li>` — the `<pre>` would be
 *  invalid there; use `InlineText` in those spots instead. */
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
          .flatMap((paragraph, j) => parseProse(paragraph).map((run, k) => renderRun(run, `s${i}p${j}r${k}`)));
      })}
    </div>
  );
}
