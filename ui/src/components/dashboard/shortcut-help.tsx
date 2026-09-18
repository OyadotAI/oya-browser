/**
 * The keyboard-shortcut cheat sheet, opened with "?".
 */
'use client';

import Dialog from '@/components/ui/dialog';
import Kbd from '@/components/ui/kbd';
import { keyCaps, type Shortcut } from '@/lib/shortcuts';

/** The shortcut groups, in the order they are shown. */
const GROUPS = ['Fleet', 'Browser', 'Navigate'] as const;

/** Whether it is open, and what to list. */
interface Props {
  /** Shows the dialog. */
  open: boolean;
  /** Closes it. */
  onClose: () => void;
  /** Every shortcut the page registers. */
  shortcuts: Shortcut[];
}

/** Every shortcut, grouped. */
export default function ShortcutHelp({ open, onClose, shortcuts }: Props) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Keyboard shortcuts"
      size="md"
      description="Press ? anywhere to see this."
    >
      <div className="grid gap-5 sm:grid-cols-3">
        {GROUPS.map((g) => (
          <div key={g}>
            <h3 className="label">{g}</h3>
            <ul className="space-y-1.5">
              {shortcuts
                .filter((s) => s.group === g)
                .map((s) => (
                  <li key={s.keys} className="flex items-center justify-between gap-3 text-[13px] text-text-secondary">
                    <span>{s.label}</span>
                    <span className="flex gap-1">
                      {keyCaps(s.keys).map((k, i) => (
                        <Kbd key={i}>{k}</Kbd>
                      ))}
                    </span>
                  </li>
                ))}
            </ul>
          </div>
        ))}
      </div>
      <p className="mt-5 text-[12px] text-text-muted">
        Inside the live view, keys go to the browser until you press <Kbd>Esc</Kbd>.
      </p>
    </Dialog>
  );
}
