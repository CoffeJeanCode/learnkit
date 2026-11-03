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
      className="session-observer-badge group"
    >
      <span className="w-2 h-2 rounded-full animate-pulse" style={{ backgroundColor: "var(--green-chalk)" }} />
      <span className="font-semibold text-chalk group-hover:text-gold transition-colors">
        {shortModel}
      </span>
      {lastTtft && (
        <>
          <span className="text-chalk-faint">|</span>
          <span className="text-chalk-soft">
            TTFT <strong className="text-chalk">{lastTtft}</strong>
          </span>
        </>
      )}
      {lastDuration && (
        <>
          <span className="text-chalk-faint">|</span>
          <span className="text-chalk-soft">
            Tot <strong className="text-chalk">{lastDuration}</strong>
          </span>
        </>
      )}
      {completed.length > 0 && (
        <>
          <span className="text-chalk-faint">|</span>
          <span
            className={
              nativePct === 100
                ? "text-green font-medium"
                : nativePct >= 80
                ? "text-gold font-medium"
                : "text-red font-medium"
            }
          >
            {nativePct}% tool
          </span>
        </>
      )}
    </button>
  );
}
