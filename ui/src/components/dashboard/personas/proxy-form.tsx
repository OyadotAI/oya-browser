/**
 * The add-a-proxy form: URL, label, country, type and how many profiles it
 * may serve.
 */
'use client';

import { Loader2 } from 'lucide-react';
import { GEO_MAX_LENGTH } from './constants';
import type { ProxyDraft } from './model';
import { add } from './proxy-actions';
import type { PartProps } from './use-proxies';

/** Label, country, type and max profiles, side by side. */
function Details({ s }: PartProps) {
  return (
    <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
      <Naming s={s} />
      <Capacity s={s} />
    </div>
  );
}

/** Label and country. */
function Naming({ s: { draft, set } }: PartProps) {
  return (
    <>
      <div>
        <label className="label" htmlFor="px-label">
          Label
        </label>
        <input
          id="px-label"
          className="field"
          value={draft.label}
          onChange={(e) => set({ label: e.target.value })}
          placeholder="us-home-1"
        />
      </div>
      <div>
        <label className="label" htmlFor="px-geo">
          Country
        </label>
        <input
          id="px-geo"
          className="field"
          value={draft.geo}
          onChange={(e) => set({ geo: e.target.value.toUpperCase() })}
          placeholder="US"
          maxLength={GEO_MAX_LENGTH}
        />
      </div>
    </>
  );
}

/** Type and how many profiles it may serve. */
function Capacity({ s: { draft, set } }: PartProps) {
  return (
    <>
      <div>
        <label className="label" htmlFor="px-kind">
          Type
        </label>
        <select
          id="px-kind"
          className="field"
          value={draft.kind}
          onChange={(e) => set({ kind: e.target.value as ProxyDraft['kind'] })}
        >
          <option value="residential">Residential</option>
          <option value="datacenter">Datacenter</option>
        </select>
      </div>
      <div>
        <label className="label" htmlFor="px-max">
          Max profiles
        </label>
        <input
          id="px-max"
          className="field num"
          type="number"
          min={1}
          value={draft.max}
          onChange={(e) => set({ max: e.target.value })}
        />
      </div>
    </>
  );
}

/** The whole add form; Add is off until a URL is typed. */
export default function ProxyForm({ s }: PartProps) {
  return (
    <fieldset className="space-y-3 rounded-xl border border-border p-4">
      <legend className="label px-1">Add a proxy</legend>
      <div>
        <label className="label" htmlFor="px-url">
          Proxy URL
        </label>
        <input
          id="px-url"
          className="field font-mono"
          value={s.draft.url}
          onChange={(e) => s.set({ url: e.target.value })}
          placeholder="http://user:pass@gate.vendor.com:7000"
          autoComplete="off"
          spellCheck={false}
        />
        <p className="mt-1 text-[11.5px] text-text-muted">
          From your proxy vendor. Use http or https: Chromium ignores SOCKS5 passwords. Stored encrypted and never shown
          again.
        </p>
      </div>
      <Details s={s} />
      <p className="text-[11.5px] text-text-muted">
        Keep max profiles at 1 for a sticky session URL, so each profile keeps its own IP.
      </p>
      <button className="btn-primary h-9" onClick={() => add(s)} disabled={!s.draft.url || s.busy === 'add'}>
        {s.busy === 'add' && <Loader2 className="h-3.5 w-3.5 animate-spin" />} Add proxy
      </button>
    </fieldset>
  );
}
