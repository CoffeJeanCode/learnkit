import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { RoadmapSessionSchema, RoadmapTurnPayloadSchema } from "./schemas";
import type {
  AgentDefinition,
  AgentEventPayload,
  AgentOutput,
  AppError,
  ModelInfo,
  ProviderWithStatus,
  ToolInfo,
} from "../types";

export function tauriError(e: unknown): AppError {
  if (typeof e === "string") return { code: "Unknown", message: e };
  if (e && typeof e === "object" && "message" in e) {
    const e2 = e as Record<string, unknown>;
    return {
      code: typeof e2.code === "string" ? e2.code : "Unknown",
      message: String(e2.message ?? e),
    };
  }
  return { code: "Unknown", message: String(e) };
}

// --- Providers -------------------------------------------------------------

export const listProviders = () => invoke<ProviderWithStatus[]>("list_providers");

export const saveProvider = (config: {
  id: string;
  provider: string;
  name: string;
  default_model: string | null;
  base_url: string | null;
}) => invoke<ProviderWithStatus>("save_provider", { config });

export const saveProviderKey = (provider_id: string, api_key: string) =>
  invoke<boolean>("save_provider_key", { providerId: provider_id, apiKey: api_key });

export const providerHasKey = (provider_id: string) =>
  invoke<boolean>("provider_has_key", { providerId: provider_id });

export const deleteProviderKey = (provider_id: string) =>
  invoke<void>("delete_provider_key", { providerId: provider_id });

export const testProvider = (provider_id: string) =>
  invoke<void>("test_provider", { providerId: provider_id });

export const listModels = (provider_id: string) =>
  invoke<ModelInfo[]>("list_models", { providerId: provider_id });

export const suggestedModels = (provider: string) =>
  invoke<string[]>("suggested_models", { provider });

// --- Agents ----------------------------------------------------------------

export const listAgents = () => invoke<AgentDefinition[]>("list_agents");

export const createAgent = (agent: AgentDefinition) =>
  invoke<AgentDefinition>("create_agent", { agent });

export const deleteAgent = (agent_id: string) =>
  invoke<void>("delete_agent", { agentId: agent_id });

export const runAgent = (agent_id: string, input: string) =>
  invoke<AgentOutput>("run_agent", { agentId: agent_id, input });

export const listTools = () => invoke<ToolInfo[]>("list_tools");

// --- Chat / workflows ------------------------------------------------------

export const delegateAgent = (from_agent_id: string, to_agent_id: string, task: string) =>
  invoke<AgentOutput>("delegate_agent", {
    fromAgentId: from_agent_id,
    toAgentId: to_agent_id,
    task,
  });

export const runResearchToDraft = (topic: string) =>
  invoke<AgentOutput>("run_research_to_draft", { topic });

// --- Roadmap & Syllabus Diagnostic Agent ------------------------------------
//
// Every payload from the roadmap agent is parsed through the Zod schemas in
// `./schemas.ts` before the UI touches it — this is the "Generative UI"
// validation boundary: a malformed backend response fails loudly here
// instead of silently rendering `undefined` deep in a component.

export const startRoadmapSession = () =>
  invoke("start_roadmap_session").then((v) => RoadmapTurnPayloadSchema.parse(v));

export const sendRoadmapMessage = (session_id: string, message: string) =>
  invoke("send_roadmap_message", { sessionId: session_id, message }).then((v) =>
    RoadmapTurnPayloadSchema.parse(v),
  );

export const getRoadmapSession = (session_id: string) =>
  invoke("get_roadmap_session", { sessionId: session_id }).then((v) => RoadmapSessionSchema.parse(v));

// --- Events ----------------------------------------------------------------

export async function onAgentEvent(
  channel: "agent://started" | "agent://completed" | "agent://error",
  handler: (e: AgentEventPayload) => void,
): Promise<UnlistenFn> {
  return listen<AgentEventPayload>(channel, (evt) => handler(evt.payload));
}
