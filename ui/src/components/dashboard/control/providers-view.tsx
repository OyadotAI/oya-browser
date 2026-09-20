/**
 * The Providers view: routing strategy, the Add provider form and a card per
 * provider the key can route to.
 */
import { Plus } from 'lucide-react';
import AddProviderForm from './add-provider-form';
import ProviderCard from './provider-card';
import { STRATEGIES } from './constants';
import { post } from './requests';
import type { Control } from './use-control';
import type { Routing } from './types';

/** Toolbar, optional form, then provider cards. */
export default function ProvidersView({
  ctl,
  routing,
}: {
  /** Control state and actions. */ ctl: Control;
  /** Routing state. */ routing: Routing;
}) {
  return (
    <div className="space-y-4">
      <ProviderToolbar ctl={ctl} routing={routing} />
      {ctl.showAdd && <AddProviderForm ctl={ctl} />}
      <div className="grid gap-3 md:grid-cols-2">
        {routing.providers.map((p) => (
          <ProviderCard key={`${p.owner ?? 'shared'}:${p.name}`} p={p} ctl={ctl} />
        ))}
        {!routing.providers.length && (
          <p className="text-text-dim text-sm">
            No providers yet. Add one above — a CDP endpoint you run, or a hosted vendor. The host can also declare
            shared providers with OYA_PROVIDERS.
          </p>
        )}
      </div>
    </div>
  );
}

/** Add provider, the strategy picker and the slot summary. */
function ProviderToolbar({
  ctl,
  routing,
}: {
  /** Control state and actions. */ ctl: Control;
  /** Routing state. */ routing: Routing;
}) {
  const setStrategy = (strategy: string) =>
    void ctl.act('strategy', () => post(ctl.apiKey, '/gateway/strategy', { strategy }));
  return (
    <div className="flex flex-wrap items-center gap-3">
      <button
        disabled={!!ctl.busy}
        onClick={ctl.toggleAdd}
        className="flex items-center gap-1.5 px-3 py-2 rounded-md text-[12px] font-medium bg-accent text-bg hover:opacity-90"
      >
        <Plus className="w-3.5 h-3.5" /> Add provider
      </button>
      <span className="text-xs text-text-dim uppercase tracking-wider">Strategy</span>
      <select
        aria-label="Routing strategy"
        disabled={!!ctl.busy}
        value={routing.strategy}
        onChange={(e) => setStrategy(e.target.value)}
        className="bg-bg-elevated border border-border rounded px-2 py-1 text-sm"
      >
        {STRATEGIES.map((s) => (
          <option key={s} value={s}>
            {s}
          </option>
        ))}
      </select>
      <span className="text-xs text-text-dim">
        {routing.active} of {routing.capacity} slots · {routing.healthy} healthy · queue {routing.queueDepth}
      </span>
    </div>
  );
}
