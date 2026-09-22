import { useEffect, useState } from "react";
import { runAgent, tauriError } from "../../lib/tauri";
import { useAgents } from "../../stores/agents";
import { useProviders } from "../../stores/providers";
import type { AgentDefinition } from "../../types";

const EMPTY: AgentDefinition = {
  id: "",
  name: "",
  description: "",
  system_prompt: "",
  model: { provider_id: "openai", model: "" },
  tools: [],
};

export function AgentsView() {
  const { agents, tools, loading, error, refresh, create, remove } = useAgents();
  const providers = useProviders((s) => s.providers);
  const refreshProviders = useProviders((s) => s.refresh);
  const [form, setForm] = useState<AgentDefinition>(EMPTY);
  const [testing, setTesting] = useState<string | null>(null);

  useEffect(() => {
    refresh();
    refreshProviders();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggleTool = (id: string) => {
    setForm((f) => ({
      ...f,
      tools: f.tools.includes(id) ? f.tools.filter((t) => t !== id) : [...f.tools, id],
    }));
  };

  const submit = async () => {
    const def: AgentDefinition = {
      ...form,
      id: form.id.trim().toLowerCase().replace(/\s+/g, "-"),
      description: form.description || null,
    };
    try {
      await create(def);
      setForm(EMPTY);
    } catch {
      // error already in store
    }
  };

  return (
    <div className="view">
      <h2>Agents</h2>
      {error && <div className="alert error">{error}</div>}
      {loading && <p>Loading…</p>}

      <div className="grid">
        {agents.map((a) => (
          <div className="card" key={a.id}>
            <div className="card-head">
              <h3>{a.name}</h3>
              <span className="badge">{a.model.provider_id}/{a.model.model}</span>
            </div>
            {a.description && <p className="muted">{a.description}</p>}
            <div className="hint">tools: {a.tools.length ? a.tools.join(", ") : "none"}</div>
            <div className="row">
              <button onClick={() => setTesting(testing === a.id ? null : a.id)}>
                {testing === a.id ? "Hide test" : "Test run"}
              </button>
              <button onClick={() => remove(a.id)}>Delete</button>
            </div>
            {testing === a.id && <AgentQuickRun agentId={a.id} />}
          </div>
        ))}
      </div>

      <h3>Create agent</h3>
      <div className="card form">
        <label>ID<input value={form.id} onChange={(e) => setForm({ ...form, id: e.target.value })} placeholder="my-agent" /></label>
        <label>Name<input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} /></label>
        <label>Description<input value={form.description ?? ""} onChange={(e) => setForm({ ...form, description: e.target.value })} /></label>
        <label>System prompt<textarea value={form.system_prompt} onChange={(e) => setForm({ ...form, system_prompt: e.target.value })} rows={3} /></label>
        <div className="row2">
          <label>Provider
            <select
              value={form.model.provider_id}
              onChange={(e) => setForm({ ...form, model: { ...form.model, provider_id: e.target.value } })}
            >
              {providers.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </label>
          <label>Model<input value={form.model.model} onChange={(e) => setForm({ ...form, model: { ...form.model, model: e.target.value } })} placeholder="model id" /></label>
        </div>
        <div>
          <div className="muted">Tools</div>
          {tools.map((t) => (
            <label key={t.id} className="check">
              <input type="checkbox" checked={form.tools.includes(t.id)} onChange={() => toggleTool(t.id)} />
              {t.id} — {t.description}
            </label>
          ))}
        </div>
        <button onClick={submit}>Save agent</button>
      </div>
    </div>
  );
}

function AgentQuickRun({ agentId }: { agentId: string }) {
  const [input, setInput] = useState("Say hello in one sentence.");
  const [output, setOutput] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const run = async () => {
    setBusy(true);
    setOutput(null);
    try {
      const out = await runAgent(agentId, input);
      setOutput(out.text);
    } catch (e) {
      const err = tauriError(e);
      setOutput(`ERROR ${err.code}: ${err.message}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="quickrun">
      <input value={input} onChange={(e) => setInput(e.target.value)} />
      <button disabled={busy} onClick={run}>{busy ? "Running…" : "Run"}</button>
      {output && <pre>{output}</pre>}
    </div>
  );
}
