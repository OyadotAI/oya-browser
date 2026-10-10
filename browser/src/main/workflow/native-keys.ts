/** Workflow key chords use native key definitions and browser-owned shortcut provenance. */
import type { PageDriver, DriverTab } from '../actions/driver.ts';
import { KEY_DEFS, Modifier } from '../input/constants.ts';
/** Modifier spellings accepted by exported workflows. */
const MODIFIERS: Record<string, number> = {
  Control: Modifier.CTRL,
  Ctrl: Modifier.CTRL,
  Meta: Modifier.META,
  Shift: Modifier.SHIFT,
  Alt: Modifier.ALT,
};
/** Keys that alter native browser UI have no workflow meaning. */
const BLOCKED = new Set(['F11', 'F12', 'F5', 'BrowserBack', 'BrowserForward', 'ZoomIn', 'ZoomOut']);
/** Parsed physical key and held modifier flags. */
export interface WorkflowKey {
  /** Logical key. */ key: string;
  /** Native input modifier bits. */ modifiers: number;
}
/** Reject malformed keys before any draft action; literal plus is a valid printable key. */
export function workflowKey(value: string): WorkflowKey {
  const parts = value === '+' ? ['+'] : value.split('+');
  const key = parts.pop() || '';
  if (BLOCKED.has(key) || (!Object.hasOwn(KEY_DEFS, key) && [...key].length !== 1))
    throw Error('Unsupported native workflow key: ' + value);
  return { key, modifiers: parts.reduce((bits, part) => bits | modifier(part), 0) };
}
/** Platform-dependent shortcut semantics stay centralized in one parser. */
function modifier(name: string): number {
  if (name === 'ControlOrMeta') return process.platform === 'darwin' ? Modifier.META : Modifier.CTRL;
  if (!Object.hasOwn(MODIFIERS, name)) throw Error('Unsupported native workflow modifier: ' + name);
  return MODIFIERS[name];
}
/** One acknowledged native key press, including its modifier flags. */
export async function pressWorkflowKey(driver: PageDriver, tab: DriverTab, value: string): Promise<void> {
  const { key, modifiers } = workflowKey(value);
  if (editingKey(tab, key, modifiers)) return;
  await driver.keyboard.press(tab.view, key, modifiers);
}

/** Electron native editing methods supply platform accelerator defaults absent from sendInputEvent on macOS. */
function editingKey(tab: DriverTab, key: string, modifiers: number): boolean {
  const command = process.platform === 'darwin' ? Modifier.META : Modifier.CTRL;
  if (modifiers !== command && modifiers !== (command | Modifier.SHIFT)) return false;
  const name = key.toLowerCase(),
    shifted = Boolean(modifiers & Modifier.SHIFT);
  const operations: Record<string, () => void> = { a: () => tab.view.webContents.selectAll() };
  if (shifted || !Object.hasOwn(operations, name)) return false;
  operations[name]();
  return true;
}
