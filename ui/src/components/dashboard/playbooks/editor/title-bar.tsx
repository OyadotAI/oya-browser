/**
 * The workbench's title bar: where you are (Playbooks › name › file), and
 * Discard, Save as and Save (⌘S), with the close button at the end.
 */
'use client';

import { ChevronRight, X } from 'lucide-react';
import { EDITOR_TEXT, FILES, type FileId } from './editor-text';
import type { PlaybookEditorState } from './use-playbook-editor';

/** Props for the bar. */
interface Props {
  /** The playbook. */
  name: string;
  /** The file showing. */
  active: FileId | null;
  /** The editor. */
  e: PlaybookEditorState;
  /** Asks to close. */
  onClose: () => void;
}

/** Asks for a name and saves a copy under it. */
function saveAs(e: PlaybookEditorState) {
  const name = window.prompt(EDITOR_TEXT.saveAsPrompt);
  if (name?.trim()) e.saveAs(name.trim());
}

/** The title bar. */
export default function TitleBar({ name, active, e, onClose }: Props) {
  const ok = Boolean(e.parsed.definition) && !e.busy;
  return (
    <header className="flex h-11 shrink-0 items-center gap-2 border-b border-border bg-bg-sunken px-3">
      <h2 id="playbook-editor-title" className="flex min-w-0 items-center gap-1 text-[12px] text-text-muted">
        <span>Playbooks</span>
        <ChevronRight className="h-3 w-3" aria-hidden />
        <span className="truncate font-semibold text-text">{name}</span>
        {active && <ChevronRight className="h-3 w-3" aria-hidden />}
        {active && <span className="truncate font-mono">{FILES[active].name}</span>}
      </h2>
      <span className="ml-auto" />
      <button className="btn-ghost h-7 px-2 text-[12px]" onClick={e.discard} disabled={!e.dirty || e.busy}>
        {EDITOR_TEXT.discard}
      </button>
      <button className="btn-ghost h-7 px-2 text-[12px]" onClick={() => saveAs(e)} disabled={!ok}>
        {EDITOR_TEXT.saveAs}
      </button>
      <button className="btn-primary h-7 px-3 text-[12px]" onClick={e.save} disabled={!ok || !e.dirty} title="⌘S">
        {e.busy ? EDITOR_TEXT.saving : EDITOR_TEXT.save}
      </button>
      <button
        onClick={onClose}
        aria-label="Close"
        className="rounded-md p-1 text-text-muted hover:bg-text/5 hover:text-text"
      >
        <X className="h-4 w-4" />
      </button>
    </header>
  );
}
