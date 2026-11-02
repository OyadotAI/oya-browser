/**
 * The workbench's status bar: problems and saved state on the left, then the
 * cursor, indentation and language of the file showing, and the buttons that
 * show the explorer and the test panel (⌘B, ⌘J).
 */
'use client';

import { AlertCircle, CheckCircle2, PanelBottom, PanelLeft } from 'lucide-react';
import { EDITOR_TEXT, FILES, type FileId } from './editor-text';
import type { Workbench } from './use-workbench';

/** Props for the bar. */
interface Props {
  /** The layout. */
  w: Workbench;
  /** Why the definition does not parse, or null. */
  error: string | null;
  /** The definition has unsaved changes. */
  dirty: boolean;
  /** Opens the definition, to fix a problem. */
  onProblems: () => void;
}

/** A small status bar button. */
const BAR_BUTTON = 'flex items-center gap-1 rounded px-1.5 py-0.5 hover:bg-text/10';

/** The status bar. */
export default function StatusBar({ w, error, dirty, onProblems }: Props) {
  const file: FileId | null = w.active;
  return (
    <footer className="flex h-6 shrink-0 items-center gap-2 border-t border-border bg-bg-sunken px-2 text-[11px] text-text-muted">
      <button
        className={BAR_BUTTON}
        aria-label={EDITOR_TEXT.toggleSidebar}
        aria-pressed={w.sidebar}
        onClick={w.toggleSidebar}
      >
        <PanelLeft className="h-3 w-3" />
      </button>
      <button className={`${BAR_BUTTON} ${error ? 'text-red' : ''}`} onClick={onProblems}>
        {error ? <AlertCircle className="h-3 w-3" /> : <CheckCircle2 className="h-3 w-3" />}
        {error ? EDITOR_TEXT.problems(1) : EDITOR_TEXT.noProblems}
      </button>
      <span className={dirty ? 'text-yellow' : ''}>{dirty ? EDITOR_TEXT.unsaved : EDITOR_TEXT.saved}</span>
      <span className="ml-auto" />
      {file && file !== 'variables' && <span className="num">{EDITOR_TEXT.cursor(w.cursor.line, w.cursor.col)}</span>}
      {file && file !== 'variables' && <span>{EDITOR_TEXT.spaces}</span>}
      {file && (
        <span>{FILES[file].readOnly ? `${FILES[file].language} · ${EDITOR_TEXT.readOnly}` : FILES[file].language}</span>
      )}
      <button
        className={BAR_BUTTON}
        aria-label={EDITOR_TEXT.togglePanel}
        aria-pressed={w.panel}
        onClick={w.togglePanel}
      >
        <PanelBottom className="h-3 w-3" />
        {EDITOR_TEXT.testPanel}
      </button>
    </footer>
  );
}
