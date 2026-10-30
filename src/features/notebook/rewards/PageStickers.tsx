import { useRef, useState } from "react";
import { type Piece, STICKER_SIZE, isSticker, toPagePlacement } from "../../../lib/rewards";
import { sfx } from "../../../lib/sound";
import { useRewards } from "../../../stores/rewards";
import { PeelSticker } from "./PeelSticker";

/** The data attribute that marks the page stickers can be stuck on (see `StickerSheet`). */
export const STICKER_LAYER_ATTR = "data-sticker-layer";

/**
 * The stickers the student has stuck on the notebook page. They scroll with the content, can be
 * picked up and moved (a press that barely moves is just a hover, so the peel still works), and a
 * double-click takes one back to the sheet. This is the notebook's own personalisation: where each
 * sticker sits is remembered per notebook.
 */
export function PageStickers({ notebookId }: { notebookId: string }) {
  const stickers = (useRewards((s) => s.byNotebook[notebookId]) ?? []).filter((p) => isSticker(p) && p.page);
  if (stickers.length === 0) return null;
  return (
    <div className="page-stickers">
      {stickers.map((p) => (
        <PageSticker key={p.id} notebookId={notebookId} piece={p} />
      ))}
    </div>
  );
}

function PageSticker({ notebookId, piece }: { notebookId: string; piece: Piece }) {
  const page = piece.page!;
  const place = useRewards.getState().placeSticker;
  const [drag, setDrag] = useState<{ x: number; y: number } | null>(null);
  const press = useRef<{ dx: number; dy: number; x0: number; y0: number; moved: boolean; rect: DOMRect } | null>(null);

  const layerRect = (el: HTMLElement) => (el.closest(`[${STICKER_LAYER_ATTR}]`) as HTMLElement | null)?.getBoundingClientRect() ?? el.parentElement!.getBoundingClientRect();

  return (
    <div
      className={`page-sticker${drag ? " is-dragging" : ""}`}
      style={{
        left: drag ? `${drag.x}px` : `${page.x * 100}%`,
        top: drag ? `${drag.y}px` : `${page.y}px`,
        width: STICKER_SIZE,
        height: STICKER_SIZE,
        transform: `translate(-50%, -50%) rotate(${page.rot}deg)`,
      }}
      title={`${piece.why} Arrástralo para moverlo; doble clic para guardarlo.`}
      onDoubleClick={() => place(notebookId, piece.id, null)}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        const rect = layerRect(e.currentTarget);
        e.currentTarget.setPointerCapture(e.pointerId);
        // Offset of the pointer from the sticker's centre, so it does not jump to the cursor.
        const cx = rect.left + page.x * rect.width;
        const cy = rect.top + page.y;
        press.current = { dx: e.clientX - cx, dy: e.clientY - cy, x0: e.clientX, y0: e.clientY, moved: false, rect };
      }}
      onPointerMove={(e) => {
        const p = press.current;
        if (!p) return;
        if (!p.moved && Math.hypot(e.clientX - p.x0, e.clientY - p.y0) < 5) return;
        p.moved = true;
        setDrag({ x: e.clientX - p.dx - p.rect.left, y: e.clientY - p.dy - p.rect.top });
      }}
      onPointerUp={(e) => {
        const p = press.current;
        press.current = null;
        if (!p?.moved) return;
        setDrag(null);
        place(notebookId, piece.id, toPagePlacement(e.clientX - p.dx, e.clientY - p.dy, p.rect, page.rot));
        sfx.stick();
      }}
      onPointerCancel={() => {
        press.current = null;
        setDrag(null);
      }}
    >
      <PeelSticker shape={piece.shape} tone={piece.tone} size={STICKER_SIZE} label={piece.why} />
    </div>
  );
}
