/**
 * Limits and retention: the project's numeric settings, rate cards and
 * managed policy, saved together. Rates and policy are edited as JSON.
 */
import type { FormEvent } from 'react';
import { DURABLE_BUTTON, DURABLE_FIELD, LIMIT_FIELDS, LLM_PROVIDERS } from './constants';
import { settingsBody, type Durable } from './use-durable';

/** Label class for each setting. */
const LABEL = 'block space-y-1 text-xs text-text-muted';

/** The settings form; bad JSON is reported instead of sent. */
export default function DurableSettings({ d }: { /** Project operations state and actions. */ d: Durable }) {
  const { rates, policy } = d.drafts;
  const save = (e: FormEvent) => {
    e.preventDefault();
    try {
      void d.act('/project', 'PATCH', settingsBody(d.drafts));
    } catch {
      d.setError('Rate cards and policy must be valid JSON');
    }
  };
  return (
    <form className="space-y-4" onSubmit={save}>
      <h3 className="text-sm font-medium">Limits and retention</h3>
      <LimitFields d={d} />
      <label className={LABEL}>
        <span>Rate cards · USD per browser hour</span>
        <textarea
          className={`${DURABLE_FIELD} h-24 font-mono`}
          value={rates}
          onChange={(e) => d.editDrafts({ rates: e.target.value })}
        />
      </label>
      <label className={LABEL}>
        <span>Managed policy · allowedHosts, humanHosts, region, redactRecording</span>
        <textarea
          className={`${DURABLE_FIELD} h-28 font-mono`}
          value={policy}
          onChange={(e) => d.editDrafts({ policy: e.target.value })}
        />
      </label>
      <ModelFields d={d} />
      <p className="text-xs text-text-dim">
        Hard budgets require managed browsers and a configured rate. Costs are estimates; provider billing may lag.
      </p>
      <button disabled={d.busy} className={DURABLE_BUTTON}>
        Save settings
      </button>
    </form>
  );
}

/** Concurrency, budget and the two retention periods; blank means no override. */
function LimitFields({ d }: { /** Project operations state and actions. */ d: Durable }) {
  const settings = d.drafts.settings!;
  return (
    <div className="grid grid-cols-2 gap-3">
      {(Object.keys(LIMIT_FIELDS) as (keyof typeof LIMIT_FIELDS)[]).map((k) => (
        <label className="space-y-1 text-xs text-text-muted" key={k}>
          <span>{LIMIT_FIELDS[k]}</span>
          <input
            className={DURABLE_FIELD}
            type="number"
            min="0.01"
            step={k === 'budgetUsd' ? '0.01' : '1'}
            value={settings[k] ?? ''}
            placeholder="No project override"
            onChange={(e) =>
              d.editDrafts({ settings: { ...settings, [k]: e.target.value === '' ? null : Number(e.target.value) } })
            }
          />
        </label>
      ))}
    </div>
  );
}

/** Every provider's name; a project with no model policy allows them all. */
const ALL_PROVIDERS = Object.keys(LLM_PROVIDERS);

/** Which model providers may see page text and screenshots: a checkbox each, all checked by default. */
function ModelFields({ d }: { /** Project operations state and actions. */ d: Durable }) {
  const settings = d.drafts.settings!;
  const allowed = settings.llm?.allow ?? ALL_PROVIDERS;
  const toggle = (name: string, on: boolean) =>
    d.editDrafts({
      settings: { ...settings, llm: { allow: ALL_PROVIDERS.filter((p) => (p === name ? on : allowed.includes(p))) } },
    });
  return (
    <fieldset className="space-y-1 text-xs text-text-muted">
      <legend>Model providers that may see page text and screenshots</legend>
      <div className="flex gap-4">
        {(Object.keys(LLM_PROVIDERS) as (keyof typeof LLM_PROVIDERS)[]).map((name) => (
          <label className="flex items-center gap-1" key={name}>
            <input type="checkbox" checked={allowed.includes(name)} onChange={(e) => toggle(name, e.target.checked)} />
            {LLM_PROVIDERS[name]}
          </label>
        ))}
      </div>
      <p className="text-text-dim">
        An agent run on a provider left unchecked is refused before any page content is sent.
      </p>
    </fieldset>
  );
}
