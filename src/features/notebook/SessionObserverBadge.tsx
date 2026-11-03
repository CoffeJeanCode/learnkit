import React from "react";
import { useSessionObserver } from "./sessionObserver";

export function SessionObserverBadge() {
  const currentSession = useSessionObserver((s) => s.currentSession);
  const setModalOpen = useSessionObserver((s) => s.setModalOpen);

  if (!currentSession) return null;

  const completed = currentSession.blocks.filter((b) => b.totalDurationMs !== null);
  const lastBlock = completed.length > 0 ? completed[completed.length - 1] : null;

  // Último TTFT medido
  const lastTtft = lastBlock?.ttftMs !== null && lastBlock?.ttftMs !== undefined
    ? `${(lastBlock.ttftMs / 1000).toFixed(1)}s`
    : null;

  // Última duración total
  const lastDuration = lastBlock?.totalDurationMs !== null && lastBlock?.totalDurationMs !== undefined
    ? `${(lastBlock.totalDurationMs / 1000).toFixed(1)}s`
    : null;

  // % de llamadas a tool nativas vs salvadas/fallidas
  const nativeCount = completed.filter((b) => b.toolResolution === "native").length;
  const nativePct = completed.length > 0 ? Math.round((nativeCount / completed.length) * 100) : 100;

  // Formato conciso del modelo (ej: "xiaomi/mimo-v2.6-pro" -> "mimo-v2.6-pro")
  const shortModel = currentSession.model.replace(/^.*\//, "");

  return (
    <button
      type="button"
      onClick={() => setModalOpen(true)}
      title="Observador de Sesión en Vivo (clic para ver desglose de latencia, tokens y métricas de modelos)"
      className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-mono rounded-full bg-slate-800/80 hover:bg-slate-700/90 text-slate-300 border border-slate-700/80 shadow-sm transition-colors cursor-pointer group"
    >
      <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
      <span className="font-semibold text-slate-200 group-hover:text-amber-300 transition-colors">
        {shortModel}
      </span>
      {lastTtft && (
        <>
          <span className="text-slate-600">|</span>
          <span className="text-slate-400">
            TTFT <strong className="text-slate-200">{lastTtft}</strong>
          </span>
        </>
      )}
      {lastDuration && (
        <>
          <span className="text-slate-600">|</span>
          <span className="text-slate-400">
            Tot <strong className="text-slate-200">{lastDuration}</strong>
          </span>
        </>
      )}
      {completed.length > 0 && (
        <>
          <span className="text-slate-600">|</span>
          <span
            className={
              nativePct === 100
                ? "text-emerald-400 font-medium"
                : nativePct >= 80
                ? "text-amber-400 font-medium"
                : "text-rose-400 font-medium"
            }
          >
            {nativePct}% tool
          </span>
        </>
      )}
    </button>
  );
}
