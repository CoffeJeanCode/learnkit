import type { DynamicSectionBlock } from "../../../lib/schemas";
import { InlineText } from "./RichText";

type Content = Extract<DynamicSectionBlock, { blockType: "anchored_micro_theory" }>;

// Layered microtheory in 3-4 parts — never one undifferentiated paragraph.
// `systemRule` has its own ~180-word ceiling and the other layers share a
// combined terse ceiling (see `notebook_service::grounding`). Diagrams are
// their own block now (declarative_visual_diagram), never embedded here.
//
// `analogyBoundary` is optional: absent/null on blocks persisted before the
// field existed (see `DynamicSectionBlockSchema` in `src/lib/schemas.ts` —
// `.nullish()` mirrors Rust's `Option<String>`), so it renders nothing for
// those, never a crash or a "formato inesperado" error.
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
      {content.analogyBoundary ? (
        <p className="theory-part theory-boundary">
          <span className="theory-part-emoji" aria-hidden="true">🚧</span>
          <InlineText text={withoutLeadingMarker(content.analogyBoundary, "🚧")} />
        </p>
      ) : null}
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
