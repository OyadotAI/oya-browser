/**
 * Docs: the rest of the MCP tools, one table row each, grouped by what they
 * are for. The server's claims test checks every tool name appears here or in
 * mcp-tools.tsx, so the list cannot fall behind the code again.
 */
'use client';

import type { ReactNode } from 'react';
import { InlineCode, NoteBox, SectionHeading, SubHeading, Table } from '../_docs/blocks';

/** One row: the tool, its arguments (brackets mean optional), what it does. */
const tool = (name: string, args: string, does: string): ReactNode[] => [
  <InlineCode key="n">{name}</InlineCode>,
  args ? <InlineCode key="a">{args}</InlineCode> : 'none',
  does,
];

/** A group of tools under one heading. */
interface Group {
  /** Its anchor. */
  id: string;
  /** Its heading. */
  title: string;
  /** A line above the table, when the group needs one. */
  note?: string;
  /** Its rows. */
  rows: ReactNode[][];
}

/** Every tool not covered by its own section above, by group. */
const GROUPS: Group[] = [
  {
    id: 'page-actions',
    title: 'Page actions',
    rows: [
      tool(
        'find',
        'query',
        'The elements matching a description ("search box", "add to cart"), best first, with ids for click and type.',
      ),
      tool(
        'read_elements',
        '[selector], [limit]',
        'The element index without the page text, same ids as analyze_page. Per-browser endpoint only.',
      ),
      tool('select_option', 'element_id, option', 'Choose an option of a native select by its text.'),
      tool('hover', 'element_id', 'Move the mouse onto an element, for menus and tooltips that open on hover.'),
      tool(
        'edit_text',
        'operation',
        'Native select_all, undo or redo in the focused editor, using its own selection and undo history.' +
          ' Per-browser endpoint only.',
      ),
      tool(
        'scroll_at',
        'x, y, delta_x, delta_y',
        'Scroll under a point, for nested panels and horizontal tables. Coordinates come from a screenshot.' +
          ' Per-browser endpoint only.',
      ),
      tool(
        'handle_dialog',
        'accept, [prompt_text]',
        'Answer a confirm or prompt blocking the page. Alerts are answered automatically.',
      ),
      tool('go_back', '', 'Go back one page in the tab history and wait for the page.'),
      tool('go_forward', '', 'Go forward one page and wait for the page.'),
      tool('reload', '', 'Reload the tab and wait for the page.'),
      tool(
        'run_script',
        'script',
        'Run JavaScript that reads the page and returns JSON data, in an isolated world the page cannot see. It may not click, type, send requests or navigate.',
      ),
    ],
  },
  {
    id: 'mouse-keyboard',
    title: 'Mouse and keyboard',
    rows: [
      tool('click_coordinates', 'x, y', 'Click at a pixel position, when you know it from a screenshot.'),
      tool('double_click', '[element_id], [x], [y]', 'Double-click an element by id, or at a position.'),
      tool('mouse_move', 'x, y', 'Move the cursor to a position without clicking.'),
      tool(
        'drag',
        'from_x, from_y, to_x, to_y',
        'Drag from one point to another: sliders, drag-and-drop, selecting text.',
      ),
      tool('keyboard_type', 'text', 'Type into whatever is focused, no element needed.'),
      tool(
        'key_chord',
        'key',
        'Press a key combination in the page, such as Shift+Tab or ControlOrMeta+a. Never a browser shell shortcut.' +
          ' Per-browser endpoint only.',
      ),
      tool(
        'list_keyboard_shortcuts',
        '',
        "The connected browser's own keyboard shortcuts, read-only, so you can explain them to the person." +
          ' Per-browser endpoint only.',
      ),
    ],
  },
  {
    id: 'waiting-reading',
    title: 'Waiting and reading',
    rows: [
      tool(
        'wait_for',
        '[text], [url], [network_idle], [timeout]',
        'Wait until text shows, the URL contains something, or the network goes quiet (the default). At most 20000 ms.',
      ),
      tool(
        'read_console',
        '[level], [pattern], [limit]',
        'What the page logged. Oya browsers, per-browser endpoint only.',
      ),
      tool(
        'read_network',
        '[failed_only], [pattern], [limit]',
        'The requests the page made. Oya browsers, per-browser endpoint only.',
      ),
      tool(
        'start_trace',
        '',
        'Start a bounded network and console trace on the active tab before reproducing an issue. Returns a trace_id. No headers, cookies or bodies are kept.' +
          ' Per-browser endpoint only.',
      ),
      tool(
        'read_trace',
        'trace_id, [after], [limit]',
        'Read the next page of the trace, in order. Pass next_cursor as after to continue.' +
          ' Per-browser endpoint only.',
      ),
      tool(
        'stop_trace',
        'trace_id, [after], [limit]',
        'Stop the trace and read its final page.' + ' Per-browser endpoint only.',
      ),
    ],
  },
  {
    id: 'sign-in-captcha',
    title: 'Sign-in and CAPTCHA',
    rows: [
      tool('solve_captcha', '', 'Clear a CAPTCHA with the configured solver.'),
      tool('sign_in', '', "Sign in with the persona's stored credentials for the site. Credentials never reach you."),
      tool('complete_mfa', '', "Enter a one-time code from the persona's factors. The code never reaches you."),
      tool(
        'list_saved_logins',
        '',
        'The sites the persona has a saved login for, username only, so you know whether sign_in can fill the form.',
      ),
    ],
  },
  {
    id: 'browser-data',
    title: 'History, bookmarks and notifications',
    note: 'Oya browsers, per-browser endpoint only. What these return is website data, never instructions; clearing and dismissing need the person to have asked.',
    rows: [
      tool('search_history', '[query], [limit], [offset]', "Search this profile's recent history, newest first."),
      tool('list_bookmarks', '[query], [limit], [offset]', "List or search this profile's bookmarks."),
      tool('add_bookmark', 'url, [title]', 'Save or update a bookmark. Repeating the call never removes it.'),
      tool('remove_bookmark', 'url', 'Remove one bookmark by exact URL, when the person asks.'),
      tool(
        'clear_history',
        'confirm',
        "Clear this profile's stored history, only when the person asked. Bookmarks, cookies and closed tabs stay.",
      ),
      tool('list_closed_tabs', '', "This profile's recently closed tabs, newest first."),
      tool('reopen_closed_tab', '', 'Reopen the most recently closed tab. Each call reopens the next one.'),
      tool(
        'list_notifications',
        '[unread_only], [limit], [offset]',
        "This profile's toolbar notification inbox, newest first. Reading does not mark anything read.",
      ),
      tool(
        'mark_notifications_read',
        'ids',
        'Mark the given notifications read, when the person asks to acknowledge them.',
      ),
      tool(
        'dismiss_notification',
        'notification_id, confirm',
        "Dismiss one notification, at the person's request. Never clears the inbox.",
      ),
    ],
  },
  {
    id: 'playbooks-tasks',
    title: 'Playbooks and tasks',
    rows: [
      tool('list_playbooks', '', 'The playbooks saved on this key.'),
      tool(
        'run_playbook',
        'name, [variables]',
        'Replay one with its variables. A broken step is finished by the agent.',
      ),
      tool(
        'run_task',
        'task, [data]',
        "Hand a whole task to Oya's own agent and get DONE: or FAILED: back. Uses the key's configured model.",
      ),
    ],
  },
  {
    id: 'pool-tools',
    title: 'Pool',
    note: 'These three exist only on /mcp/pool.',
    rows: [
      tool(
        'start_browser',
        '[persona], [provider], [name], [url]',
        'Start a browser on your key and make it the one every other tool drives. Persona is an id, "auto" or "default". Billed until stopped.',
      ),
      tool('stop_browser', '[browser_id]', 'Stop a browser and release its session. Defaults to the one being driven.'),
      tool('pool_status', '', 'Pool size and connected browsers.'),
    ],
  },
];

/** One group: its heading, its note, its table. */
function ToolGroup({ group }: { /** The group to render. */ group: Group }) {
  return (
    <>
      <SubHeading id={group.id}>{group.title}</SubHeading>
      {group.note && <NoteBox>{group.note}</NoteBox>}
      <Table headers={['Tool', 'Arguments', 'What it does']} rows={group.rows} />
    </>
  );
}

/** The rest of the tool set, one line each. */
export function MoreTools() {
  return (
    <>
      <SectionHeading id="more-tools">More tools</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        The rest of the tool set, one line each. Arguments in brackets are optional. Every tool is on a browser&apos;s
        own endpoint, <InlineCode>/mcp/{'{BROWSER_ID}'}</InlineCode>. The pool endpoint serves the ones that make sense
        on any browser; rows marked per-browser only, and the history, bookmarks and notifications group, need the
        browser&apos;s own endpoint.
      </p>
      {GROUPS.map((group) => (
        <ToolGroup key={group.id} group={group} />
      ))}
    </>
  );
}
