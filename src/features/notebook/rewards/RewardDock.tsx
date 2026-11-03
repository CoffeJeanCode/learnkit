import { useEffect, useState } from "react";
import { type PagePlacement, type Piece, type Placement, isBrick, isSticker } from "../../../lib/rewards";
import { useSound } from "../../../lib/sound";
import { useRewards } from "../../../stores/rewards";
import { BrickYard } from "./BrickYard";
import { StickerSheet } from "./StickerSheet";

/** How long the "new piece" line stays before the caption goes back to the hint. */
const EARNED_CAPTION_MS = 6000;

/**
 * The student's table, docked at the corner of the notebook. It stays out of
 * sight until the first piece is earned (no empty widget), opens by itself
 * when a new one drops so the reward is seen where it happens, and remembers
 * how the pieces were left. The caption says how a piece was earned: that is
 * the information the reward carries.
 */
export function RewardDock({ notebookId }: { notebookId: string }) {
  const pieces = useRewards((s) => s.byNotebook[notebookId]);
  const open = useRewards((s) => s.open);
  const lastEarned = useRewards((s) => s.lastEarned);
  const { load, place, placeSticker, setOpen } = useRewards.getState();
  const soundOn = useSound((s) => s.on);
  const [hover, setHover] = useState<Piece | null>(null);

  useEffect(() => {
    load(notebookId);
  }, [load, notebookId]);

  useEffect(() => {
    if (!lastEarned) return;
    const t = window.setTimeout(() => useRewards.setState({ lastEarned: null }), EARNED_CAPTION_MS);
    return () => window.clearTimeout(t);
  }, [lastEarned]);

  if (!pieces || pieces.length === 0) return null;

  // Do not alter caption on hover: tooltips on the pieces prevent layout shifting
  const caption = lastEarned
    ? `Pieza nueva: ${lastEarned.why}`
    : "Arma tus piezas sobre la mesa como legos: encajan en los studs y cada una cuenta cómo la ganaste.";
  return (
    <RewardDockView
      pieces={pieces}
      open={open}
      caption={caption}
      isNew={!!lastEarned}
      onOpen={() => setOpen(true)}
      onClose={() => setOpen(false)}
      onSettle={(placements) => place(notebookId, placements)}
      onStick={(id, page) => placeSticker(notebookId, id, page)}
      soundOn={soundOn}
      onToggleSound={() => useSound.getState().toggle()}
      onHover={setHover}
    />
  );
}

/** The table itself, driven only by props (the connected `RewardDock` above feeds it from the store). */
export function RewardDockView({
  pieces,
  open,
  caption,
  isNew,
  onOpen,
  onClose,
  onSettle,
  onStick,
  soundOn,
  onToggleSound,
  onHover,
}: {
  pieces: readonly Piece[];
  open: boolean;
  caption: string;
  isNew: boolean;
  onOpen: () => void;
  onClose: () => void;
  onSettle: (placements: Placement[]) => void;
  onStick: (id: string, page: PagePlacement) => void;
  soundOn: boolean;
  onToggleSound: () => void;
  onHover: (piece: Piece | null) => void;
}) {
  const bricks = pieces.filter(isBrick);
  const sheet = pieces.filter((p) => isSticker(p) && !p.page);
  return (
    <aside className={`reward-dock${open ? " is-open" : ""}`} aria-label="Tu mesa de piezas">
      {open ? (
        <div className="reward-panel">
          <header className="reward-head">
            <span className="pf-label">Tu mesa</span>
            <span className="reward-count">
              {pieces.length} {pieces.length === 1 ? "pieza" : "piezas"}
            </span>
            <button type="button" className="btn-quiet reward-sound" onClick={onToggleSound} aria-pressed={soundOn} aria-label={soundOn ? "Silenciar los sonidos" : "Activar los sonidos"}>
              {soundOn ? "Sonido: sí" : "Sonido: no"}
            </button>
            <button type="button" className="btn-quiet" onClick={onClose} aria-label="Cerrar tu mesa">
              Cerrar
            </button>
          </header>
          <StickerSheet stickers={sheet} onStick={onStick} />
          {bricks.length > 0 && <BrickYard pieces={bricks} onSettle={onSettle} onHover={onHover} />}
          <p className={`reward-caption${isNew ? " is-new" : ""}`} role="status" aria-live="polite">
            {caption}
          </p>
        </div>
      ) : (
        <button type="button" className="reward-tab" onClick={onOpen} aria-label={`Abrir tu mesa de piezas (${pieces.length})`}>
          <svg viewBox="0 0 28 22" width="28" height="22" aria-hidden="true">
            <rect x="3" y="9" width="22" height="11" rx="2.5" fill="currentColor" />
            <rect x="6" y="4" width="5" height="6" rx="1.5" fill="currentColor" />
            <rect x="17" y="4" width="5" height="6" rx="1.5" fill="currentColor" />
          </svg>
          <span className="reward-tab-count">{pieces.length}</span>
        </button>
      )}
    </aside>
  );
}
