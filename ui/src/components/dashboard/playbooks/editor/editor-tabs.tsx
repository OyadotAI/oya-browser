/**
 * The workbench's tab strip, as VS Code draws it: one tab per open file, the
 * showing one lit along its top edge, a dot on the definition while it has
 * unsaved changes, and a close button that shows on hover or focus.
 */
'use client';

import { X } from 'lucide-react';
import { EDITOR_TEXT, FILES, type FileId } from './editor-text';

/** Props for the strip. */
interface Props {
  /** The open files, in order. */
  open: FileId[];
  /** The one showing. */
  active: FileId | null;
  /** The definition has unsaved changes. */
  dirty: boolean;
  /** Shows a tab. */
  onShow: (id: FileId) => void;
  /** Closes a tab. */
  onClose: (id: FileId) => void;
}

/** The tab strip. */
export default function EditorTabs({ open, active, dirty, onShow, onClose }: Props) {
  return (
    <div role="tablist" className="flex h-9 shrink-0 overflow-x-auto border-b border-border bg-bg-sunken">
      {open.map((id) => {
        const selected = id === active;
        const name = FILES[id].name;
        return (
          <div
            key={id}
            className={`group flex max-w-[200px] shrink-0 items-center border-r border-border ${selected ? 'border-t-2 border-t-accent bg-bg text-text' : 'border-t-2 border-t-transparent text-text-muted'}`}
          >
            <button
              role="tab"
              aria-selected={selected}
              onClick={() => onShow(id)}
              className="truncate py-1 pl-3 pr-1 font-mono text-[12px]"
            >
              {name}
            </button>
            {id === 'definition' && dirty && <span className="px-1 text-[10px] text-yellow group-hover:hidden">●</span>}
            <button
              aria-label={EDITOR_TEXT.closeTab(name)}
              onClick={() => onClose(id)}
              className={`mr-1 rounded p-0.5 hover:bg-text/10 focus:opacity-100 ${selected ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'} ${id === 'definition' && dirty ? 'hidden group-hover:block' : ''}`}
            >
              <X className="h-3 w-3" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
