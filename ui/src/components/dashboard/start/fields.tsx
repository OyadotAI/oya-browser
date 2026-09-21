/**
 * The start-browser dialog's fields: which profile, a name and how many, and
 * (when the key has more than one ready provider) where.
 */
'use client';

import type { Persona } from '../types';
import { providerLabel } from '../types';
import { MAX_BROWSERS_PER_START } from './constants';
import { clampCount } from './start-run';
import type { ProviderOption } from './use-start-browser';

/** The profile picker's value and choices. */
interface ProfileProps {
  /** Chosen persona id. */
  value: string;
  /** Picks one. */
  onChange: (value: string) => void;
  /** Every persona of the key. */
  personas: Persona[];
}

/** Default, auto, or a named persona with how many of its slots are in use. */
export function ProfileField({ value, onChange, personas }: ProfileProps) {
  return (
    <div>
      <label className="label" htmlFor="sb-persona">
        Profile
      </label>
      <select id="sb-persona" className="field" value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="default">Default profile</option>
        <option value="auto">Auto, least recently used under its cap</option>
        {personas
          .filter((p) => !p.isDefault)
          .map((p) => (
            <option key={p.id} value={p.id}>
              {p.name} · {p.activeBrowsers}/{p.maxConcurrent === null ? '∞' : p.maxConcurrent} running
            </option>
          ))}
      </select>
    </div>
  );
}

/** The name and count fields' values. */
interface NameCountProps {
  /** Optional name. */
  name: string;
  /** Sets it. */
  onName: (name: string) => void;
  /** How many to start. */
  count: number;
  /** Sets it. */
  onCount: (count: number) => void;
}

/** An optional name, and how many to start. */
export function NameCountFields({ name, onName, count, onCount }: NameCountProps) {
  return (
    <div className="grid grid-cols-[1fr_88px] gap-3">
      <div>
        <label className="label" htmlFor="sb-name">
          Name <span className="normal-case tracking-normal text-text-dim">(optional)</span>
        </label>
        <input
          id="sb-name"
          className="field"
          value={name}
          onChange={(e) => onName(e.target.value)}
          placeholder="e.g. checkout-worker"
        />
      </div>
      <div>
        <label className="label" htmlFor="sb-count">
          How many
        </label>
        <input
          id="sb-count"
          type="number"
          min={1}
          max={MAX_BROWSERS_PER_START}
          className="field num"
          value={count}
          onChange={(e) => onCount(clampCount(e.target.value))}
        />
      </div>
    </div>
  );
}

/** The provider picker's value and choices. */
interface ProviderProps {
  /** Chosen provider; empty for the key default. */
  value: string;
  /** Picks one. */
  onChange: (value: string) => void;
  /** The key default. */
  defaultProvider: string;
  /** Ready providers. */
  configured: ProviderOption[];
}

/** A per-browser provider override. */
export function ProviderField({ value, onChange, defaultProvider, configured }: ProviderProps) {
  return (
    <div>
      <label className="label" htmlFor="sb-provider">
        Provider <span className="normal-case tracking-normal text-text-dim">(this browser only)</span>
      </label>
      <select id="sb-provider" className="field" value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="">Key default, {providerLabel(defaultProvider)}</option>
        {configured.map((p) => (
          <option key={p.id} value={p.id}>
            {p.label}
          </option>
        ))}
      </select>
    </div>
  );
}
