// Colours shared by the 3D scenes and the plain-HTML parts around them
// (legend chips, message tables). Kept free of three.js so the React
// components can use it without pulling the 3D bundle in.

/** The palette the rest of the visuals already use (`StaticVisual.tsx`). */
export const PALETTE: Record<string, string> = {
  sky: "#38bdf8",
  green: "#4ade80",
  rose: "#f87171",
  amber: "#fbbf24",
  violet: "#c084fc",
  neutral: "#94a3b8",
};

const CYCLE = ["sky", "green", "rose", "amber", "violet"];

/** The colour of part `index`: its own when given, else cycling the palette. */
export function partColor(color: string | null | undefined, index: number): string {
  return PALETTE[color ?? ""] ?? PALETTE[CYCLE[index % CYCLE.length]];
}

export const TONE_COLOR: Record<string, string> = {
  request: PALETTE.sky,
  response: PALETTE.green,
  control: PALETTE.amber,
  error: PALETTE.rose,
};

export const TONE_LABEL: Record<string, string> = {
  request: "Petición",
  response: "Respuesta",
  control: "Control",
  error: "Error",
};

export function toneColor(tone?: string | null): string {
  return TONE_COLOR[tone ?? "request"] ?? TONE_COLOR.request;
}
