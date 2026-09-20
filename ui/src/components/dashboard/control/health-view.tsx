/**
 * The Overview view: headline stats for the hour and the key's remaining
 * allowance under each rate limit.
 */
import { Activity, AlertTriangle, Clock, Gauge, Server, ShieldCheck, Users, Zap } from 'lucide-react';
import { Stat } from './stat';
import LimitsTable from './limits-table';
import { duration, errorRateTone, healthFigures, num } from './format';
import type { ControlFleet } from './types';

/** Loading until the first poll lands, then stats and limits. */
export default function HealthView({ fleet }: { /** Fleet summary, once loaded. */ fleet: ControlFleet | null }) {
  if (!fleet) return <p className="text-text-dim text-sm">Loading…</p>;
  return (
    <div className="space-y-6">
      <ActivityStats fleet={fleet} />
      <CapacityStats fleet={fleet} />
      <LimitsTable limits={fleet.limits} />
    </div>
  );
}

/** Browsers, sessions, command errors and throttling. */
function ActivityStats({ fleet }: { /** Fleet summary, once loaded. */ fleet: ControlFleet }) {
  const u = fleet.usage;
  const { commands, errors, errorRate, throttled } = healthFigures(u);
  const clients = Object.entries(fleet.browsers.byClient).map(([k, v]) => `${k} ${v}`);
  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
      <Stat
        label="Your browsers"
        value={num(fleet.browsers.total)}
        icon={Users}
        sub={clients.join(' · ') || 'none connected'}
      />
      <Stat
        label="CDP sessions"
        value={num(fleet.sessions.total)}
        icon={Zap}
        sub={`${fleet.sessions.attached} attached · ${fleet.sessions.recording} recording`}
      />
      <Stat
        label="Command errors"
        value={`${errorRate.toFixed(1)}%`}
        icon={AlertTriangle}
        tone={errorRateTone(errorRate)}
        sub={`${num(errors)} of ${num(commands)} this hour`}
      />
      <Stat
        label="Throttled"
        value={num(throttled)}
        icon={ShieldCheck}
        tone={throttled > 0 ? 'warn' : 'good'}
        sub={`${num(u?.rate_limited)} rate · ${num(u?.quota_denied)} quota`}
      />
    </div>
  );
}

/** Browser time, model tokens, provider capacity and host uptime. */
function CapacityStats({ fleet }: { /** Fleet summary, once loaded. */ fleet: ControlFleet }) {
  const u = fleet.usage;
  const r = fleet.routing;
  const tokens = (u?.chat_input_tokens ?? 0) + (u?.chat_output_tokens ?? 0);
  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
      <Stat
        label="Browser time"
        value={duration(u?.browser_seconds ?? 0)}
        icon={Clock}
        sub={`${num(u?.browsers_started)} started this hour`}
      />
      <Stat
        label="Model tokens"
        value={num(tokens)}
        icon={Gauge}
        sub={`quota ${num(fleet.quotas.chatTokensPerHour)}/hr`}
      />
      <Stat
        label="Provider capacity"
        value={`${num(r.active)}/${num(r.capacity)}`}
        icon={Server}
        tone={r.healthy ? 'normal' : 'bad'}
        sub={`${r.healthy} healthy · queue ${r.queueDepth}`}
      />
      <Stat
        label="Host uptime"
        value={duration(fleet.uptimeSeconds)}
        icon={Activity}
        sub={new Date(fleet.at).toLocaleTimeString()}
      />
    </div>
  );
}
