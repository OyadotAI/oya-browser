/**
 * Rename a saved playbook. Code calling play() with the old name breaks, so the
 * dialog says so.
 */
'use client';

import { useState } from 'react';
import Dialog from '@/components/ui/dialog';
import { isPlaybookName } from './format';

/** Props for the rename dialog. */
interface Props {
  /** The playbook's current name. */
  current: string;
  /** A rename is in flight. */
  busy: boolean;
  /** Closes without renaming. */
  onClose: () => void;
  /** Renames to the (trimmed) new name. */
  onRename: (name: string) => void;
}

/** A name field that accepts only a valid, different name. */
export default function RenameDialog({ current, busy, onClose, onRename }: Props) {
  const [name, setName] = useState(current);
  const trimmed = name.trim();
  const ok = isPlaybookName(trimmed) && trimmed !== current;
  return (
    <Dialog
      open
      onClose={onClose}
      title={`Rename ${current}`}
      description="Any healed draft moves with it. Code calling play() with the old name will fail."
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button className="btn-primary" onClick={() => onRename(trimmed)} disabled={!ok || busy}>
            {busy ? 'Renaming…' : 'Rename'}
          </button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (ok && !busy) onRename(trimmed);
        }}
      >
        <label className="label" htmlFor="pb-rename">
          Name
        </label>
        <input
          id="pb-rename"
          className="field font-mono"
          value={name}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => setName(e.target.value)}
        />
        <p className="mt-1 text-[12px] text-text-muted">1-64 letters, digits, _ or -.</p>
      </form>
    </Dialog>
  );
}
