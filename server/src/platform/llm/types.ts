/**
 * The one shape every model request and answer takes inside Oya: OpenAI's chat
 * completions. Each provider is a strategy that translates to and from its own API,
 * so the agent loop, its tools and its tests never know which model answered.
 */

/** What a relayed model call came back with: the status, the raw event stream, and any retry-after. */
export type RelayedAnswer = {
  /** The HTTP status the model endpoint answered. */
  status: number;
  /** The whole response body, an event stream. */
  text: string;
  /** The endpoint's retry-after header, when it sent one. */
  retryAfter?: string;
};

/** Carries one model request body to wherever the credential lives (a desktop) and answers the raw stream. */
export type ModelTransport = (body: unknown) => Promise<RelayedAnswer>;

/** One chat request, as chatCompletion takes it. */
export type ChatRequest = {
  /** Provider base URL; it picks the provider (see index.ts). */
  baseUrl: string;
  /** Provider API key. */
  apiKey: string;
  /** Model name. */
  model: string;
  /** OpenAI-shaped messages. */
  messages: any[];
  /** OpenAI-shaped tool definitions. */
  tools?: any[];
  /** OpenAI tool_choice; defaults to 'auto' when tools are given. */
  toolChoice?: any;
  /** Sends the request from elsewhere (the person's desktop) when the credential is not this server's to use. */
  transport?: ModelTransport;
  /**
   * 'low' asks a thinking model to think little: work like page translation needs no
   * reasoning, and Gemini 3 spent most of 20–35 s a batch on it. Not 'minimal': Gemini
   * 3.8 Flash refuses that level with a 400. Sent only where the provider is known to
   * take it; elsewhere it is left out.
   */
  effort?: 'low';
};

/** One answer, OpenAI-shaped: the assistant message and the tokens it cost. */
export type ChatResponse = {
  /** The assistant's message: text, tool calls, and whatever the provider needs repeated next turn in `extra_content`. */
  choices: ChatChoice[];
  /** Tokens billed. */
  usage?: ChatUsage;
};

/** One answer the model gave. */
export type ChatChoice = {
  /** The assistant message, OpenAI-shaped. */
  message: any;
};

/** Tokens one request cost. */
export type ChatUsage = {
  /** Everything read, cached reads and cache writes included. */
  prompt_tokens: number;
  /** Everything written, thinking included. */
  completion_tokens: number;
};

/** A model API: which base URLs it serves, and how one request is sent to it. */
export interface LlmProvider {
  /** A short name for logs. */
  readonly name: string;
  /** Whether this provider serves `baseUrl`. */
  handles(baseUrl: string): boolean;
  /** Sends one request (one attempt) and answers it OpenAI-shaped; throws LlmError on failure. */
  send(request: ChatRequest): Promise<ChatResponse>;
}
