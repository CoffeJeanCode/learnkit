import React, { useState } from "react";
import { useSessionObserver } from "./sessionObserver";

export function SessionMetricsModal() {
  const currentSession = useSessionObserver((s) => s.currentSession);
  const pastSessions = useSessionObserver((s) => s.pastSessions);
  const modalOpen = useSessionObserver((s) => s.modalOpen);
  const setModalOpen = useSessionObserver((s) => s.setModalOpen);
  const exportMarkdown = useSessionObserver((s) => s.exportMarkdown);
  const [copied, setCopied] = useState(false);
  const [selectedPastIndex, setSelectedPastIndex] = useState<number | null>(null);

  if (!modalOpen) return null;

  const session = selectedPastIndex !== null && pastSessions[selectedPastIndex]
    ? pastSessions[selectedPastIndex]
    : currentSession;

  const handleCopy = async () => {
    try {
      const md = exportMarkdown();
      await navigator.clipboard.writeText(md);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      console.error("No se pudo copiar el reporte:", e);
    }
  };

  const completed = session?.blocks.filter((b) => b.totalDurationMs !== null) ?? [];
  const avgDuration = completed.length > 0
    ? (completed.reduce((acc, b) => acc + (b.totalDurationMs ?? 0), 0) / completed.length / 1000).toFixed(2)
    : "0.00";
  const avgTtft = completed.length > 0
    ? (completed.reduce((acc, b) => acc + (b.ttftMs ?? 0), 0) / completed.length / 1000).toFixed(2)
    : "0.00";

  const totalInputTokens = completed.reduce((acc, b) => acc + b.tokens.input, 0);
  const totalCachedTokens = completed.reduce((acc, b) => acc + b.tokens.cached, 0);
  const totalReasoningTokens = completed.reduce((acc, b) => acc + b.tokens.reasoning, 0);
  const totalOutputTokens = completed.reduce((acc, b) => acc + b.tokens.output, 0);

  const nativeCount = completed.filter((b) => b.toolResolution === "native").length;
  const nativePct = completed.length > 0 ? Math.round((nativeCount / completed.length) * 100) : 100;

  return (
    <div className="graph-modal-backdrop">
      <div className="graph-modal-box flex flex-col overflow-hidden" style={{ maxWidth: "860px", maxHeight: "90vh", width: "100%", padding: 0 }}>
        {/* Encabezado */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-soft bg-paper-deep">
          <div>
            <div className="flex items-center gap-2">
              <span className="w-2.5 h-2.5 rounded-full animate-pulse" style={{ backgroundColor: "var(--green-chalk)" }} />
              <h2 className="text-base font-semibold text-chalk">
                Observador de Sesión en Vivo: Telemetría y Comparador de Modelos
              </h2>
            </div>
            {session && (
              <p className="text-xs text-chalk-soft mt-0.5">
                Clase: <span className="text-chalk-soft font-mono">{session.classId}</span> • Proveedor:{" "}
                <span className="text-chalk-soft font-mono">{session.provider}</span> • Modelo:{" "}
                <span className="text-gold font-mono font-medium">{session.model}</span>
              </p>
            )}
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleCopy}
              className="px-3 py-1.5 text-xs font-medium rounded-md bg-card hover:bg-card-hi text-chalk-soft border border-soft transition-colors"
            >
              {copied ? "✓ Copiado en Markdown" : "Copiar Reporte"}
            </button>
            <button
              type="button"
              onClick={() => setModalOpen(false)}
              className="p-1.5 text-chalk-soft hover:text-chalk rounded-md hover:bg-card transition-colors"
              title="Cerrar"
            >
              ✕
            </button>
          </div>
        </div>

        {/* Pestañas de comparación de sesión */}
        {pastSessions.length > 0 && (
          <div className="flex items-center gap-2 px-6 py-2 bg-paper-deep border-b border-soft text-xs overflow-x-auto">
            <span className="text-chalk-faint font-medium">Comparar:</span>
            <button
              type="button"
              onClick={() => setSelectedPastIndex(null)}
              className={`px-2.5 py-1 rounded-md transition-colors ${
                selectedPastIndex === null
                  ? "bg-gold-wash text-gold font-medium border-soft"
                  : "text-chalk-soft hover:bg-card hover:text-chalk"
              }`}
            >
              Sesión Actual ({currentSession?.model})
            </button>
            {pastSessions.map((ps, idx) => (
              <button
                key={ps.sessionId}
                type="button"
                onClick={() => setSelectedPastIndex(idx)}
                className={`px-2.5 py-1 rounded-md transition-colors ${
                  selectedPastIndex === idx
                    ? "bg-gold-wash text-gold font-medium border-soft"
                    : "text-chalk-soft hover:bg-card hover:text-chalk"
                }`}
              >
                {ps.model.replace(/^.*\//, "")} ({new Date(ps.startedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })})
              </button>
            ))}
          </div>
        )}

        {/* Contenido scrolleable */}
        <div className="p-6 overflow-y-auto space-y-6">
          {/* Tarjetas de métricas agregadas */}
          <div className="grid grid-cols-2 sm:grid-cols-5 gap-3">
            <div className="bg-card-hi border border-soft p-3 rounded-md">
              <span className="text-[11px] text-chalk-soft font-medium">TTFT Promedio</span>
              <div className="text-xl font-bold font-mono text-green mt-1">{avgTtft}s</div>
              <span className="text-[10px] text-chalk-faint">primer token</span>
            </div>
            <div className="bg-card-hi border border-soft p-3 rounded-md">
              <span className="text-[11px] text-chalk-soft font-medium">Duración Bloque</span>
              <div className="text-xl font-bold font-mono text-blue mt-1">{avgDuration}s</div>
              <span className="text-[10px] text-chalk-faint">latencia total</span>
            </div>
            <div className="bg-card-hi border border-soft p-3 rounded-md">
              <span className="text-[11px] text-chalk-soft font-medium">Fiabilidad Tool</span>
              <div
                className={`text-xl font-bold font-mono mt-1 ${
                  nativePct === 100 ? "text-green" : nativePct >= 80 ? "text-gold" : "text-red"
                }`}
              >
                {nativePct}%
              </div>
              <span className="text-[10px] text-chalk-faint">invocación nativa</span>
            </div>
            <div className="bg-card-hi border border-soft p-3 rounded-md">
              <span className="text-[11px] text-chalk-soft font-medium">Tokens Salida</span>
              <div className="text-xl font-bold font-mono text-gold mt-1">{totalOutputTokens}</div>
              <span className="text-[10px] text-chalk-faint">acumulados</span>
            </div>
            <div className="bg-card-hi border border-soft p-3 rounded-md">
              <span className="text-[11px] text-chalk-soft font-medium">Thinking Tokens</span>
              <div
                className={`text-xl font-bold font-mono mt-1 ${
                  totalReasoningTokens === 0 ? "text-green" : "text-gold"
                }`}
              >
                {totalReasoningTokens}
              </div>
              <span className="text-[10px] text-chalk-faint">{totalReasoningTokens === 0 ? "desactivado (rápido)" : "razonando"}</span>
            </div>
          </div>

          {/* Desglose de Caché y Tokens */}
          <div className="flex flex-wrap items-center gap-4 text-xs font-mono bg-paper-deep p-3 rounded-md border border-soft">
            <span className="text-chalk-soft">Tokens Totales:</span>
            <span>Entrada: <strong className="text-chalk">{totalInputTokens}</strong></span>
            <span>KV-Caché: <strong className="text-green">{totalCachedTokens}</strong></span>
            <span>Salida: <strong className="text-gold">{totalOutputTokens}</strong></span>
            {totalInputTokens > 0 && (
              <span className="text-chalk-soft">
                (Ahorro de Caché:{" "}
                <strong className="text-green">
                  {Math.round((totalCachedTokens / (totalInputTokens + totalCachedTokens)) * 100 || 0)}%
                </strong>
                )
              </span>
            )}
          </div>

          {/* Tabla de bloques */}
          <div>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-chalk-soft mb-3">
              Historial Cronológico de Bloques Generados
            </h3>
            <div className="border border-soft rounded-md overflow-hidden">
              <table className="w-full text-left text-xs font-mono">
                <thead className="bg-paper-deep text-chalk-soft border-b border-soft">
                  <tr>
                    <th className="py-2.5 px-3 font-medium">#</th>
                    <th className="py-2.5 px-3 font-medium">Tipo de Bloque</th>
                    <th className="py-2.5 px-3 font-medium text-center">Intentos</th>
                    <th className="py-2.5 px-3 font-medium text-right">TTFT</th>
                    <th className="py-2.5 px-3 font-medium text-right">Duración</th>
                    <th className="py-2.5 px-3 font-medium text-right">In/Cache Toks</th>
                    <th className="py-2.5 px-3 font-medium text-right">Out Toks</th>
                    <th className="py-2.5 px-3 font-medium text-center">Resolución</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-800/60">
                  {session?.blocks.map((b) => (
                    <tr key={b.blockIndex} className="hover:bg-card-hi transition-colors">
                      <td className="py-2.5 px-3 text-chalk-soft font-bold">{b.blockIndex}</td>
                      <td className="py-2.5 px-3 font-sans font-medium text-chalk">
                        {b.blockType ?? (
                          <span className="inline-flex items-center gap-1 text-gold">
                            <span className="w-1.5 h-1.5 rounded-full animate-ping" style={{ backgroundColor: "var(--gold-chalk)" }} />
                            generando...
                          </span>
                        )}
                      </td>
                      <td className="py-2.5 px-3 text-center">
                        <span
                          className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-bold ${
                            b.attempts === 1
                              ? "bg-card text-chalk-soft"
                              : "bg-gold-wash text-gold border-soft"
                          }`}
                        >
                          {b.attempts}
                        </span>
                      </td>
                      <td className="py-2.5 px-3 text-right text-chalk-soft">
                        {b.ttftMs !== null ? `${(b.ttftMs / 1000).toFixed(2)}s` : "-"}
                      </td>
                      <td className="py-2.5 px-3 text-right font-bold text-chalk">
                        {b.totalDurationMs !== null ? `${(b.totalDurationMs / 1000).toFixed(2)}s` : "-"}
                      </td>
                      <td className="py-2.5 px-3 text-right text-chalk-soft">
                        {b.tokens.input} / <span className="text-green">{b.tokens.cached}</span>
                      </td>
                      <td className="py-2.5 px-3 text-right text-gold">{b.tokens.output}</td>
                      <td className="py-2.5 px-3 text-center">
                        <span
                          className={`inline-block px-2 py-0.5 rounded text-[10px] font-medium uppercase ${
                            b.toolResolution === "native"
                              ? "bg-green-wash text-green border-soft"
                              : b.toolResolution === "salvaged"
                              ? "bg-gold-wash text-gold border-soft"
                              : b.toolResolution === "backstop"
                              ? "bg-blue-wash text-blue border-soft"
                              : "bg-rose-500/20 text-red border border-rose-500/30"
                          }`}
                        >
                          {b.toolResolution}
                        </span>
                      </td>
                    </tr>
                  ))}
                  {(!session || session.blocks.length === 0) && (
                    <tr>
                      <td colSpan={8} className="py-8 text-center text-chalk-faint font-sans">
                        Aún no se han generado bloques en esta sesión. Abre una clase para observar métricas en vivo.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
