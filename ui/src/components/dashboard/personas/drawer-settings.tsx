/**
 * The profile drawer's editable settings: name, cap, proxy geo hint, and the
 * exit proxy it is pinned to.
 */
'use client';

import { GEO_MAX_LENGTH } from './constants';
import { savePin } from './drawer-actions';
import { choiceLabel } from './model';
import type { DrawerCtx } from './use-persona-drawer';

/** The exit picker, applied on its own; changing it mid-life changes the IP the identity is known by. */
function ExitPicker(ctx: DrawerCtx) {
  const { d, p } = ctx;
  return (
    <div>
      <label className="label" htmlFor="pd-pin">
        Exit proxy
      </label>
      <div className="flex gap-2">
        <select id="pd-pin" className="field" value={d.fields.pin} onChange={(e) => d.set({ pin: e.target.value })}>
          <option value="">Auto, assigned at first connect{p.exit ? ` (currently ${p.exit.label})` : ''}</option>
          {d.proxies.map((x) => (
            <option key={x.id} value={x.id}>
              {choiceLabel(x)}
            </option>
          ))}
        </select>
        <button
          className="btn-ghost shrink-0"
          onClick={() => savePin(ctx)}
          disabled={d.busy === 'pin' || (d.fields.pin || '') === (p.exit?.id || '')}
        >
          Apply
        </button>
      </div>
      {p.exit && (
        <p className="mt-1 text-[11.5px] text-text-muted">
          On <span className="text-text">{p.exit.label}</span>
          {p.exit.geo ? ` (${p.exit.geo})` : ''}
          {p.exit.healthy ? '' : ', unhealthy'}. Changing the exit mid-life changes the IP this identity is known by.
        </p>
      )}
    </div>
  );
}

/** Name, cap and geo hint (saved from the footer), then the exit picker. */
export default function SettingsSection(ctx: DrawerCtx) {
  const { d } = ctx;
  return (
    <section className="space-y-3">
      <div>
        <label className="label" htmlFor="pd-name">
          Name
        </label>
        <input id="pd-name" className="field" value={d.fields.name} onChange={(e) => d.set({ name: e.target.value })} />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="label" htmlFor="pd-cap">
            Concurrent browsers
          </label>
          <input
            id="pd-cap"
            className="field num"
            type="number"
            min={1}
            value={d.fields.cap}
            onChange={(e) => d.set({ cap: e.target.value })}
            placeholder="∞"
          />
        </div>
        <div>
          <label className="label" htmlFor="pd-geo">
            Proxy geo hint
          </label>
          <input
            id="pd-geo"
            className="field"
            value={d.fields.geo}
            onChange={(e) => d.set({ geo: e.target.value.toUpperCase() })}
            placeholder="any"
            maxLength={GEO_MAX_LENGTH}
          />
        </div>
      </div>
      <ExitPicker {...ctx} />
    </section>
  );
}
