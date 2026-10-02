/**
 * The admin page's other tabs: the heaviest users (each one a lookup away),
 * self-hosted installs with their licenses and downloads, and the live fleet.
 */
import { Fold, Kpi, Section, Table } from './parts';
import Licenses from './licenses';
import { INSTALL_ID_CHARS, INSTALLS_LISTED } from './constants';
import { dayOf, downloadsByDay, hours } from './model';
import type { AdminState } from './use-admin';
import type { Overview, Person } from './types';

/** Downloads per day, folded away. */
function Downloads({ o }: { /** The overview. */ o: Overview }) {
  const rows = downloadsByDay(o.downloads).map((d) => [d.day, d.installer, d.update, d.update_check]);
  return (
    <Fold label="Downloads, day by day (30 days)">
      <Table head={['Day', 'Installers', 'Updates', 'Update checks']} rows={rows} />
    </Fold>
  );
}

/** This month's heaviest users by cloud hours and by agent steps; an email looks that person up. */
export function TopUsers({
  o,
  pick,
}: {
  /** The overview. */ o: Overview;
  /** Looks a person up. */ pick: (email: string) => void;
}) {
  const person = (p: Person) =>
    p.email ? (
      <button key={p.userId} onClick={() => pick(p.email!)} className="text-accent hover:underline">
        {p.email}
      </button>
    ) : (
      p.userId
    );
  const row = (p: Person) => [person(p), hours(p.cloud_seconds), p.agent_steps];
  return (
    <Section title="Heaviest this month" hint="click someone to look them up">
      <div className="grid gap-4 lg:grid-cols-2">
        <Table head={['By cloud hours', 'Hours', 'Steps']} rows={o.top.cloud.map(row)} />
        <Table head={['By agent steps', 'Hours', 'Steps']} rows={o.top.steps.map(row)} />
      </div>
    </Section>
  );
}

/** Self-hosted installs, most recently seen first. */
function Installs({ o }: { /** The overview. */ o: Overview }) {
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
    <Section title="Installs" hint="most recently seen first">
      <Table head={['Install', 'Version', 'Browsers', 'Peak cloud', 'Licensed', 'Pings', 'Last seen']} rows={rows} />
    </Section>
  );
}

/** The self-hosted tab: how many installs, their licenses, the installs themselves and downloads. */
export function SelfHostedTab({ s, o }: { /** The page's state. */ s: AdminState; /** The overview. */ o: Overview }) {
  return (
    <div className="flex flex-col gap-6">
      <div className="grid grid-cols-3 gap-3">
        <Kpi label="Installs" value={o.installs.total} />
        <Kpi label="Active this week" value={o.installs.active} />
        <Kpi label="Unlicensed over cap" value={o.installs.overCap} />
      </div>
      <Licenses s={s} />
      <Installs o={o} />
      <Downloads o={o} />
    </div>
  );
}

/** The fleet tab: browsers connected now, in all and by provider. */
export function FleetTab({ o }: { /** The overview. */ o: Overview }) {
  const rows = Object.entries(o.fleet.byProvider).map(([p, n]) => [p, n as number]);
  return (
    <div className="flex flex-col gap-6">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <Kpi label="Browsers now" value={o.fleet.total} />
        <Kpi label="In the cloud" value={o.fleet.cloud} />
      </div>
      <Section title="By provider">
        <Table head={['Provider', 'Browsers']} rows={rows} />
      </Section>
    </div>
  );
}
