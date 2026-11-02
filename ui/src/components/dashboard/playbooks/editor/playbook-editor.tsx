/**
 * The playbook editor, laid out as VS Code is: an explorer of the playbook's
 * files (its definition as JSON, its variables, its Playwright code), tabs, a
 * CodeMirror editor that marks a JSON error where it is, a test panel below
 * (⌘J) that runs the edit before it is saved, and a status bar. Save replaces
 * the playbook (⌘S); Save as keeps the original; leaving with changes asks.
 */
'use client';

import { createPortal } from 'react-dom';
import CodeEditor from './code-editor';
import EditorTabs from './editor-tabs';
import Explorer from './explorer';
import StatusBar from './status-bar';
import TesterPane from './tester-pane';
import TitleBar from './title-bar';
import VariablesPane from './variables-pane';
import { variableRows } from './definition';
import { EDITOR_TEXT, type FileId } from './editor-text';
import { usePlaybookEditor, type PlaybookEditorState } from './use-playbook-editor';
import { useWorkbench, type Workbench } from './use-workbench';
import { useWorkbenchKeys } from './use-workbench-keys';
import { useWorkbenchFocus } from './use-workbench-focus';
import type { BrowserRow } from '../../types';
import type { PlaybookInfo } from '../types';

/** Props for the editor. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** The playbook edited. */
  playbook: PlaybookInfo;
  /** Running browsers, to test on. */
  browsers: BrowserRow[];
  /** Closes it. */
  onClose: () => void;
  /** Runs after a save, to refresh the list. */
  onSaved: () => void;
}

/** What the file views need. */
interface FileProps {
  /** The editor. */
  e: PlaybookEditorState;
  /** The layout. */
  w: Workbench;
  /** The editor's props. */
  props: Props;
}

/** The file showing: the definition, its variables, the code, or a hint to open one. */
function FileView({ file, e, w, props }: FileProps & { /** The file. */ file: FileId | null }) {
  if (file === 'definition')
    return (
      <CodeEditor
        language="json"
        label={EDITOR_TEXT.definitionLabel}
        value={e.text}
        onChange={e.setText}
        onCursor={w.onCursor}
        onSave={() => e.dirty && e.save()}
      />
    );
  if (file === 'code')
    return (
      <CodeEditor
        language="typescript"
        label={EDITOR_TEXT.codeHint}
        readOnly
        value={e.code ?? props.playbook.code}
        onCursor={w.onCursor}
      />
    );
  if (file === 'variables')
    return (
      <div className="h-full overflow-auto p-4">
        {e.parsed.definition ? (
          <VariablesPane definition={e.parsed.definition} onChange={e.setText} />
        ) : (
          <p className="text-[13px] text-red">{e.parsed.error}</p>
        )}
      </div>
    );
  return <p className="grid h-full place-items-center text-[13px] text-text-dim">{EDITOR_TEXT.empty}</p>;
}

/** The tabs, the file showing, its problem, and the test panel. */
function EditorArea({ e, w, props }: FileProps) {
  return (
    <main className="flex min-w-0 flex-1 flex-col">
      <EditorTabs open={w.open} active={w.active} dirty={e.dirty} onShow={w.show} onClose={w.close} />
      {e.saveError && (
        <p role="alert" className="border-b border-red/30 bg-red/10 px-3 py-1.5 text-[12px] text-red">
          {e.saveError}
        </p>
      )}
      <div className="min-h-0 flex-1">
        <FileView file={w.active} e={e} w={w} props={props} />
      </div>
      {e.parsed.error && w.active === 'definition' && (
        <p
          role="alert"
          id="definition-error"
          className="border-t border-border px-3 py-1 font-mono text-[11.5px] text-red"
        >
          {e.parsed.error}
        </p>
      )}
      {w.panel && (
        <section
          aria-label={EDITOR_TEXT.testPanel}
          className="h-[40%] min-h-[200px] shrink-0 overflow-auto border-t border-border bg-bg-sunken p-4"
        >
          <TesterPane apiKey={props.apiKey} definition={e.parsed.definition} browsers={props.browsers} />
        </section>
      )}
    </main>
  );
}

/** Loading, or why it could not load. */
function Waiting({ e }: { /** The editor. */ e: PlaybookEditorState }) {
  return (
    <p
      role={e.loadError ? 'alert' : 'status'}
      className={`grid flex-1 place-items-center text-[13px] ${e.loadError ? 'text-red' : 'text-text-muted'}`}
    >
      {e.loadError ?? EDITOR_TEXT.loading}
    </p>
  );
}

/** The workbench, over the whole console. */
export default function PlaybookEditor(props: Props) {
  const e = usePlaybookEditor(props.apiKey, props.playbook.name, props.onSaved);
  const w = useWorkbench();
  const close = () => (!e.dirty || window.confirm(EDITOR_TEXT.confirmClose)) && props.onClose();
  const save = () => e.dirty && e.save();
  useWorkbenchKeys({ save, togglePanel: w.togglePanel, toggleSidebar: w.toggleSidebar, close });
  const root = useWorkbenchFocus();
  const variables = e.parsed.definition ? variableRows(e.parsed.definition).length : 0;
  return createPortal(
    <div
      ref={root}
      role="dialog"
      aria-modal
      tabIndex={-1}
      aria-labelledby="playbook-editor-title"
      className="fixed inset-0 z-50 flex flex-col bg-bg text-text outline-none"
    >
      <TitleBar name={props.playbook.name} active={w.active} e={e} onClose={close} />
      {e.saved === null ? (
        <Waiting e={e} />
      ) : (
        <div className="flex min-h-0 flex-1">
          {w.sidebar && (
            <Explorer
              name={props.playbook.name}
              active={w.active}
              dirty={e.dirty}
              variables={variables}
              onOpen={w.show}
            />
          )}
          <EditorArea e={e} w={w} props={props} />
        </div>
      )}
      <StatusBar w={w} error={e.parsed.error} dirty={e.dirty} onProblems={() => w.show('definition')} />
    </div>,
    document.body,
  );
}
