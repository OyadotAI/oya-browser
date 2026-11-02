// Status: one snapshot of your project, what is running, what it cost, and how much allowance is left.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
const [overview, usage] = await Promise.all([oya.control.overview(), oya.usage() as Promise<any>]);

// States: pending → provisioning → ready, then stopped, cancelled, disconnected or unknown_outcome.
const byState: Record<string, number> = {};
for (const s of overview.sessions) byState[s.state] = (byState[s.state] ?? 0) + 1;
console.log(`project ${overview.project.name}:`, byState);
const live = overview.sessions.filter((s) => ['pending', 'provisioning', 'ready'].includes(s.state));
for (const s of live)
  console.log(`  ${s.id}  ${s.state.padEnd(12)} ${s.provider}  persona=${s.persona}  $${s.costUsd}`);

const hour = usage.current;
console.log(
  `this hour: ${hour.commands} commands (${hour.command_errors} errors), ${hour.browsers_started} browsers started`,
);
console.log(`browsers connected: ${usage.browsers.connected} of ${usage.browsers.quota}`);
console.log(`commands left in this window: ${usage.limits.command.remaining}`);
