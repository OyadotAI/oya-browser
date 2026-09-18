/**
 * Every number the shell's renderer uses, by name. Loaded first; the other
 * renderer scripts read it as the global `RendererConstants`.
 */
/* exported RendererConstants */

/** Timeouts (milliseconds), panel sizes (CSS pixels) and display limits. */
const RendererConstants = Object.freeze({
  /** How long Connect waits for the server before saying it could not connect. */
  CONNECT_TIMEOUT_MS: 5000,
  /** How long a profile save may take before the button comes back. */
  PROFILE_SAVE_TIMEOUT_MS: 15000,
  /** How long "Copied!" shows on a chat message. */
  COPIED_MS: 1500,
  /** The tallest the Ask box grows before it scrolls. */
  CHAT_INPUT_MAX_PX: 160,
  /** Characters of the prompt suggested as a playbook's name. */
  PLAYBOOK_NAME_SUGGESTION: 48,
  /** The workspace panel's narrowest and widest. */
  PANEL_MIN_WIDTH: 320,
  PANEL_MAX_WIDTH: 560,
  /** The panel's width in compact studio mode. */
  PANEL_COMPACT_WIDTH: 360,
  /** The page keeps at least this much width beside the panel. */
  PAGE_MIN_WIDTH: 480,
  /** Below this window width the panel cannot be dragged. */
  RESIZE_MIN_WINDOW: 960,
  /** One arrow-key press on the resize handle. */
  PANEL_KEY_STEP: 16,
  /** The activity log keeps this many entries. */
  NET_LOG_LIMIT: 500,
  /** The activity log follows new entries while scrolled within this of the bottom. */
  NET_LOG_STICK_PX: 60,
  /** Characters of an analysis, and of any other action result, shown. */
  ANALYZE_PREVIEW: 5000,
  RESULT_PREVIEW: 3000,
  /** The most steps a draft may have (shown as "n / 500"). */
  MAX_STEPS: 500,
  /** The run timeline shows this many latest events. */
  RUN_EVENTS_SHOWN: 100,
  /** Zero-padded widths: step numbers, clock fields, milliseconds. */
  STEP_NUMBER_DIGITS: 2,
  CLOCK_DIGITS: 2,
  MS_DIGITS: 3,
  /** Indentation of JSON shown to people. */
  JSON_INDENT: 2,
});
