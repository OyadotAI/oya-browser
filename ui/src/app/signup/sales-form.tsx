/**
 * The questions before a sales call: who is asking, what their agents need
 * to reach, where it would run, and how much.
 */
'use client';

import { FormError, SubmitButton, TextField } from '@/components/auth/fields';
import type { Patch } from '@/lib/hooks/use-patch-state';
import { DEPLOYS, PORTALS, REQUIREMENTS, ROLES, VOLUMES, toggled, type SalesDetails } from './sales';
import type { Sales } from './use-sales';

/** A chip's look, picked or not. */
const CHIP = {
  on: 'border-accent bg-accent/10 text-text',
  off: 'border-border text-text-muted hover:text-text',
};

/** A label above a control. */
const LABEL = 'mb-2 block text-sm font-medium text-text-muted';

/** A select's look. */
const SELECT =
  'block w-full rounded-lg border border-border bg-bg-input px-3 py-2.5 text-sm text-text focus:border-border-focus focus:outline-none';

/** Chips' props. */
interface ChipsProps {
  /** The legend above the chips. */
  legend: string;
  /** Every choice. */
  options: string[];
  /** The picked ones. */
  value: string[];
  /** Receives the new picks. */
  onChange: (value: string[]) => void;
}

/** A row of toggle chips for a multiple choice. */
function Chips({ legend, options, value, onChange }: ChipsProps) {
  return (
    <fieldset>
      <legend className={LABEL}>{legend}</legend>
      <div className="flex flex-wrap gap-2">
        {options.map((o) => (
          <button
            key={o}
            type="button"
            aria-pressed={value.includes(o)}
            onClick={() => onChange(toggled(value, o))}
            className={`rounded-full border px-3.5 py-1.5 text-sm ${value.includes(o) ? CHIP.on : CHIP.off}`}
          >
            {o}
          </button>
        ))}
      </div>
    </fieldset>
  );
}

/** SelectField's props. */
interface SelectProps {
  /** The select's id, which the label points at. */
  id: string;
  /** The label. */
  label: string;
  /** Every choice. */
  options: string[];
  /** The picked one. */
  value: string;
  /** Receives the new pick. */
  onChange: (value: string) => void;
}

/** A labeled select. */
function SelectField({ id, label, options, value, onChange }: SelectProps) {
  return (
    <div>
      <label htmlFor={id} className={LABEL}>
        {label}
      </label>
      <select id={id} value={value} onChange={(e) => onChange(e.target.value)} className={SELECT}>
        {options.map((o) => (
          <option key={o}>{o}</option>
        ))}
      </select>
    </div>
  );
}

/** DeployPicker's props. */
interface DeployProps {
  /** The picked DEPLOYS id. */
  value: string;
  /** Receives the new pick. */
  onChange: (id: string) => void;
}

/** Where it would run: one of three cards. */
function DeployPicker({ value, onChange }: DeployProps) {
  return (
    <fieldset>
      <legend className={LABEL}>Where would it run?</legend>
      <div role="radiogroup" className="grid grid-cols-3 gap-2">
        {DEPLOYS.map((d) => (
          <button
            key={d.id}
            type="button"
            role="radio"
            aria-checked={value === d.id}
            onClick={() => onChange(d.id)}
            className={`rounded-lg border px-3 py-2 text-left ${value === d.id ? CHIP.on : CHIP.off}`}
          >
            <span className="block text-sm font-semibold">{d.label}</span>
            <span className="block text-xs text-text-dim">{d.sub}</span>
          </button>
        ))}
      </div>
    </fieldset>
  );
}

/** Setters for each field, so the form reads as a list of fields. */
function setters(patch: Patch<SalesDetails>) {
  const set = (key: keyof SalesDetails) => (value: string | string[]) => patch({ [key]: value });
  return { set, text: (key: keyof SalesDetails) => set(key) as (v: string) => void };
}

/** Name, work email, company and role. */
function WhoFields({ details: d, patch }: Pick<Sales, 'details' | 'patch'>) {
  const { text } = setters(patch);
  return (
    <>
      <div className="grid grid-cols-2 gap-3">
        <TextField
          id="firstName"
          label="First name"
          autoComplete="given-name"
          placeholder=""
          value={d.firstName}
          onChange={text('firstName')}
        />
        <TextField
          id="lastName"
          label="Last name"
          autoComplete="family-name"
          placeholder=""
          value={d.lastName}
          onChange={text('lastName')}
        />
      </div>
      <TextField
        id="workEmail"
        label="Work email"
        type="email"
        autoComplete="email"
        placeholder="you@company.com"
        value={d.email}
        onChange={text('email')}
      />
      <div className="grid grid-cols-2 gap-3">
        <TextField
          id="company"
          label="Company"
          autoComplete="organization"
          placeholder=""
          value={d.company}
          onChange={text('company')}
        />
        <SelectField id="role" label="Your role" options={ROLES} value={d.role} onChange={text('role')} />
      </div>
    </>
  );
}

/** SalesForm's props. */
interface FormProps {
  /** The enterprise path's state. */
  sales: Sales;
}

/** The whole form; submitting moves on to the calendar. */
export function SalesForm({ sales }: FormProps) {
  const { details: d, patch, error, submit } = sales;
  const { set, text } = setters(patch);
  return (
    <form onSubmit={submit} noValidate className="space-y-5">
      <WhoFields details={d} patch={patch} />
      <Chips
        legend="What do your agents need to reach?"
        options={PORTALS}
        value={d.portals}
        onChange={set('portals')}
      />
      <DeployPicker value={d.deploy} onChange={text('deploy')} />
      <SelectField
        id="volume"
        label="Expected runs / month"
        options={VOLUMES}
        value={d.volume}
        onChange={text('volume')}
      />
      <Chips legend="Requirements" options={REQUIREMENTS} value={d.requirements} onChange={set('requirements')} />
      <div>
        <label htmlFor="notes" className={LABEL}>
          Anything we should know? <span className="text-text-dim">(optional)</span>
        </label>
        <textarea
          id="notes"
          rows={2}
          value={d.notes}
          onChange={(e) => patch({ notes: e.target.value })}
          placeholder="e.g. eligibility checks on three payer portals, nightly"
          className={SELECT}
        />
      </div>
      <FormError error={error} />
      <SubmitButton busy={false} label="Continue to pick a time" busyLabel="" />
    </form>
  );
}
