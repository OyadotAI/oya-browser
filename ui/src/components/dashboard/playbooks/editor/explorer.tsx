/**
 * The workbench's explorer: the playbook as a folder of three files, the
 * definition (with a dot while it has unsaved changes), its variables and the
 * Playwright code. Clicking one opens it as a tab; arrow keys move between them.
 */
'use client';

import { Braces, FileCode2, Lock, SlidersHorizontal } from 'lucide-react';
import type { KeyboardEvent } from 'react';
import { EDITOR_TEXT, FILES, type FileId } from './editor-text';

/** Props for the explorer. */
interface Props {
  /** The playbook, as the folder's name. */
  name: string;
  /** The file showing. */
  active: FileId | null;
  /** The definition has unsaved changes. */
  dirty: boolean;
  /** How many variables the steps use. */
  variables: number;
  /** Opens a file. */
  onOpen: (id: FileId) => void;
}

/** Each file's icon. */
const ICONS = { definition: Braces, variables: SlidersHorizontal, code: FileCode2 } as const;

/** The files in order. */
const IDS = Object.keys(FILES) as FileId[];

/** Moves focus to the next or previous file on ↓ / ↑. */
function moveFocus(e: KeyboardEvent<HTMLUListElement>) {
  const step = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
  if (!step) return;
  const items = [...e.currentTarget.querySelectorAll<HTMLElement>('[role="treeitem"]')];
  const at = items.indexOf(document.activeElement as HTMLElement);
  items[Math.min(items.length - 1, Math.max(0, at + step))]?.focus();
  e.preventDefault();
}

/** The explorer. */
export default function Explorer({ name, active, dirty, variables, onOpen }: Props) {
  return (
    <nav
      aria-label={EDITOR_TEXT.explorer}
      className="flex h-full w-[220px] shrink-0 flex-col border-r border-border bg-bg-sunken"
    >
      <div className="px-3 pb-1 pt-3 text-[10.5px] font-semibold uppercase tracking-[0.1em] text-text-muted">
        {EDITOR_TEXT.explorer}
      </div>
      <div className="truncate px-3 py-1 text-[12px] font-semibold text-text">{name}</div>
      <ul role="tree" aria-label={name} className="flex-1" onKeyDown={moveFocus}>
        {IDS.map((id) => {
          const Icon = ICONS[id];
          const selected = active === id;
          return (
            <li key={id} role="none">
              <button
                role="treeitem"
                aria-selected={selected}
                onClick={() => onOpen(id)}
                className={`flex h-7 w-full items-center gap-2 pl-6 pr-3 text-left font-mono text-[12px] ${selected ? 'bg-accent/10 text-text' : 'text-text-secondary hover:bg-text/5'}`}
              >
                <Icon className="h-3.5 w-3.5 shrink-0 text-text-muted" aria-hidden />
                <span className="flex-1 truncate">{FILES[id].name}</span>
                {id === 'definition' && dirty && (
                  <span className="text-[10px] text-yellow" aria-label={EDITOR_TEXT.unsaved}>
                    ●
                  </span>
                )}
                {id === 'variables' && <span className="num text-[11px] text-text-dim">{variables}</span>}
                {FILES[id].readOnly && <Lock className="h-3 w-3 text-text-dim" aria-label={EDITOR_TEXT.readOnly} />}
              </button>
            </li>
          );
        })}
      </ul>
      <p className="border-t border-border px-3 py-2 text-[11px] text-text-dim">{EDITOR_TEXT.explorerHint}</p>
    </nav>
  );
}
