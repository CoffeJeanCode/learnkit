export type ProviderKind = "openai" | "anthropic" | "gemini" | "openrouter" | "deepseek";

export interface ProviderConfig {
  id: string;
  provider: string;
  name: string;
  default_model: string | null;
  base_url: string | null;
}

export interface ProviderWithStatus extends ProviderConfig {
  configured: boolean;
}

export interface ModelInfo {
  id: string;
  name: string | null;
}

export interface ModelRef {
  provider_id: string;
  model: string;
}

export interface AgentDefinition {
  id: string;
  name: string;
  description: string | null;
  system_prompt: string;
  model: ModelRef;
  tools: string[];
}

export interface ToolInfo {
  id: string;
  name: string;
  description: string;
}

export interface AgentOutput {
  run_id: string;
  agent_id: string;
  parent_run_id: string | null;
  text: string;
  tool_calls: string[];
  duration_ms: number;
}

export interface ChatMessage {
  role: "user" | "assistant" | "system" | "status" | "error";
  content: string;
  run_id?: string;
}

export interface AgentEventPayload {
  event: string;
  run_id: string;
  agent_id: string;
  parent_run_id: string | null;
  text: string | null;
  tool: string | null;
  error: string | null;
}

export interface AppError {
  code: string;
  message: string;
}

export type View = "chat" | "agents" | "workflows" | "providers" | "roadmap" | "notebook";

// Roadmap & Syllabus Diagnostic Agent types live in `../lib/schemas` now —
// they're Zod schemas (the runtime validation boundary for the agent's JSON
// contract), with `z.infer` types derived from them, not hand-duplicated
// here. Import `RoadmapSession`, `RoadmapPhase`, etc. from `../lib/schemas`.
// Same for the Notebook engine types (`Course`, `ClassRecord`,
// `NotebookPayload`, etc.).

/** One block's edited content, sent to `save_notebook_state`. */
export interface BlockUpdate {
  id: string;
  content_json: Record<string, unknown>;
}
