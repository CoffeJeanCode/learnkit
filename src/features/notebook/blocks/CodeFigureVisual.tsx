import { useEffect, useMemo, useRef, useState } from "react";
import type { StaticVisualSpec } from "../../../lib/schemas";
import { codeFigureDocument } from "./codeFigureSdk";

type Spec = Extract<StaticVisualSpec, { renderEngine: "code_figure" }>;

/** How long the figure has to say it started before it counts as broken.
 *  The dev-only figure preview stretches it: headless screenshots run on
 *  virtual time, which fast-forwards this timer past the frame's real load. */
const readyTimeoutMs = () => (globalThis as { __figureReadyTimeoutMs?: number }).__figureReadyTimeoutMs ?? 6000;

/** Tier-2 figure: model-written code running in an isolated iframe (no same
 *  origin, no network). The frame is built imperatively — `sandbox` first,
 *  then its document, then into the page — so the document never loads
 *  before its restrictions exist. The parent only listens for
 *  "ready" / "error" from that one frame; a figure that throws or never starts
 *  degrades to its caption with a note, never to a blank box. */
export function CodeFigureVisual({ visual }: { visual: Spec }) {
  const hostRef = useRef<HTMLDivElement>(null);
  const [state, setState] = useState<"pending" | "ready" | "error">("pending");
  const [error, setError] = useState("");
  const height = Math.min(640, Math.max(200, visual.height));
  const doc = useMemo(() => codeFigureDocument(visual.code, height), [visual.code, height]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    setState("pending");
    setError("");
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.title = visual.title ?? "Figura interactiva";
    frame.className = "code-figure-frame";
    frame.style.height = `${height + 140}px`;
    const onMessage = (e: MessageEvent) => {
      if (e.source !== frame.contentWindow) return;
      const data = e.data as { source?: string; type?: string; message?: string };
      if (data?.source !== "code-figure") return;
      if (data.type === "ready") setState((s) => (s === "error" ? s : "ready"));
      if (data.type === "error") {
        setState("error");
        setError(data.message ?? "");
      }
    };
    window.addEventListener("message", onMessage);
    // A Blob URL, not `srcdoc`: the sandbox still gives it an opaque origin,
    // and it loads reliably wherever the frame is created.
    const url = URL.createObjectURL(new Blob([doc], { type: "text/html" }));
    frame.src = url;
    host.appendChild(frame);
    const timer = window.setTimeout(() => setState((s) => (s === "pending" ? "error" : s)), readyTimeoutMs());
    return () => {
      window.removeEventListener("message", onMessage);
      window.clearTimeout(timer);
      frame.remove();
      URL.revokeObjectURL(url);
    };
  }, [doc, height, visual.title]);

  return (
    <figure className="static-visual code-figure">
      {visual.title && <strong className="scene3d-title">{visual.title}</strong>}
      <div ref={hostRef} hidden={state === "error"} />
      {state === "error" && (
        <div className="alert error">
          <span className="alert-text">Esta figura no se pudo ejecutar; el texto de arriba explica la idea.</span>
          {error && <span className="hint">{error}</span>}
        </div>
      )}
      <figcaption className="hint">{visual.caption}</figcaption>
    </figure>
  );
}
