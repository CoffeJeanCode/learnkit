import { useState } from "react";
import { delegateAgent, runResearchToDraft, tauriError } from "../../lib/tauri";

export function WorkflowsView() {
  const [topic, setTopic] = useState("Benefits of multi-agent systems");
  const [output, setOutput] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const runDemo = async () => {
    setBusy(true);
    setOutput(null);
    setError(null);
    try {
      const out = await runResearchToDraft(topic);
      setOutput(`[${out.agent_id} · ${out.duration_ms}ms · run ${out.run_id}]\n\n${out.text}`);
    } catch (e) {
      const err = tauriError(e);
      setError(`${err.code}: ${err.message}`);
    } finally {
      setBusy(false);
    }
  };

  const runDelegate = async () => {
    setBusy(true);
    setOutput(null);
    setError(null);
    try {
      const out = await delegateAgent("researcher", "writer", topic);
      setOutput(`[${out.agent_id} · parent ${out.parent_run_id}]\n\n${out.text}`);
    } catch (e) {
      const err = tauriError(e);
      setError(`${err.code}: ${err.message}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="view">
      <h2>Workflows</h2>
      <p className="muted">
        Minimal orchestration demo: <code>researcher → writer</code>. No workflow engine yet —
        sequential/parallel/supervisor patterns plug into the Orchestrator later.
      </p>
      <div className="card">
        <h3>Research → Draft</h3>
        <label>Topic<input value={topic} onChange={(e) => setTopic(e.target.value)} /></label>
        <div className="row">
          <button disabled={busy} onClick={runDemo}>
            {busy ? "Running…" : "Run research → draft"}
          </button>
          <button disabled={busy} onClick={runDelegate}>
            Delegate researcher → writer
          </button>
        </div>
      </div>
      {error && <div className="alert error">{error}</div>}
      {output && <pre className="output">{output}</pre>}
    </div>
  );
}
