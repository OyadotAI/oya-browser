/**
 * Every number the chat agent runs on, by name: timeouts, output caps and the
 * limits that keep a long run inside the model's context window.
 */

/** Browsers whose last run is kept in memory; the oldest is evicted past this. */
export const MAX_RECORDED_RUNS = 1000;
/** Values shorter than this are not redacted; doing so would blank unrelated text. */
export const MIN_REDACT_LENGTH = 3;
/** Uploads carry the whole file in one evaluate, so they get longer than a command. */
export const UPLOAD_TIMEOUT_MS = 120_000;
/** Navigation waits on slow sites. */
export const NAVIGATE_TIMEOUT_MS = 90_000;
/** Off-screen elements listed by name in an analysis; the rest are counted. */
export const MAX_OFFSCREEN_LISTED = 30;
/** Characters of a link the element index shows. */
export const MAX_INDEX_LINK = 80;
/** Elements read_elements lists when the model gives no limit. */
export const READ_ELEMENTS_LIMIT = 50;
/**
 * The format analyze_page writes the page in for the agent and MCP clients, when
 * the server pins one (markdown, toon or jsonl). Unset, each browser uses the
 * default chosen in its settings (markdown unless changed). Read from the
 * environment here and only here.
 */
export const PAGE_FORMAT: string | undefined = process.env.OYA_PAGE_FORMAT || undefined;
/** An analysis longer than this is cut, so one page cannot fill the context window. */
export const MAX_ANALYSIS_CHARS = 30_000;
/** Conversation size (~4 chars per token) past which old tool results are dropped. */
export const MAX_CONTEXT_CHARS = 400_000;
/** Messages always kept when trimming: the system prompt and the task. */
export const MIN_MESSAGES_KEPT = 3;
/** Bytes carried by one base64 quad. */
export const BASE64_QUAD_BYTES = 3;
/** Characters in one base64 quad. */
export const BASE64_QUAD_CHARS = 4;
/** Bytes per KB, and KB per MB, for file sizes shown to the model. */
export const BYTES_PER_KB = 1024;
/** Identical tool calls in a row after which the model is told its approach is not working. */
export const REPEAT_WARNING_AFTER = 3;
/** OYA_AGENT_LOG=1 prints each tool call and the start of its result, for debugging a run. */
export const AGENT_LOG = process.env.OYA_AGENT_LOG === '1';
/** Characters of a call's arguments and result the agent log prints when OYA_AGENT_LOG_CHARS is unset. */
const DEFAULT_AGENT_LOG_CHARS = 300;
/** Characters of a call's arguments and result the agent log prints. */
export const AGENT_LOG_CHARS = Number(process.env.OYA_AGENT_LOG_CHARS) || DEFAULT_AGENT_LOG_CHARS;
/** Agent loop iterations when CHAT_MAX_ITERATIONS is unset. */
export const DEFAULT_MAX_ITERATIONS = '200';
