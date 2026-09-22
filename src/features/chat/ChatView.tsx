import { useEffect, useState } from "react";
import { useAgents } from "../../stores/agents";
import { useChat } from "../../stores/chat";

export function ChatView() {
  const { agents, refresh } = useAgents();
  const { agentId, selectAgent, messages, running, status, error, send, clear } = useChat();
  const [input, setInput] = useState("");

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const submit = () => {
    send(input);
    setInput("");
  };

  const current = agents.find((a) => a.id === agentId);

  return (
    <div className="view chat">
      <h2>Chat playground</h2>
      <div className="row">
        <label>
          Agent
          <select value={agentId ?? ""} onChange={(e) => selectAgent(e.target.value || null)}>
            <option value="">— select —</option>
            {agents.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name} ({a.model.provider_id}/{a.model.model})
              </option>
            ))}
          </select>
        </label>
        {current && (
          <span className="badge">
            {current.model.provider_id}/{current.model.model}
          </span>
        )}
        <button onClick={clear}>Clear history</button>
      </div>

      <div className="messages">
        {messages.length === 0 && <p className="muted">No messages yet. History lives in memory only.</p>}
        {messages.map((m, i) => (
          <div key={i} className={`msg ${m.role}`}>
            <div className="msg-role">{m.role}</div>
            <div className="msg-body">{m.content}</div>
          </div>
        ))}
        {status && <div className="msg status"><div className="msg-body">{status}</div></div>}
      </div>

      {error && <div className="alert error">{error}</div>}

      <div className="row composer">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submit()}
          placeholder={agentId ? "Type a message…" : "Select an agent first"}
          disabled={!agentId || running}
        />
        <button onClick={submit} disabled={!agentId || running || !input.trim()}>
          {running ? "Running…" : "Run"}
        </button>
      </div>
    </div>
  );
}
