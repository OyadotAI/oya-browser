/**
 * The new-profile form's field groups: the device (platform, then the
 * timezones and locales that platform really reports) and the limits (cap and
 * proxy geo).
 */
'use client';

import { platformLabel } from '../types';
import { GEO_MAX_LENGTH } from './constants';
import type { Options, PersonaDraft } from './model';
import type { SetDraft } from './use-persona-form';

/** What each field group reads and writes. */
interface GroupProps {
  /** The form as typed. */
  draft: PersonaDraft;
  /** Changes some of its fields. */
  set: SetDraft;
}

/** A select of plain string choices behind an `auto` option. */
function Choice(props: {
  /** What is being chosen; also the select's accessible name. */
  label: string;
  /** The current choice, or `auto`. */
  value: string;
  /** What may be chosen besides auto. */
  choices: string[];
  /** Locked until it makes sense to choose. */
  disabled?: boolean;
  /** Hears the new choice. */
  onPick: (v: string) => void;
}) {
  return (
    <select
      className="field"
      value={props.value}
      onChange={(e) => props.onPick(e.target.value)}
      aria-label={props.label}
      disabled={props.disabled}
    >
      <option value="auto">{props.label}: auto</option>
      {props.choices.map((z) => (
        <option key={z} value={z}>
          {z}
        </option>
      ))}
    </select>
  );
}

/** Platform, timezone and locale; the last two unlock once a platform is chosen. */
export function DeviceFields({
  draft,
  set,
  opts,
}: GroupProps & { /** What the server offers; null until it answers. */ opts: Options | null }) {
  const auto = draft.platform === 'auto';
  return (
    <fieldset>
      <legend className="label">Device</legend>
      <div className="grid grid-cols-3 gap-2">
        <select
          className="field"
          value={draft.platform}
          onChange={(e) => set({ platform: e.target.value })}
          aria-label="Platform"
        >
          <option value="auto">Platform: auto</option>
          {opts?.platforms.map((p) => (
            <option key={p} value={p}>
              {platformLabel(p)}
            </option>
          ))}
        </select>
        <Choice
          label="Timezone"
          value={draft.timezone}
          choices={auto ? [] : opts?.timezones[draft.platform] || []}
          disabled={auto}
          onPick={(timezone) => set({ timezone })}
        />
        <Choice
          label="Locale"
          value={draft.locale}
          choices={auto ? [] : opts?.locales[draft.platform] || []}
          disabled={auto}
          onPick={(locale) => set({ locale })}
        />
      </div>
      <p className="mt-1.5 text-[11.5px] text-text-muted">
        Auto picks a coherent set. Choose a platform to pick its timezone and locale — only combinations a real machine
        reports are offered.
      </p>
    </fieldset>
  );
}

/** How many browsers may run at once, and where its proxy should be. */
export function LimitFields({ draft, set }: GroupProps) {
  return (
    <div className="grid grid-cols-2 gap-3">
      <div>
        <label className="label" htmlFor="pf-cap">
          Concurrent browsers
        </label>
        <input
          id="pf-cap"
          className="field num"
          type="number"
          min={1}
          value={draft.cap}
          onChange={(e) => set({ cap: e.target.value })}
          placeholder="∞"
        />
        <p className="mt-1 text-[11.5px] text-text-muted">
          {draft.cap === ''
            ? 'Uncapped — one device in many places at once is a signal.'
            : 'A phone and a laptop is plausible; a hundred is not.'}
        </p>
      </div>
      <div>
        <label className="label" htmlFor="pf-geo">
          Proxy geo
        </label>
        <input
          id="pf-geo"
          className="field"
          value={draft.geo}
          onChange={(e) => set({ geo: e.target.value.toUpperCase() })}
          placeholder="any, or e.g. US, DE"
          maxLength={GEO_MAX_LENGTH}
        />
        <p className="mt-1 text-[11.5px] text-text-muted">Picked from your proxies at first connect, then kept.</p>
      </div>
    </div>
  );
}
