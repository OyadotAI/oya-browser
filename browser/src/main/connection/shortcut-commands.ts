/** Read-only keyboard discovery from the registry used by the footer and native dispatch. */
import { SHORTCUTS, shortcutLabel, type Shortcut } from '../../shared/shortcuts.ts';
import type { TabHandler } from './tab-commands.ts';

/** Describes shell bindings without focusing controls, sending keys or changing browser state. */
export function keyboardShortcuts(platform: string) {
  return {
    platform,
    scope: 'browser-shell',
    guidance:
      'These are human-facing browser shell shortcuts. Prefer dedicated agent tools for actions; press_key targets webpage input and is not a shell-shortcut execution API.',
    shortcuts: SHORTCUTS.map((row) => describeShortcut(row, platform)),
  };
}
/** No active tab is needed to learn the connected browser's own bindings. */
export const SHORTCUT_COMMANDS: Record<string, TabHandler> = {
  list_keyboard_shortcuts: (runner, id) => runner.sendResult(id, true, keyboardShortcuts(process.platform)),
};

/** Named fields keep the agent independent of the registry's compact tuple representation. */
function describeShortcut(row: Shortcut, platform: string) {
  return {
    command: row[0],
    description: row[1],
    category: row[2],
    shortcut: shortcutLabel(row, platform),
    key: row[4],
    code: row[5] ?? null,
  };
}
