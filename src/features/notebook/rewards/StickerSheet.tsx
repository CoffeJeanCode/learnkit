import { useRef, useState } from "react";
import { type PagePlacement, type Piece, STICKER_SIZE, tiltFor, toPagePlacement } from "../../../lib/rewards";
import { sfx } from "../../../lib/sound";
import { PeelSticker } from "./PeelSticker";
import { STICKER_LAYER_ATTR } from "./PageStickers";

/**
 * The stickers that have not been stuck yet, as a sheet in the table's panel. Each one peels under
 * the pointer; drag one out of the panel and drop it on the notebook page to stick it there.
 * Dropping it back inside the panel leaves it on the sheet.
 */
export function StickerSheet({ stickers, onStick }: { stickers: readonly Piece[]; onStick: (id: string, page: PagePlacement) => void }) {
  const [drag, setDrag] = useState<{ piece: Piece; x: number; y: number } | null>(null);
  const press = useRef<{ piece: Piece; x0: number; y0: number; moved: boolean } | null>(null);
  if (stickers.length === 0) return null;

  const finish = (ev: React.PointerEvent) => {
    const p = press.current;
    press.current = null;
    setDrag(null);
    if (!p?.moved) return;
    const layer = document.querySelector(`[${STICKER_LAYER_ATTR}]`) as HTMLElement | null;
    const panel = (ev.currentTarget as HTMLElement).closest(".reward-panel");
    const over = document.elementFromPoint(ev.clientX, ev.clientY);
    // Dropped back on the panel (or there is no page to stick it on): it stays on the sheet.
    if (!layer || (panel && over && panel.contains(over))) return;
    onStick(p.piece.id, toPagePlacement(ev.clientX, ev.clientY, layer.getBoundingClientRect(), tiltFor(p.piece.id)));
    sfx.stick();
  };

  return (
    <div className="sticker-sheet" aria-label="Stickers por pegar">
      <span className="sticker-sheet-hint">Arrástralos a la página</span>
      <div className="sticker-sheet-row">
        {stickers.map((piece) => (
          <div
            key={piece.id}
            className={`sheet-sticker${drag?.piece.id === piece.id ? " is-lifted" : ""}`}
            title={piece.why}
            onPointerDown={(ev) => {
              if (ev.button !== 0) return;
              ev.currentTarget.setPointerCapture(ev.pointerId);
              press.current = { piece, x0: ev.clientX, y0: ev.clientY, moved: false };
            }}
            onPointerMove={(ev) => {
              const p = press.current;
              if (!p) return;
              if (!p.moved && Math.hypot(ev.clientX - p.x0, ev.clientY - p.y0) < 5) return;
              p.moved = true;
              setDrag({ piece: p.piece, x: ev.clientX, y: ev.clientY });
            }}
            onPointerUp={finish}
            onPointerCancel={() => {
              press.current = null;
              setDrag(null);
            }}
          >
            <PeelSticker shape={piece.shape} tone={piece.tone} size={56} label={piece.why} />
          </div>
        ))}
      </div>
      {drag && (
        <span className="sticker-ghost" style={{ left: drag.x, top: drag.y, width: STICKER_SIZE, height: STICKER_SIZE }} aria-hidden="true">
          <PeelSticker shape={drag.piece.shape} tone={drag.piece.tone} size={STICKER_SIZE} />
        </span>
      )}
    </div>
  );
}
