/**
 * The shapes the Ask pane reads from the main process. The IPC contract
 * (src/shared/ipc.ts) types these payloads loosely; the Ask pane owns them,
 * so their precise shapes live here and are read at the bridge's edge.
 */

/** One tool call the agent made in a run. */
export interface ToolCall {
  /** The tool's name, e.g. `click`. */
  name: string;
}

/** The answer to `sendChat`. */
export interface ChatAnswer {
  /** The agent's reply. */
  text?: string;
  /** Why it failed, in words for the person. */
  error?: string;
  /** A machine code for the failure (`llm_unconfigured`, `llm_rejected`). */
  code?: string;
  /** The tools the run called. */
  toolCalls?: ToolCall[];
  /** Whether the server recorded steps a playbook can replay (absent on an older server). */
  replayable?: boolean;
}

/** One model a provider offers. */
export interface ModelOption {
  /** The id the provider knows it by. */
  id: string;
  /** Its name for people ("Vendor: Model" on OpenRouter). */
  label: string;
  /** An id typed by the person, not in the list. */
  custom?: boolean;
}

/** One provider in the server's catalog. */
export interface CatalogEntry {
  /** Its id, e.g. `openai`. */
  id: string;
  /** Its name for people. */
  label: string;
  /** What its keys look like, as the key field's placeholder. */
  hint?: string;
  /** Where to get a key. */
  keysUrl?: string;
  /** Its default model. */
  model?: string;
  /** The models it offers. */
  models?: ModelOption[];
}

/** The answer to `modelStatus`. */
export interface ModelStatus {
  /** This browser belongs to a project. */
  signedIn?: boolean;
  /** The project has a model key. */
  hasLlmKey?: boolean;
  /** The providers the server offers. */
  catalog?: CatalogEntry[];
  /** The provider in use. */
  provider?: string;
  /** The model in use. */
  model?: string;
}

/** One step of the plan the agent wrote. */
export interface PlanStep {
  /** What the step is. */
  step?: string;
  /** Whether it is done. */
  done?: boolean;
}

/** One event of an agent run (the `event` of an `onAgentEvent`). */
export interface AgentRunEvent {
  /** `start`, `plan`, `step`, or kinds the pane ignores. */
  kind?: string;
  /** A plan's steps. */
  steps?: PlanStep[];
  /** A step's narrated line. */
  line?: string;
  /** A step's tool, when it has no line. */
  tool?: string;
}

/** An activity entry (`onDevLog`). */
export interface DevLogEntry {
  /** `in` for what the server sent, `out` for what went back. */
  dir?: string;
  /** What it is, e.g. `cmd: navigate`. */
  type?: string;
  /** Its JSON, possibly cut short. */
  data?: string;
}
