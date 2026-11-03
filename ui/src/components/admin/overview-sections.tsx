/** Reported installations, customer rankings and local server connections use distinct, explicit scopes. */
import { Kpi, Section, Table } from './parts';
import { FeedNotice } from './feed-state';
import Licenses from './licenses';
import { hours, utcTime } from './model';
import type { AdminState } from './use-admin';
import type { Overview, Person } from './types';

/** Top-ten month leaders are independent of the selected customer's allowance period. */
export function TopUsers({
  o,
  pick,
}: {
  /** Core snapshot. */ o: Overview;
  /** Customer selection. */ pick: (email: string) => void;
}) {
  const person = (p: Person) =>
    p.email ? (
      <button key={p.userId} onClick={() => pick(p.email!)} className="break-all text-accent hover:underline">
        {p.email}
      </button>
    ) : (
      p.userId
    );
  const row = (p: Person) => [person(p), hours(p.cloud_seconds), p.agent_steps];
  return (
    <Section
      title="Calendar-month leaders"
      hint={`${utcTime(o.periods.month.from)} through ${utcTime(o.periods.month.to)} · Top 10`}
    >
      <div className="grid gap-4 lg:grid-cols-2">
        <Table
          head={['By cloud hours', 'Hours', 'Steps']}
          empty="No cloud usage recorded this calendar month."
          rows={o.top.cloud.map(row)}
        />
        <Table
          head={['By agent steps', 'Hours', 'Steps']}
          empty="No agent steps recorded this calendar month."
          rows={o.top.steps.map(row)}
        />
      </div>
    </Section>
  );
}

/** Full reported identifiers are observations rather than proof of issued or valid licenses. */
function Installs({ o }: { /** Complete counts with a bounded table. */ o: Overview }) {
  const rows = o.installs.list.map((install) => [
    <span key={install.install_id} className="break-all font-mono">
      {install.install_id}
    </span>,
    install.version,
    install.browsers,
    install.peak_cloud,
    <span key="license" className="break-all font-mono">
      {install.license_id || 'None reported'}
    </span>,
    install.pings,
    utcTime(install.last_seen),
  ]);
  return (
    <Section
      title="Reported installations"
      hint={`Showing ${rows.length} of ${o.installs.total} · Most recently seen first`}
    >
      <p className="text-xs text-text-muted">
        Browsers and peak cloud are values from each install’s last report. A license ID is self-reported.
      </p>
      <Table
        head={[
          'Install ID',
          'Version',
          'Browsers reported',
          'Peak cloud reported',
          'Reported license ID',
          'Pings',
          'Last report · UTC',
        ]}
        rows={rows}
      />
    </Section>
  );
}

/** Observation failures do not hide the independent issuance controls or one-time key. */
export function SelfHostedTab({
  s,
  o,
}: {
  /** Independent feeds. */ s: AdminState;
  /** Core snapshot. */ o: Overview | null;
}) {
  return (
    <div className="flex flex-col gap-6">
      {o && (
        <>
          <div className="grid gap-3 sm:grid-cols-3">
            <Kpi label="Installs seen · all time" value={o.installs.total} />
            <Kpi label="Reporting · last 7 days" value={o.installs.active} sub="Reports received; not online now" />
            <Kpi label="Above cap · no reported ID" value={o.installs.overCap} sub="Recently reporting installs only" />
          </div>
          <Installs o={o} />
        </>
      )}
      <div className="space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h2 className="text-sm font-semibold text-text">Issued Oya licenses</h2>
          <button
            type="button"
            disabled={s.licenseFeed.loading}
            onClick={s.licenseFeed.refresh}
            className="min-h-11 text-xs text-accent"
          >
            Refresh licenses
          </button>
        </div>
        <FeedNotice feed={s.licenseFeed} available={Boolean(s.licenseFeed.value)} name="Issued licenses" />
        <Licenses s={s} />
      </div>
    </div>
  );
}

/** Connection counts belong to the responding server, not a global Kubernetes inventory. */
export function FleetTab({ o }: { /** Core snapshot. */ o: Overview }) {
  const rows = Object.entries(o.fleet.byProvider).map(([provider, count]) => [provider, count]);
  return (
    <div className="flex flex-col gap-6">
      <p className="text-xs text-text-muted">
        Connections registered on the responding server at {utcTime(o.fleet.generated_at)}. Other replicas are outside
        this snapshot.
      </p>
      <div className="grid gap-3 sm:grid-cols-2">
        <Kpi label="Connected browsers · this server" value={o.fleet.total} />
        <Kpi label="Oya Cloud + self-hosted" value={o.fleet.cloud} sub="Provider subtotal on this server" />
      </div>
      <Section title="By provider">
        {rows.length ? (
          <Table head={['Provider', 'Connected browsers']} rows={rows} />
        ) : (
          <p className="text-xs text-text-dim">No browsers connected to this server.</p>
        )}
      </Section>
    </div>
  );
}
