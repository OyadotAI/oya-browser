/**
 * The workbench's layout state, as VS Code keeps it: which files are open as
 * tabs and which one shows, whether the explorer and the test panel are out, and
 * where the cursor is, for the status bar.
 */
import { useCallback, useState } from 'react';
import type { FileId } from './editor-text';

/** The open tabs and the one showing; closing it shows the last one left. */
function useTabs() {
  const [open, setOpen] = useState<FileId[]>(['definition']);
  const [active, setActive] = useState<FileId | null>('definition');
  const show = useCallback((id: FileId) => {
    setOpen((tabs) => (tabs.includes(id) ? tabs : [...tabs, id]));
    setActive(id);
  }, []);
  const close = (id: FileId) => closeTab({ open, active, setOpen, setActive }, id);
  return { open, active, show, close };
}

/** The tabs and their setters. */
interface Tabs {
  /** The open tabs. */
  open: FileId[];
  /** The one showing. */
  active: FileId | null;
  /** Sets the open tabs. */
  setOpen: (tabs: FileId[]) => void;
  /** Shows one. */
  setActive: (id: FileId | null) => void;
}

/** Closes tab `id`; when it was showing, the last one left shows. */
function closeTab(t: Tabs, id: FileId) {
  const rest = t.open.filter((tab) => tab !== id);
  t.setOpen(rest);
  if (t.active === id) t.setActive(rest.at(-1) ?? null);
}

/** The explorer (out at first) and the test panel (in at first), each toggled. */
function usePanels() {
  const [sidebar, setSidebar] = useState(true);
  const [panel, setPanel] = useState(false);
  const toggleSidebar = useCallback(() => setSidebar((v) => !v), []);
  const togglePanel = useCallback(() => setPanel((v) => !v), []);
  return { sidebar, panel, toggleSidebar, togglePanel };
}

/** The tabs, the panels and the cursor. */
export function useWorkbench() {
  const [cursor, setCursor] = useState({ line: 1, col: 1 });
  const onCursor = useCallback((line: number, col: number) => setCursor({ line, col }), []);
  return { ...useTabs(), ...usePanels(), cursor, onCursor };
}

/** What the workbench hook gives the views. */
export type Workbench = ReturnType<typeof useWorkbench>;
