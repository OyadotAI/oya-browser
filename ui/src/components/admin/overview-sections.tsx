/**
 * The admin overview's read-only sections: headline numbers, downloads,
 * heaviest users, installs and the live fleet.
 */
import { Section, Table, Tile } from './parts';
import { INSTALL_ID_CHARS, INSTALLS_LISTED } from './constants';
import { dayOf, downloadsByDay, hours } from './model';

import type { Overview, Person } from './types';

/** Accounts, plans, installs and the fleet, one number each. */
export function Headline({ o }: { /** The overview. */ o: Overview }) {
  const paying =
    Object.entries(o.accounts.byPlan || {})
      .map(([plan, n]) => `${n} ${plan}`)
      .join(', ') || 'none';
  return (
    <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
      <Tile label="Accounts" value={o.accounts.total} />
      <Tile label="Paying" value={paying} />
      <Tile label="Payment failed" value={o.accounts.pastDue} />
      <Tile label="Browsers now (cloud)" value={`${o.fleet.total} (${o.fleet.cloud})`} />
      <Tile label="Self-hosted installs" value={o.installs.total} />
      <Tile label="Active this week" value={o.installs.active} />
      <Tile label="Unlicensed over cap" value={o.installs.overCap} />
      <Tile label="Signups (30 days)" value={o.accounts.signups.reduce((n, d) => n + d.count, 0)} />
    </div>
  );
}

/** Downloads per day. */
export function Downloads({ o }: { /** The overview. */ o: Overview }) {
  const rows = downloadsByDay(o.downloads).map((d) => [d.day, d.installer, d.update, d.update_check]);
  return (
    <Section title="Downloads (30 days)">
      <Table head={['Day', 'Installers', 'Updates', 'Update checks']} rows={rows} />
    </Section>
  );
}

/** This month's heaviest users by cloud hours and by agent steps. */
export function TopUsers({ o }: { /** The overview. */ o: Overview }) {
  const row = (p: Person) => [p.email || p.userId, hours(p.cloud_seconds), p.agent_steps];
  return (
    <Section title="Heaviest users this month">
      <Table head={['By cloud hours', 'Hours', 'Steps']} rows={o.top.cloud.map(row)} />
      <Table head={['By agent steps', 'Hours', 'Steps']} rows={o.top.steps.map(row)} />
    </Section>
  );
}

/** Self-hosted installs, most recently seen first. */
export function Installs({ o }: { /** The overview. */ o: Overview }) {
  const rows = o.installs.list
    .slice(0, INSTALLS_LISTED)
    .map((i) => [
      i.install_id.slice(0, INSTALL_ID_CHARS),
      i.version,
      i.browsers,
      i.peak_cloud,
      i.license_id ? 'yes' : 'no',
      i.pings,
      dayOf(i.last_seen),
    ]);
  return (
    <Section title="Self-hosted installs">
      <Table head={['Install', 'Version', 'Browsers', 'Peak cloud', 'Licensed', 'Pings', 'Last seen']} rows={rows} />
    </Section>
  );
}

/** Browsers connected now, by provider. */
export function Fleet({ o }: { /** The overview. */ o: Overview }) {
  return (
    <Section title="Fleet now">
      <Table
        head={['Provider', 'Browsers']}
        rows={Object.entries(o.fleet.byProvider).map(([p, n]) => [p, n as number])}
      />
    </Section>
  );
}
