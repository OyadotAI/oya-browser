/**
 * The project switcher in the dashboard header. The facade for `projects/`:
 * the session logic is in `session.ts`, the popover's actions in `actions.ts`
 * and `submit.ts`, and its panels in their own components.
 */
'use client';

import { ChevronDown, Folder } from 'lucide-react';
import { toggle } from './projects/actions';
import PickerPanel from './projects/picker-panel';
import { useProjectSwitcher } from './projects/use-project-switcher';
import { triggerLabel } from './projects/view';

/** The console credential, and how to replace it when the project changes. */
interface Props {
  /** The console's current credential. */
  apiKey: string;
  /** Replaces the credential; `project` is null when none is open. */
  setApiKey: (credential: string, project: string | null) => void;
}

/**
 * One switcher for every project, opened the same way whether you own it or it
 * was shared with you: a one-hour credential for your role there, renewed while
 * the tab stays open, held in sessionStorage.
 *
 * API key listings contain only digests and prefixes. The owner's explicit
 * copy action retrieves the encrypted project key; it stays in component
 * memory instead of being persisted as the console's credential.
 */
export default function ProjectSwitcher({ apiKey, setApiKey }: Props) {
  const { c, rootRef, triggerRef } = useProjectSwitcher(setApiKey);
  if (!c.session.token) return null;
  const open = c.ui.open;
  return (
    <div className="relative" ref={rootRef}>
      <button
        ref={triggerRef}
        aria-label="Switch project"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls="project-picker"
        onClick={() => toggle(c)}
        className={`flex h-8 items-center gap-2 rounded-lg border px-2.5 text-sm transition-colors ${open ? 'border-accent/30 bg-text/5 text-text' : 'border-border text-text-muted hover:bg-text/5 hover:text-text'}`}
      >
        <Folder className="h-3.5 w-3.5 shrink-0" />
        <span className="hidden max-w-[160px] truncate text-xs sm:inline">{triggerLabel(c, apiKey)}</span>
        <ChevronDown className={`h-3 w-3 text-text-dim transition-transform ${open ? 'rotate-180' : ''}`} />
      </button>
      {open && <PickerPanel c={c} />}
    </div>
  );
}
