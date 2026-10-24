import { type BlockDraft, hasDraftContent } from "./blockDraft";
import { BLOCK_TYPE_LABEL } from "./blockTypeInfo";

/** Whitelisted reading fields the server may stream, in reading order. The
 *  server decides what is safe to send; this only decides how to label it. */
const FIELD_LABEL: Record<string, string | null> = {
  title: null,
  intuitiveHook: "La idea",
  systemRule: "La regla",
  frequentError: "Error frecuente",
};
const FIELD_ORDER = ["title", "intuitiveHook", "systemRule", "frequentError"];

/**
 * Draft of the block being written, shown in place of a bare spinner. Purely
 * a preview: it is replaced by the real block (and never submitted, edited or
 * graded). Renders nothing until the server has sent something worth showing.
 *
 * `showKind` draws the "Escribiendo: Explicación" line; a host that already
 * narrates the status (the generating card) turns it off to avoid saying it twice.
 */
export function BlockDraftPreview({ draft, showKind = true }: { draft: BlockDraft | null; showKind?: boolean }) {
  if (!hasDraftContent(draft)) return null;
  const typeLabel = draft.blockType ? BLOCK_TYPE_LABEL[draft.blockType] : undefined;
  const entries = FIELD_ORDER.filter((k) => draft.fields[k]).map((k) => [k, draft.fields[k]] as const);
  const reasoningOnly = entries.length === 0;

  return (
    // Deliberately NOT an aria-live region: the text grows every ~150 ms and a
    // screen reader would re-announce it constantly. The card's status line
    // (which only changes between phases) is the live region.
    <div className="block-draft" aria-label="Borrador del siguiente bloque">
      {showKind && (
        <div className="block-draft-kind">
          {typeLabel ? `Escribiendo: ${typeLabel}` : reasoningOnly ? "Pensando cómo explicártelo…" : "Escribiendo…"}
        </div>
      )}
      {entries.map(([key, text]) =>
        FIELD_LABEL[key] === null ? (
          <h4 key={key} className="block-draft-title">
            {text}
          </h4>
        ) : (
          <p key={key} className="block-draft-field">
            <span className="block-draft-label">{FIELD_LABEL[key]}</span> {text}
          </p>
        ),
      )}
    </div>
  );
}
