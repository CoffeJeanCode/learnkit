import { useState } from "react";
import type { StaticVisualSpec } from "../../../lib/schemas";
import { diffRow, symbolsOf } from "../../../lib/sequenceStrip";

type Spec = Extract<StaticVisualSpec, { renderEngine: "sequence_strip" }>;

/** Something read in pieces, drawn as it is read: symbol tiles grouped into
 *  chunks, each chunk's meaning right under it. Extra rows are variants of
 *  the first one: the symbols that changed and the meanings that moved are
 *  marked by us (`lib/sequenceStrip.ts`), never described by the model. */
export function SequenceStripVisual({ visual }: { visual: Spec }) {
  const [open, setOpen] = useState<string | null>(null);
  const base = visual.rows[0]?.chunks ?? [];
  const compare = visual.rows.length > 1;

  const note = (() => {
    if (!open) return null;
    const [r, c] = open.split(":").map(Number);
    const chunk = visual.rows[r]?.chunks[c];
    return chunk ? { title: `${chunk.text} — ${chunk.meaning}`, body: chunk.explanation } : null;
  })();

  return (
    <figure className="static-visual seq-strip">
      {visual.title && <strong className="seq-title">{visual.title}</strong>}
      <div className="seq-rows">
        {visual.rows.map((row, r) => {
          const diffs = r > 0 ? diffRow(base, row.chunks) : null;
          return (
            <div key={r} className="seq-row">
              <span className="seq-row-label">{row.label}</span>
              <ol className="seq-chunks" aria-label={row.label}>
                {row.chunks.map((chunk, c) => {
                  const diff = diffs?.[c];
                  const changed = !!diff && (diff.changedSymbols.size > 0 || diff.meaningChanged);
                  const id = `${r}:${c}`;
                  return (
                    <li key={c} className={`seq-chunk${changed ? " is-changed" : ""}`}>
                      <button
                        type="button"
                        className="seq-chunk-btn"
                        aria-pressed={open === id}
                        aria-label={`${chunk.text}: ${chunk.meaning}${changed ? " (cambia)" : ""}`}
                        onClick={() => setOpen((cur) => (cur === id ? null : id))}
                      >
                        <span className="seq-tiles">
                          {symbolsOf(chunk.text).map((s, k) => (
                            <span key={k} className={`seq-tile${diff?.changedSymbols.has(k) ? " is-changed" : ""}`}>
                              {s}
                            </span>
                          ))}
                        </span>
                        <span className={`seq-meaning${diff?.meaningChanged ? " is-changed" : ""}`}>{chunk.meaning}</span>
                      </button>
                    </li>
                  );
                })}
              </ol>
            </div>
          );
        })}
      </div>
      {note ? (
        <aside className="node-note" role="note" aria-live="polite">
          <strong>{note.title}</strong>
          {note.body && <p>{note.body}</p>}
        </aside>
      ) : (
        <p className="hint node-note-hint">{compare ? "Lo marcado en rojo es lo que cambia. " : ""}Toca un trozo para ver qué hace.</p>
      )}
      {visual.caption && <figcaption className="hint">{visual.caption}</figcaption>}
    </figure>
  );
}
