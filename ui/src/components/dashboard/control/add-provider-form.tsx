/**
 * The Add provider form: a name and type, then either a CDP WebSocket URL or
 * a vendor API key, then capacity and routing weights.
 */
import { DRAFT_NUMBERS, PROVIDER_NAME_MAX, PROVIDER_TYPES } from './constants';
import { CdpUrlField, VendorKeyField } from './provider-endpoint';
import type { Control } from './use-control';

/** Label class for every field on the form. */
const LABEL = 'text-xs text-text-dim space-y-1 block';

/** The form; submitting saves the provider and closes it. */
export default function AddProviderForm({ ctl }: { /** Control state and actions. */ ctl: Control }) {
  return (
    <form
      aria-label="Add provider"
      className="border border-border rounded-xl p-5 space-y-5 bg-bg-card/45"
      onSubmit={ctl.submitAdd}
    >
      <NameAndType ctl={ctl} />
      {ctl.draft.type === 'cdp' ? <CdpUrlField ctl={ctl} /> : <VendorKeyField ctl={ctl} />}
      <NumberFields ctl={ctl} />
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={!!ctl.busy}
          className="px-3 py-1.5 rounded text-xs font-medium bg-accent text-bg disabled:opacity-40"
        >
          {ctl.busy === 'add-provider' ? 'Adding…' : 'Add provider'}
        </button>
        <button
          type="button"
          disabled={!!ctl.busy}
          onClick={ctl.cancelAdd}
          className="px-3 py-1.5 rounded text-xs text-text-dim hover:text-text"
        >
          Cancel
        </button>
      </div>
    </form>
  );
}

/** Provider name and type; changing type drops a typed vendor key. */
function NameAndType({ ctl }: { /** Control state and actions. */ ctl: Control }) {
  const { draft, busy, edit } = ctl;
  return (
    <div className="grid gap-3 sm:grid-cols-2">
      <label className={LABEL}>
        <span>Name</span>
        <input
          required
          maxLength={PROVIDER_NAME_MAX}
          disabled={!!busy}
          value={draft.name}
          onChange={(e) => edit({ name: e.target.value })}
          placeholder="my-chrome"
          className="settings-input"
        />
      </label>
      <label className={LABEL}>
        <span>Type</span>
        <select
          aria-label="Type"
          disabled={!!busy}
          value={draft.type}
          onChange={(e) => edit({ type: e.target.value, apiKey: '' })}
          className="settings-input"
        >
          {PROVIDER_TYPES.map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}

/** Max sessions, priority and weight; priority alone may be zero. */
function NumberFields({ ctl }: { /** Control state and actions. */ ctl: Control }) {
  return (
    <div className="grid gap-3 sm:grid-cols-3">
      {DRAFT_NUMBERS.map(([k, label]) => (
        <label key={k} className={LABEL}>
          <span>{label}</span>
          <input
            required
            disabled={!!ctl.busy}
            type="number"
            min={k === 'priority' ? 0 : 1}
            step={1}
            value={ctl.draft[k]}
            onChange={(e) => ctl.edit({ [k]: e.target.value })}
            className="settings-input font-mono"
          />
        </label>
      ))}
    </div>
  );
}
