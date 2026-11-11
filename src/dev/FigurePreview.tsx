// DEV ONLY (see figure-preview.html): renders the figure encoded in the URL
// hash — base64url of `{ "visual": StaticVisualSpec, "highlight"?: string }` —
// so scripts can screenshot any generated figure without touching the app.
import { createRoot } from "react-dom/client";
import "../styles.css";
import { StaticVisual } from "../features/notebook/blocks/StaticVisual";
import { StaticVisualSpecSchema } from "../lib/schemas";

function decode(hash: string): { visual: unknown; highlight?: string } | null {
  try {
    const b64 = hash.replace(/^#/, "").replace(/-/g, "+").replace(/_/g, "/");
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    return JSON.parse(new TextDecoder().decode(bytes));
  } catch {
    return null;
  }
}

// Headless screenshots run on virtual time: give code figures room to load.
(globalThis as { __figureReadyTimeoutMs?: number }).__figureReadyTimeoutMs = 600_000;

const payload = decode(location.hash);
const parsed = payload ? StaticVisualSpecSchema.safeParse(payload.visual) : null;
const root = createRoot(document.getElementById("root")!);
root.render(
  parsed?.success ? (
    <StaticVisual visual={parsed.data} highlightGroupId={payload?.highlight} />
  ) : (
    <p style={{ color: "#f87171" }} data-figure-error="1">
      Figura inválida: {parsed && !parsed.success ? parsed.error.issues[0]?.message : "hash ilegible"}
    </p>
  ),
);
