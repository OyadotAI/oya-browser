/**
 * The title and hints every MCP tool is registered with. Connector
 * directories (Anthropic's among them) require a title and read-only or
 * destructive hints on each tool, and Claude's tool picker shows the title.
 * One table for the three registration sites, so a tool cannot be added to one
 * without a decision here: toolConfig throws on a name it does not know.
 */
import type { z } from 'zod';

/** What a tool is called and what calling it can do. */
export interface ToolMeta {
  /** The short name a person sees, Title Case, verb first for an action. */
  title: string;
  /** True when the tool changes nothing. */
  readOnlyHint: boolean;
  /** True when the tool can undo work or remove something: close, stop, clear, submit. */
  destructiveHint: boolean;
  /** True when the tool reaches arbitrary websites; false for Oya's own state. */
  openWorldHint: boolean;
}

/** A tool that only looks. */
const reads = (title: string, openWorldHint = true): ToolMeta => ({
  title,
  readOnlyHint: true,
  destructiveHint: false,
  openWorldHint,
});
/** A tool that acts on a page but can be undone by the person. */
const writes = (title: string, openWorldHint = true): ToolMeta => ({
  title,
  readOnlyHint: false,
  destructiveHint: false,
  openWorldHint,
});
/** A tool that removes, stops or submits something. */
const destroys = (title: string, openWorldHint = true): ToolMeta => ({
  title,
  readOnlyHint: false,
  destructiveHint: true,
  openWorldHint,
});

/**
 * Every tool, by name. Read-only tools never destroy; page tools are open
 * world; what reads or changes Oya's own state (pool, playbooks, saved logins)
 * is closed world. Tools that can submit a form on a site are destructive.
 */
export const TOOL_META: Record<string, ToolMeta> = {
  // Reading the page
  analyze_page: reads('Analyze Page'),
  screenshot: reads('Take Screenshot'),
  read_elements: reads('Read Elements'),
  find: reads('Find Elements'),
  wait: reads('Wait for Element'),
  wait_for: reads('Wait for Page'),
  list_tabs: reads('List Tabs'),
  read_console: reads('Read Console'),
  read_network: reads('Read Network'),
  read_trace: reads('Read Trace'),
  search_history: reads('Search History'),
  list_bookmarks: reads('List Bookmarks'),
  list_closed_tabs: reads('List Closed Tabs'),
  list_notifications: reads('List Notifications'),
  list_keyboard_shortcuts: reads('List Keyboard Shortcuts'),
  // Acting on the page
  navigate: writes('Navigate'),
  run_script: writes('Run Page Script'),
  click: writes('Click Element'),
  type: writes('Type Text'),
  press_key: writes('Press Key'),
  scroll: writes('Scroll Page'),
  scroll_at: writes('Scroll at Point'),
  hover: writes('Hover Element'),
  select_option: writes('Select Option'),
  edit_text: writes('Edit Text'),
  click_coordinates: writes('Click at Coordinates'),
  double_click: writes('Double Click'),
  mouse_move: writes('Move Mouse'),
  drag: writes('Drag'),
  keyboard_type: writes('Type on Keyboard'),
  key_chord: writes('Press Key Combination'),
  handle_dialog: writes('Answer Dialog'),
  go_back: writes('Go Back'),
  go_forward: writes('Go Forward'),
  reload: writes('Reload Page'),
  open_tab: writes('Open Tab'),
  switch_tab: writes('Switch Tab'),
  reopen_closed_tab: writes('Reopen Closed Tab'),
  add_bookmark: writes('Add Bookmark'),
  mark_notifications_read: writes('Mark Notifications Read'),
  start_trace: writes('Start Trace'),
  stop_trace: writes('Stop Trace'),
  solve_captcha: writes('Solve CAPTCHA'),
  sign_in: writes('Sign In'),
  complete_mfa: writes('Complete MFA'),
  // Removing or submitting
  close_tab: destroys('Close Tab'),
  remove_bookmark: destroys('Remove Bookmark'),
  clear_history: destroys('Clear History'),
  dismiss_notification: destroys('Dismiss Notification'),
  run_task: destroys('Run Task'),
  run_playbook: destroys('Run Playbook'),
  // Oya's own state
  list_saved_logins: reads('List Saved Logins', false),
  list_playbooks: reads('List Playbooks', false),
  pool_status: reads('Pool Status', false),
  start_browser: writes('Start Browser', false),
  stop_browser: destroys('Stop Browser', false),
};

/** The registerTool config for `name`: its title and hints, with the description and schema passed through. */
export function toolConfig<S extends z.ZodRawShape>(name: string, description: string, inputSchema: S) {
  if (!Object.hasOwn(TOOL_META, name)) throw new Error(`MCP tool ${name} has no entry in annotations.ts`);
  const { title, ...hints } = TOOL_META[name];
  return { title, description, inputSchema, annotations: { title, ...hints } };
}
