import { BRICK_UNIT, type Piece, TONE_COLOR, pieceSize } from "../../../lib/rewards";

// SVG path data on a 24x24 grid, centred on (12, 12). One path per die-cut sticker.
export const STICKER_PATHS: Record<string, string> = {
  star: "M12 2l3.1 6.3 6.9 1-5 4.9 1.2 6.9L12 17.8 5.8 21.1 7 14.2 2 9.3l6.9-1z",
  bolt: "M13 2L4 14h6l-1 8 9-12h-6z",
  heart: "M12 21s-8-5.3-8-11a4.5 4.5 0 0 1 8-2.8A4.5 4.5 0 0 1 20 10c0 5.7-8 11-8 11z",
  flame: "M12 2c1 4 5 6 5 11a5 5 0 0 1-10 0c0-2 1-3.5 2-4.5 0 2 1 3 2 3 0-3-1-5 1-9.5z",
};
const paths = new Map<string, Path2D>();
const path2d = (shape: string) => {
  let p = paths.get(shape);
  if (!p) {
    p = new Path2D(STICKER_PATHS[shape]);
    paths.set(shape, p);
  }
  return p;
};

/** Mixes a hex colour toward black (<0) or white (>0): the lit top and shaded foot of a brick. */
function shade(hex: string, amount: number): string {
  const n = parseInt(hex.slice(1), 16);
  const mix = (c: number) => Math.round(amount < 0 ? c * (1 + amount) : c + (255 - c) * amount);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map(mix);
  return `rgb(${r}, ${g}, ${b})`;
}

/** Draws one piece centred on the current origin (the caller translates and rotates). */
export function drawPiece(ctx: CanvasRenderingContext2D, piece: Piece, lifted: boolean): void {
  const { w, h } = pieceSize(piece);
  const base = TONE_COLOR[piece.tone];
  if (lifted) {
    ctx.shadowColor = "rgba(0, 0, 0, 0.45)";
    ctx.shadowBlur = 14;
    ctx.shadowOffsetY = 6;
  }

  if (piece.shape === "brick") {
    // Studs first, so the body overlaps their base.
    const stud = BRICK_UNIT * 0.5;
    ctx.fillStyle = shade(base, -0.12);
    for (let i = 0; i < piece.cols; i++) {
      const cx = -w / 2 + BRICK_UNIT * (i + 0.5);
      ctx.beginPath();
      ctx.roundRect(cx - stud / 2, -h / 2 - 5, stud, 8, 2);
      ctx.fill();
    }
    ctx.fillStyle = base;
    ctx.beginPath();
    ctx.roundRect(-w / 2, -h / 2, w, h, 4);
    ctx.fill();
    ctx.shadowColor = "transparent";
    // A lit top edge and a shaded foot read as plastic.
    ctx.fillStyle = shade(base, 0.28);
    ctx.fillRect(-w / 2 + 2, -h / 2 + 1, w - 4, 3);
    ctx.fillStyle = shade(base, -0.22);
    ctx.fillRect(-w / 2 + 2, h / 2 - 4, w - 4, 3);
    return;
  }

  // Die-cut sticker: a thick paper-white border around the shape, then the colour.
  const scale = w / 24;
  ctx.save();
  ctx.scale(scale, scale);
  ctx.translate(-12, -12);
  ctx.lineJoin = "round";
  ctx.lineWidth = 3.4;
  ctx.strokeStyle = "#f3efe4";
  ctx.stroke(path2d(piece.shape));
  ctx.shadowColor = "transparent";
  ctx.fillStyle = base;
  ctx.fill(path2d(piece.shape));
  ctx.fillStyle = "rgba(255, 255, 255, 0.28)";
  ctx.save();
  ctx.clip(path2d(piece.shape));
  ctx.fillRect(0, 0, 24, 9);
  ctx.restore();
  ctx.restore();
}

/** Draws a translucent ghost preview showing where a brick will snap on drop. */
export function drawSnapGhost(ctx: CanvasRenderingContext2D, piece: Piece): void {
  const { w, h } = pieceSize(piece);
  const base = TONE_COLOR[piece.tone];
  const stud = BRICK_UNIT * 0.5;

  ctx.save();
  ctx.shadowColor = base;
  ctx.shadowBlur = 12;

  // Ghost body
  ctx.fillStyle = shade(base, 0.1);
  ctx.globalAlpha = 0.38;
  ctx.beginPath();
  ctx.roundRect(-w / 2, -h / 2, w, h, 4);
  ctx.fill();

  // Ghost studs on top
  for (let i = 0; i < piece.cols; i++) {
    const cx = -w / 2 + BRICK_UNIT * (i + 0.5);
    ctx.beginPath();
    ctx.roundRect(cx - stud / 2, -h / 2 - 5, stud, 8, 2);
    ctx.fill();
  }

  // Dashed outline
  ctx.globalAlpha = 0.85;
  ctx.lineWidth = 1.8;
  ctx.strokeStyle = "#ffffff";
  ctx.setLineDash([4, 3]);
  ctx.beginPath();
  ctx.roundRect(-w / 2, -h / 2, w, h, 4);
  ctx.stroke();

  for (let i = 0; i < piece.cols; i++) {
    const cx = -w / 2 + BRICK_UNIT * (i + 0.5);
    ctx.beginPath();
    ctx.roundRect(cx - stud / 2, -h / 2 - 5, stud, 8, 2);
    ctx.stroke();
  }

  // Interlocking alignment dots at bottom
  ctx.setLineDash([]);
  ctx.fillStyle = "#ffffff";
  ctx.globalAlpha = 0.9;
  for (let i = 0; i < piece.cols; i++) {
    const cx = -w / 2 + BRICK_UNIT * (i + 0.5);
    ctx.beginPath();
    ctx.arc(cx, h / 2 - 2, 2.2, 0, Math.PI * 2);
    ctx.fill();
  }

  ctx.restore();
}
