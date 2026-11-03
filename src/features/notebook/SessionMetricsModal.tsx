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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 animate-in fade-in duration-200">
      <div className="bg-slate-900 border border-slate-700/80 rounded-xl shadow-2xl max-w-4xl w-full max-h-[90vh] flex flex-col overflow-hidden text-slate-200">
        {/* Encabezado */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-slate-900/60">
          <div>
            <div className="flex items-center gap-2">
              <span className="w-2.5 h-2.5 rounded-full bg-emerald-500 animate-pulse" />
              <h2 className="text-base font-semibold text-slate-100">
                Observador de Sesión en Vivo: Telemetría y Comparador de Modelos
              </h2>
            </div>
            {session && (
              <p className="text-xs text-slate-400 mt-0.5">
                Clase: <span className="text-slate-300 font-mono">{session.classId}</span> • Proveedor:{" "}
                <span className="text-slate-300 font-mono">{session.provider}</span> • Modelo:{" "}
                <span className="text-amber-400 font-mono font-medium">{session.model}</span>
              </p>
            )}
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleCopy}
              className="px-3 py-1.5 text-xs font-medium rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 transition-colors"
            >
              {copied ? "✓ Copiado en Markdown" : "Copiar Reporte"}
            </button>
            <button
              type="button"
              onClick={() => setModalOpen(false)}
              className="p-1.5 text-slate-400 hover:text-slate-100 rounded-lg hover:bg-slate-800 transition-colors"
              title="Cerrar"
            >
              ✕
            </button>
          </div>
        </div>

        {/* Pestañas de comparación de sesión */}
        {pastSessions.length > 0 && (
          <div className="flex items-center gap-2 px-6 py-2 bg-slate-950/40 border-b border-slate-800 text-xs overflow-x-auto">
            <span className="text-slate-500 font-medium">Comparar:</span>
            <button
              type="button"
              onClick={() => setSelectedPastIndex(null)}
              className={`px-2.5 py-1 rounded-md transition-colors ${
                selectedPastIndex === null
                  ? "bg-amber-500/20 text-amber-300 font-medium border border-amber-500/40"
                  : "text-slate-400 hover:bg-slate-800 hover:text-slate-200"
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
                    ? "bg-amber-500/20 text-amber-300 font-medium border border-amber-500/40"
                    : "text-slate-400 hover:bg-slate-800 hover:text-slate-200"
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
            <div className="bg-slate-800/50 border border-slate-800 p-3 rounded-lg">
              <span className="text-[11px] text-slate-400 font-medium">TTFT Promedio</span>
              <div className="text-xl font-bold font-mono text-emerald-400 mt-1">{avgTtft}s</div>
              <span className="text-[10px] text-slate-500">primer token</span>
            </div>
            <div className="bg-slate-800/50 border border-slate-800 p-3 rounded-lg">
              <span className="text-[11px] text-slate-400 font-medium">Duración Bloque</span>
              <div className="text-xl font-bold font-mono text-sky-400 mt-1">{avgDuration}s</div>
              <span className="text-[10px] text-slate-500">latencia total</span>
            </div>
            <div className="bg-slate-800/50 border border-slate-800 p-3 rounded-lg">
              <span className="text-[11px] text-slate-400 font-medium">Fiabilidad Tool</span>
              <div
                className={`text-xl font-bold font-mono mt-1 ${
                  nativePct === 100 ? "text-emerald-400" : nativePct >= 80 ? "text-amber-400" : "text-rose-400"
                }`}
              >
                {nativePct}%
              </div>
              <span className="text-[10px] text-slate-500">invocación nativa</span>
            </div>
            <div className="bg-slate-800/50 border border-slate-800 p-3 rounded-lg">
              <span className="text-[11px] text-slate-400 font-medium">Tokens Salida</span>
              <div className="text-xl font-bold font-mono text-amber-400 mt-1">{totalOutputTokens}</div>
              <span className="text-[10px] text-slate-500">acumulados</span>
            </div>
            <div className="bg-slate-800/50 border border-slate-800 p-3 rounded-lg">
              <span className="text-[11px] text-slate-400 font-medium">Thinking Tokens</span>
              <div
                className={`text-xl font-bold font-mono mt-1 ${
                  totalReasoningTokens === 0 ? "text-emerald-400" : "text-amber-400"
                }`}
              >
                {totalReasoningTokens}
              </div>
              <span className="text-[10px] text-slate-500">{totalReasoningTokens === 0 ? "desactivado (rápido)" : "razonando"}</span>
            </div>
          </div>

          {/* Desglose de Caché y Tokens */}
          <div className="flex flex-wrap items-center gap-4 text-xs font-mono bg-slate-950/60 p-3 rounded-lg border border-slate-800">
            <span className="text-slate-400">Tokens Totales:</span>
            <span>Entrada: <strong className="text-slate-200">{totalInputTokens}</strong></span>
            <span>KV-Caché: <strong className="text-emerald-400">{totalCachedTokens}</strong></span>
            <span>Salida: <strong className="text-amber-300">{totalOutputTokens}</strong></span>
            {totalInputTokens > 0 && (
              <span className="text-slate-400">
                (Ahorro de Caché:{" "}
                <strong className="text-emerald-400">
                  {Math.round((totalCachedTokens / (totalInputTokens + totalCachedTokens)) * 100 || 0)}%
                </strong>
                )
              </span>
            )}
          </div>

          {/* Tabla de bloques */}
          <div>
            <h3 className="text-xs font-semibold uppercase tracking-wider text-slate-400 mb-3">
              Historial Cronológico de Bloques Generados
            </h3>
            <div className="border border-slate-800 rounded-lg overflow-hidden">
              <table className="w-full text-left text-xs font-mono">
                <thead className="bg-slate-950/80 text-slate-400 border-b border-slate-800">
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
                    <tr key={b.blockIndex} className="hover:bg-slate-800/30 transition-colors">
                      <td className="py-2.5 px-3 text-slate-400 font-bold">{b.blockIndex}</td>
                      <td className="py-2.5 px-3 font-sans font-medium text-slate-200">
                        {b.blockType ?? (
                          <span className="inline-flex items-center gap-1 text-amber-400">
                            <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-ping" />
                            generando...
                          </span>
                        )}
                      </td>
                      <td className="py-2.5 px-3 text-center">
                        <span
                          className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-bold ${
                            b.attempts === 1
                              ? "bg-slate-800 text-slate-400"
                              : "bg-amber-500/20 text-amber-300 border border-amber-500/30"
                          }`}
                        >
                          {b.attempts}
                        </span>
                      </td>
                      <td className="py-2.5 px-3 text-right text-slate-300">
                        {b.ttftMs !== null ? `${(b.ttftMs / 1000).toFixed(2)}s` : "-"}
                      </td>
                      <td className="py-2.5 px-3 text-right font-bold text-slate-100">
                        {b.totalDurationMs !== null ? `${(b.totalDurationMs / 1000).toFixed(2)}s` : "-"}
                      </td>
                      <td className="py-2.5 px-3 text-right text-slate-400">
                        {b.tokens.input} / <span className="text-emerald-400">{b.tokens.cached}</span>
                      </td>
                      <td className="py-2.5 px-3 text-right text-amber-300">{b.tokens.output}</td>
                      <td className="py-2.5 px-3 text-center">
                        <span
                          className={`inline-block px-2 py-0.5 rounded text-[10px] font-medium uppercase ${
                            b.toolResolution === "native"
                              ? "bg-emerald-500/20 text-emerald-300 border border-emerald-500/30"
                              : b.toolResolution === "salvaged"
                              ? "bg-amber-500/20 text-amber-300 border border-amber-500/30"
                              : b.toolResolution === "backstop"
                              ? "bg-purple-500/20 text-purple-300 border border-purple-500/30"
                              : "bg-rose-500/20 text-rose-300 border border-rose-500/30"
                          }`}
                        >
                          {b.toolResolution}
                        </span>
                      </td>
                    </tr>
                  ))}
                  {(!session || session.blocks.length === 0) && (
                    <tr>
                      <td colSpan={8} className="py-8 text-center text-slate-500 font-sans">
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
