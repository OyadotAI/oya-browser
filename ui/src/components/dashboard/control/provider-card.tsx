/**
 * One routing provider: health, capacity, and its figures. Shared providers
 * are the host's and cannot be removed; busy ones cannot be either.
 */
import { Circle, Trash2 } from 'lucide-react';
import { Bar } from './stat';
import { MS_PER_SECOND } from './constants';
import { num, providerStatus } from './format';
import { del } from './requests';
import type { Control } from './use-control';
import type { Provider } from './types';

/** A provider's card. */
export default function ProviderCard({
  p,
  ctl,
}: {
  /** The provider. */ p: Provider;
  /** Control state and actions. */ ctl: Control;
}) {
  return (
    <div className="border border-border rounded-lg p-4 space-y-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <Circle className={`w-2.5 h-2.5 shrink-0 fill-current ${p.healthy ? 'text-accent' : 'text-red'}`} />
          <span className="font-medium truncate">{p.name}</span>
          <span className="text-text-dim text-xs">{p.type}</span>
        </div>
        {p.shared ? (
          <span className="text-text-dim text-xs" title="Declared by the host via OYA_PROVIDERS">
            shared
          </span>
        ) : (
          <RemoveButton p={p} ctl={ctl} />
        )}
      </div>
      <p className="text-xs text-text-muted">{providerStatus(p)}</p>
      <Bar used={p.active} capacity={p.maxConcurrent} />
      <ProviderFigures p={p} />
    </div>
  );
}

/** Removes a provider the key owns, once nothing runs on it. */
function RemoveButton({
  p,
  ctl,
}: {
  /** The provider. */ p: Provider;
  /** Control state and actions. */ ctl: Control;
}) {
  const remove = () => void ctl.act(p.name, () => del(ctl.apiKey, `/gateway/providers/${encodeURIComponent(p.name)}`));
  return (
    <button
      disabled={!!ctl.busy || p.active > 0}
      onClick={remove}
      className="text-text-dim hover:text-red disabled:opacity-40"
      title={p.active ? 'End active sessions before removing' : 'Remove provider'}
      aria-label={`Remove ${p.name}`}
    >
      <Trash2 className="w-3.5 h-3.5" />
    </button>
  );
}

/** Slots, priority, latency, sessions, and any failures or cooldown. */
function ProviderFigures({ p }: { /** The provider. */ p: Provider }) {
  return (
    <div className="grid grid-cols-2 gap-y-1 text-xs text-text-dim">
      <span>
        Slots{' '}
        <span className="text-text font-mono">
          {p.active}/{p.maxConcurrent}
        </span>
      </span>
      <span>
        Priority <span className="text-text font-mono">{p.priority}</span>
      </span>
      <span>
        Latency <span className="text-text font-mono">{p.latencyMs == null ? '—' : `${p.latencyMs}ms`}</span>
      </span>
      <span>
        Sessions <span className="text-text font-mono">{num(p.totalSessions)}</span>
      </span>
      {p.totalFailures > 0 && (
        <span className="text-yellow">
          Failures <span className="font-mono">{p.totalFailures}</span>
        </span>
      )}
      {p.cooldownMsRemaining > 0 && (
        <span className="text-red">
          Cooldown <span className="font-mono">{Math.ceil(p.cooldownMsRemaining / MS_PER_SECOND)}s</span>
        </span>
      )}
    </div>
  );
}
