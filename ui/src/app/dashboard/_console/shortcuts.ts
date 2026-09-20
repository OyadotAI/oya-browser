/**
 * The console's keyboard shortcuts as a command table: each binding's keys,
 * label and group, and what it does given the console's current state.
 */
import type { RefObject } from 'react';
import type { Shortcut } from '@/lib/shortcuts';
import type { Patch } from '@/lib/hooks/use-patch-state';
import type { ConsoleView, MainTab } from './view';

/** What a shortcut can read and do. */
export interface ShortcutContext {
  /** The browser open in the panel. */
  selected: string | null;
  /** The ticked browsers. */
  checked: Set<string>;
  /** Whether a dialog or drawer is open (it handles Escape itself). */
  overlayOpen: boolean;
  /** Changes the view. */
  patch: Patch<ConsoleView>;
  /** The fleet filter input. */
  filterRef: RefObject<HTMLInputElement | null>;
  /** The panel's URL bar. */
  urlRef: RefObject<HTMLInputElement | null>;
  /** Asks to stop these browsers. */
  requestStop: (ids: string[]) => void;
}

/** A binding without its handler, plus what it runs. */
type Binding = Omit<Shortcut, 'handler'> & {
  /** Runs the shortcut. */
  run: (c: ShortcutContext) => void;
};

/** Selects the next or previous browser row, in the table's current order. */
function moveSelection(c: ShortcutContext, dir: 1 | -1) {
  const rows = [...document.querySelectorAll<HTMLElement>('tbody [data-id]')].map((el) => el.dataset.id!);
  if (!rows.length) return;
  const i = c.selected ? rows.indexOf(c.selected) : -1;
  c.patch({ selected: rows[Math.min(rows.length - 1, Math.max(0, i + dir))] });
}

/** Escape closes the panel, else clears the ticks; an open dialog handles it instead. */
function closeOrClear(c: ShortcutContext) {
  if (c.overlayOpen) return; // the dialog handles it
  if (c.selected) c.patch({ selected: null });
  else if (c.checked.size) c.patch({ checked: new Set() });
}

/** Clicks the selected browser's panel button whose text matches. */
function clickPanelButton(c: ShortcutContext, text: RegExp) {
  if (!c.selected) return;
  [...document.querySelectorAll<HTMLButtonElement>('aside button')]
    .find((b) => text.test(b.textContent || ''))
    ?.click();
}

/** A tab switch that works from anywhere. */
const tab = (keys: string, label: string, key: MainTab): Binding => ({
  keys,
  label,
  group: 'Navigate',
  global: true,
  run: (c) => c.patch({ tab: key }),
});

/** Every binding, in help-sheet order. */
const BINDINGS: Binding[] = [
  tab('mod+1', 'Browsers', 'browsers'),
  tab('mod+2', 'Personas', 'personas'),
  tab('mod+3', 'Control', 'control'),
  tab('mod+4', 'Playbooks', 'playbooks'),
  { keys: '?', label: 'This help', group: 'Navigate', run: (c) => c.patch({ showHelp: true }) },
  { keys: 'n', label: 'Start a browser', group: 'Fleet', run: (c) => c.patch({ showStart: true }) },
  {
    keys: '/',
    label: 'Filter the fleet',
    group: 'Fleet',
    run: (c) => (c.patch({ tab: 'browsers' }), c.filterRef.current?.focus()),
  },
  { keys: 'down', label: 'Next browser', group: 'Fleet', run: (c) => moveSelection(c, 1) },
  { keys: 'up', label: 'Previous browser', group: 'Fleet', run: (c) => moveSelection(c, -1) },
  { keys: 'j', label: 'Next browser', group: 'Fleet', run: (c) => moveSelection(c, 1) },
  { keys: 'k', label: 'Previous browser', group: 'Fleet', run: (c) => moveSelection(c, -1) },
  { keys: 'escape', label: 'Close panel / clear', group: 'Fleet', global: true, run: closeOrClear },
  {
    keys: 'x',
    label: 'Stop selected',
    group: 'Browser',
    run: (c) => c.requestStop(c.checked.size ? [...c.checked] : c.selected ? [c.selected] : []),
  },
  { keys: 'l', label: 'Focus the URL bar', group: 'Browser', run: (c) => c.urlRef.current?.focus() },
  {
    keys: 'r',
    label: 'Reload',
    group: 'Browser',
    run: (c) => c.selected && document.querySelector<HTMLButtonElement>('[title^="Reload"]')?.click(),
  },
  { keys: 's', label: 'Screenshot', group: 'Browser', run: (c) => clickPanelButton(c, /Screenshot/) },
];

/** The console's shortcuts, bound to its current state. */
export function buildShortcuts(c: ShortcutContext): Shortcut[] {
  return BINDINGS.map(({ run, ...binding }) => ({ ...binding, handler: () => run(c) }));
}
