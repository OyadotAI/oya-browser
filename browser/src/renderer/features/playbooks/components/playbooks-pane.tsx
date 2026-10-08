/** Saved playbooks and their replay inputs, kept inside the browser workspace. */
import { RendererConstants as C } from '../../../core/constants.ts';
import { useState } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Button } from '../../../ui/index.ts';
import type { PanelViewModel } from '../../../app/panel/panel-view-model.ts';
import type { PlaybooksViewModel } from '../view-models/playbooks-view-model.ts';
import './playbooks.css';

/** Shared library view props. */
interface Props {
  /** Library state and user intents. */
  vm: PlaybooksViewModel;
}
/** Search and select a saved playbook. */
function Library({ vm }: Props) {
  const state = useViewModel(vm);
  return (
    <>
      <label className="pb-field">
        Search playbooks
        <input type="search" value={state.query} onChange={(e) => vm.search(e.target.value)} />
      </label>
      <div className="pb-list" aria-label="Saved playbooks">
        {vm.matches.map((item) => (
          <button
            key={item.name}
            className="pb-row"
            aria-pressed={item.name === state.selected}
            onClick={() => vm.select(item.name)}
          >
            <strong>{item.name}</strong>
            <span>{item.steps} steps</span>
          </button>
        ))}
      </div>
      {!state.loading && !vm.matches.length && (
        <p className="quiet">
          {state.query ? 'No matching playbooks.' : 'No saved playbooks yet. Save a run from Ask or record a workflow.'}
        </p>
      )}
    </>
  );
}
/** Replay variables and the model-assisted repair preference. */
function Inputs({ vm }: Props) {
  const state = useViewModel(vm);
  return (
    <fieldset disabled={state.busy || vm.running} className="pb-inputs">
      {vm.selected?.variables.map((name) => (
        <label className="pb-field" key={name}>
          {name}
          <input autoComplete="off" value={state.values[name] || ''} onChange={(e) => vm.value(name, e.target.value)} />
        </label>
      ))}
      <label className="pb-check">
        <input type="checkbox" checked={state.autoHeal} onChange={(e) => vm.heal(e.target.checked)} />
        Repair changed pages using your model
      </label>
      <Button variant="primary" onClick={() => void vm.run()}>
        Run in this browser
      </Button>
    </fieldset>
  );
}
/** Rename and deletion use explicit inline confirmation, never destructive single clicks. */
function Manage({ vm }: Props) {
  const state = useViewModel(vm);
  const [name, setName] = useState('');
  const [deleting, setDeleting] = useState(false);
  return (
    <fieldset className="pb-inputs" disabled={state.busy || vm.running}>
      <details>
        <summary>Manage playbook</summary>
        <label className="pb-field">
          New name
          <input value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <div className="pb-actions">
          <Button disabled={!name.trim()} onClick={() => void vm.manage('rename', name.trim())}>
            Rename
          </Button>
          <Button onClick={() => void vm.manage('export')}>Export JSON</Button>
          <Button onClick={() => setDeleting(true)}>Delete…</Button>
        </div>
        {deleting && (
          <div role="alert">
            <p>Delete “{state.selected}”?</p>
            <Button
              onClick={() => {
                setDeleting(false);
                void vm.manage('delete');
              }}
            >
              Delete playbook
            </Button>
            <Button onClick={() => setDeleting(false)}>Cancel</Button>
          </div>
        )}
      </details>
    </fieldset>
  );
}
/** Details and replay form for the selected item. */
function Details({ vm }: Props) {
  useViewModel(vm);
  const item = vm.selected;
  if (!item) return null;
  return (
    <section className="pb-details" aria-label={item.name}>
      <h3>{item.name}</h3>
      <p className="quiet">Runs with this browser’s current profile and logins.</p>
      <Inputs vm={vm} />
      <details>
        <summary>View steps and code</summary>
        <pre>{item.code}</pre>
      </details>
      <Manage key={item.name} vm={vm} />
    </section>
  );
}
/** A background replay's status, final result, or request for assistance. */
function Run({ vm }: Props) {
  const { run } = useViewModel(vm);
  const [response, setResponse] = useState('');
  if (!run) return null;
  return (
    <section className="pb-run" aria-label="Playbook run" aria-live="polite">
      <strong>{run.status.replaceAll('_', ' ')}</strong>
      {run.error && <p role="alert">{run.error}</p>}
      {run.attention && (
        <>
          <p>{run.attention.message}</p>
          <p className="quiet">Use Take control in the toolbar to help on the page.</p>
          <label className="pb-field">
            Reply
            <input value={response} onChange={(e) => setResponse(e.target.value)} />
          </label>
          <Button
            disabled={!response.trim()}
            onClick={() => {
              void vm.respond(response);
              setResponse('');
            }}
          >
            Continue
          </Button>
        </>
      )}
      {run.result && <pre>{JSON.stringify(run.result, null, C.JSON_INDENT)}</pre>}
    </section>
  );
}
/** The library pane sits beside existing Ask, Record, and Routines tools. */
export function PlaybooksPane({ vm, panel }: Props & { /** Workspace selection. */ panel: PanelViewModel }) {
  const { pane } = useViewModel(panel);
  const state = useViewModel(vm);
  return (
    <div id="pane-playbooks" className={pane === 'playbooks' ? 'dev-pane active' : 'dev-pane'}>
      <div className="pb-body">
        <header className="pb-actions">
          <h2>Playbooks</h2>
          <Button disabled={state.loading} onClick={() => void vm.refresh()}>
            Refresh
          </Button>
          <Button disabled={state.busy || vm.running} onClick={() => void vm.manage('import')}>
            Import
          </Button>
        </header>
        {state.loading && <p role="status">Loading playbooks…</p>}
        {state.error && (
          <p role="alert" className="pb-error">
            {state.error}
          </p>
        )}
        {state.note && <p role="status">{state.note}</p>}
        <Library vm={vm} />
        <Details vm={vm} />
        <Run vm={vm} />
      </div>
    </div>
  );
}
