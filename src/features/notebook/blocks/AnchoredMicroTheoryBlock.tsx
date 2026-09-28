import type { DynamicSectionBlock } from "../../../lib/schemas";
import { InlineText } from "./RichText";

type Content = Extract<DynamicSectionBlock, { blockType: "anchored_micro_theory" }>;

// Layered microtheory in exactly 3 parts — never one undifferentiated
// paragraph, and capped at 160 words combined (see `notebook_service`'s
// grounding check). Diagrams are their own block now
// (declarative_visual_diagram), never embedded here.
//
// The emoji on the left is OUR structural marker; the model often prefixes
// the same emoji into the text itself (it mirrors the prompt's examples).
// Strip that leading duplicate so the legend shows exactly once — doing it
// in the renderer fixes already-generated notebooks without regenerating.
function withoutLeadingMarker(text: string, marker: string): string {
  let t = text.trimStart();
  // The emoji alone, or with the U+FE0F variation selector the model may
  // or may not include after it.
  const bare = marker.replace("\u{FE0F}", "");
  for (const prefix of [marker, marker + "\u{FE0F}", bare]) {
    if (t.startsWith(prefix)) return t.slice(prefix.length).trimStart();
  }
  return text;
}

export function AnchoredMicroTheoryBlock({ content }: { content: Content }) {
  return (
    <div className="notebook-block-body theory-exposition">
      <h3>
        <InlineText text={content.title} />
      </h3>
      <p className="theory-part theory-hook">
        <span className="theory-part-emoji" aria-hidden="true">🎯</span>
        <InlineText text={withoutLeadingMarker(content.intuitiveHook, "🎯")} />
      </p>
      <p className="theory-part theory-rule">
        <span className="theory-part-emoji" aria-hidden="true">📐</span>
        <InlineText text={withoutLeadingMarker(content.systemRule, "📐")} />
      </p>
      <p className="theory-part theory-error">
        <span className="theory-part-emoji" aria-hidden="true">⚠️</span>
        <InlineText text={withoutLeadingMarker(content.frequentError, "⚠️")} />
      </p>
    </div>
  );
}
