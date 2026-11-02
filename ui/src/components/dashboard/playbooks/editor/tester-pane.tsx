/**
 * The tester: run the definition as it stands, saved or not, on a browser with a
 * sample value per variable, strictly unless asked to heal, and watch each step
 * play or break. Nothing it runs is saved.
 */
'use client';

import { CheckCircle2, XCircle } from 'lucide-react';
import { EDITOR_TEXT, RUN_STATUS } from './editor-text';
import { variableRows } from './definition';
import { useTester, type TesterState } from './use-tester';
import type { BrowserRow } from '../../types';
import type { Definition, StepEvent } from './editor-api';

/** Props for the pane. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** The definition to test, or null while it does not parse. */
  definition: Definition | null;
  /** Running browsers. */
  browsers: BrowserRow[];
}

/** The steps that have played, each with whether it played as recorded. */
function Timeline({ events }: { /** The steps so far. */ events: StepEvent[] }) {
  if (!events.length) return <p className="text-[12px] text-text-dim">{EDITOR_TEXT.noSteps}</p>;
  return (
    <ol className="space-y-1" aria-label="Steps played">
      {events.map((e) => (
        <li key={e.index} className="flex items-center gap-2 text-[13px]">
          {e.ok ? <CheckCircle2 className="h-3.5 w-3.5 text-accent" /> : <XCircle className="h-3.5 w-3.5 text-red" />}
          <span className="num text-text-dim">{e.index + 1}.</span>
          <span className="font-mono text-text-secondary">{e.action}</span>
          <span className={e.ok ? 'text-text-muted' : 'text-red'}>{e.ok ? EDITOR_TEXT.ok : EDITOR_TEXT.broke}</span>
        </li>
      ))}
    </ol>
  );
}

/** The tester's state. */
type T = TesterState;

/** The browser, a sample value per variable, and the healing choice. */
function Inputs({
  t,
  definition,
  browsers,
}: {
  /** The tester. */ t: T;
  /** The definition. */ definition: Definition | null;
  /** Browsers. */ browsers: BrowserRow[];
}) {
  return (
    <>
      <label className="label" htmlFor="test-browser">
        {EDITOR_TEXT.browser}
      </label>
      <select
        id="test-browser"
        className="field"
        value={t.browserId}
        onChange={(e) => t.patch({ browserId: e.target.value })}
      >
        {browsers.map((b) => (
          <option key={b.id} value={b.id}>
            {b.name || b.id}
          </option>
        ))}
      </select>
      {definition &&
        variableRows(definition).map((v) => (
          <input
            key={v.name}
            className="field font-mono"
            type={v.secret ? 'password' : 'text'}
            aria-label={EDITOR_TEXT.sample(v.name)}
            placeholder={`${v.name}${v.value && !v.secret ? ` (${v.value})` : ''}`}
            value={t.values[v.name] ?? ''}
            onChange={(e) => t.sample(v.name, e.target.value)}
          />
        ))}
      <label className="flex items-center gap-2 text-[13px] text-text">
        <input
          type="checkbox"
          className="accent-accent"
          checked={t.autoHeal}
          onChange={(e) => t.patch({ autoHeal: e.target.checked })}
        />
        {EDITOR_TEXT.heal}
      </label>
    </>
  );
}

/** The run's state and its timeline. */
function Result({ t }: { /** The tester. */ t: T }) {
  if (!t.run) return null;
  return (
    <div className="space-y-2 rounded-lg border border-border p-3" role="status">
      <p className="text-[13px] font-medium text-text">
        {RUN_STATUS[t.run.status] ?? t.run.status}
        {t.run.error ? `: ${t.run.error}` : ''}
      </p>
      <Timeline events={t.run.events ?? []} />
    </div>
  );
}

/** The browser, the sample values, healing, Run, and the run's progress. */
export default function TesterPane({ apiKey, definition, browsers }: Props) {
  const t = useTester(apiKey, definition, browsers[0]?.id ?? '');
  if (!browsers.length) return <p className="py-6 text-[13px] text-text-muted">{EDITOR_TEXT.noBrowsers}</p>;
  return (
    <div className="space-y-3">
      <Inputs t={t} definition={definition} browsers={browsers} />
      <button className="btn-primary" onClick={t.start} disabled={!definition || !t.browserId || t.running}>
        {t.running ? EDITOR_TEXT.runningLabel : EDITOR_TEXT.run}
      </button>
      {t.error && (
        <p role="alert" className="text-[13px] text-red">
          {t.error}
        </p>
      )}
      <Result t={t} />
    </div>
  );
}
