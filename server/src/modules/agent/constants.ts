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
export const MAX_ANALYSIS_CHARS = 20_000;
/** Conversation size (~4 chars per token) past which old tool results are dropped. */
export const MAX_CONTEXT_CHARS = 400_000;
/**
 * Page reads kept in full. Element ids are only valid until the next
 * analyze_page, so older pages are dead weight the model still pays for on
 * every turn. Keeping the last two costs little and saves the agent from
 * reading a page again only to compare it with the one before.
 */
export const PAGES_KEPT_IN_FULL = 1;
/** A page read is at least this long: shorter tool results are answers, not pages. */
export const PAGE_RESULT_CHARS = 2_000;
/** What a screenshot turn counts for in the context budget: an image costs about as much as this much text. */
export const IMAGE_CONTEXT_CHARS = 6_000;
/** Characters a dropped turn leaves in the running record of earlier steps. */
export const DROPPED_STEP_CHARS = 120;
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
/** The longest cycle of calls the loop guard looks for. */
const LONGEST_CYCLE = 3;
/** Cycle lengths the loop guard looks for: A A A, A B A B, A B C A B C. */
export const CYCLE_LENGTHS = Array.from({ length: LONGEST_CYCLE }, (_, i) => i + 1);
/** Times a cycle longer than one call must repeat before the model is warned. */
export const CYCLE_REPEATS = 2;
/** Warnings a run may get for going round in circles before it is stopped. */
export const STUCK_NOTES_ALLOWED = 3;
/** Empty replies in a row a run may get before it is stopped. */
export const EMPTY_REPLIES_ALLOWED = 2;
/** Times a run's report may be sent back by the check before it is accepted as it stands. */
export const VERIFY_ROUNDS = 2;
/** Characters of the final page the check reads. */
export const VERIFY_PAGE_CHARS = 12_000;
/** Of the run's calls, how many (the latest) the check reads. */
export const VERIFY_CALLS_SHOWN = 40;
/** OYA_AGENT_VERIFY=0 turns the check before done off (it costs one short model call per run). */
export const AGENT_VERIFY = process.env.OYA_AGENT_VERIFY !== '0';
/** Recent calls the loop guard remembers: enough for the longest cycle twice over, and then some. */
export const GUARD_HISTORY = 12;
/** Hex characters of a result's digest kept in a call's key: plenty to tell two results apart. */
export const RESULT_DIGEST_CHARS = 16;
/** OYA_AGENT_LOG=1 prints each tool call and the start of its result, for debugging a run. */
export const AGENT_LOG = process.env.OYA_AGENT_LOG === '1';
/** Characters of a call's arguments and result the agent log prints when OYA_AGENT_LOG_CHARS is unset. */
const DEFAULT_AGENT_LOG_CHARS = 300;
/** Characters of a call's arguments and result the agent log prints. */
export const AGENT_LOG_CHARS = Number(process.env.OYA_AGENT_LOG_CHARS) || DEFAULT_AGENT_LOG_CHARS;
/** Agent loop iterations when CHAT_MAX_ITERATIONS is unset. */
export const DEFAULT_MAX_ITERATIONS = '200';
/** The most of a run_script result the model reads, in characters. */
export const RUN_SCRIPT_OUTPUT_CHARS = 8_000;
/** How long wait_for waits by default, in ms. */
export const WAIT_FOR_DEFAULT_MS = 10_000;
/** The longest wait_for may wait, in ms: under the browser command's own timeout. */
export const WAIT_FOR_MAX_MS = 20_000;
/** How often wait_for looks at the page, in ms. */
export const WAIT_FOR_POLL_MS = 250;
/** How long no new request must start before the page counts as settled, in ms. */
export const NETWORK_QUIET_MS = 500;
/** How many times wait_for starts again after the page it was watching navigated away. */
export const WAIT_FOR_TRIES = 3;
/** How many elements find lists. */
export const FIND_LIMIT = 15;
/** The most notes the agent keeps for one site; the oldest go first. */
export const MAX_SITE_NOTES = 20;
/** The longest one note may be, in characters. */
export const MAX_NOTE_CHARS = 300;
/** How many steps before the limit the agent is told to wrap up and report what it has. */
export const WRAP_UP_STEPS = 5;
