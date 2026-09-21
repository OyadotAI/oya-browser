/**
 * Before a run: which profile and browser, the variable values, and auto-heal.
 */
'use client';

import type { Persona } from '../types';
import type { RunDialogState } from './use-run';
import type { PlaybookInfo } from './types';

/** Props for the form. */
interface Props {
  /** The dialog's state. */
  run: RunDialogState;
  /** The playbook to run. */
  playbook: PlaybookInfo;
  /** Profiles to filter browsers by. */
  personas: Persona[];
}

/** The running browsers on the chosen profile, or why there are none. */
function BrowserChoice({ run }: Pick<Props, 'run'>) {
  const { matching, browserId, setBrowserId, persona } = run.target;
  if (matching.length)
    return (
      <select
        id="pb-browser"
        className="field"
        value={browserId}
        onChange={(e) => setBrowserId(e.target.value)}
        disabled={run.runner.starting}
      >
        {matching.map((b) => (
          <option key={b.id} value={b.id}>
            {b.name} · {b.id}
          </option>
        ))}
      </select>
    );
  return persona ? (
    <p className="text-sm text-yellow">
      No browser is running this profile. Start one and the replay follows it there, its logins, fingerprint and exit IP
      come with it.
    </p>
  ) : (
    <p className="text-sm text-text-muted">No browser is running. Start one from the Browsers tab.</p>
  );
}

/** One field per variable. */
function VariableFields({ run, playbook }: Omit<Props, 'personas'>) {
  const { values, setValues } = run.form;
  return playbook.variables.map((v) => (
    <div key={v}>
      <label className="label" htmlFor={`pb-var-${v}`}>
        {v}
      </label>
      <input
        id={`pb-var-${v}`}
        className="field font-mono"
        value={values[v] || ''}
        autoComplete="off"
        onChange={(e) => setValues((cur) => ({ ...cur, [v]: e.target.value }))}
      />
    </div>
  ));
}

/** The whole form. */
export default function RunForm({ run, playbook, personas }: Props) {
  const { starting } = run.runner;
  return (
    <div className="flex flex-col gap-4">
      <div>
        <label className="label" htmlFor="pb-persona">
          Profile
        </label>
        <select
          id="pb-persona"
          className="field"
          value={run.target.persona}
          onChange={(e) => run.target.setPersona(e.target.value)}
          disabled={starting}
        >
          <option value="">Any profile</option>
          {personas.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
              {p.isDefault ? ' · default' : ''}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label className="label" htmlFor="pb-browser">
          Browser
        </label>
        <BrowserChoice run={run} />
      </div>
      <VariableFields run={run} playbook={playbook} />
      <label className="flex items-start gap-2 text-sm text-text-secondary">
        <input
          type="checkbox"
          className="mt-0.5 h-4 w-4"
          checked={run.form.autoHeal}
          onChange={(e) => run.form.setAutoHeal(e.target.checked)}
        />
        <span>Auto-heal: if the page changed, the agent finishes the run and saves its fix as a draft to review.</span>
      </label>
    </div>
  );
}
