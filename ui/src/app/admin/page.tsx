/** Five staff domains share navigation while every source retains its own loading, refresh and access lifecycle. */
'use client';
import Link from 'next/link';
import { Loader2 } from 'lucide-react';
import { useAdmin, type AdminState } from '@/components/admin/use-admin';
import { useLookup, type LookupState } from '@/components/admin/use-lookup';
import { useTab } from '@/components/admin/use-tab';
import { TopBar } from '@/components/admin/tabs';
import { OverviewTab } from '@/components/admin/overview-tab';
import { FleetTab, SelfHostedTab } from '@/components/admin/overview-sections';
import { CustomersTab } from '@/components/admin/lookup';
import { KeelView } from '@/components/admin/keel-tab';
import { FeedNotice, SnapshotHeading } from '@/components/admin/feed-state';
import type { TabId } from '@/components/admin/constants';

/** Access notices reveal no previous protected snapshot. */
const Notice = ({ children }: { /** Authentication or permission message. */ children: React.ReactNode }) => (
  <main className="flex min-h-screen items-center justify-center bg-bg p-6 text-sm text-text-muted">{children}</main>
);

/** Tab bodies can display independent data even when core activity is unavailable. */
interface TabProps {
  /** Authorized feeds and mutations. */ s: AdminState;
  /** Selected customer persists across ordinary feed refresh. */ l: LookupState;
}

/** Explicit scope descriptions make tabs easier to scan without opening documentation. */
const SCOPES = {
  overview: 'Account and product activity · completed UTC days',
  customers: 'Selected customer allowance period · calendar-month rankings',
  'self-hosted': 'Reported Oya installs · issued Oya licenses',
  fleet: 'Browser connections · responding server only',
};

/** Dispatches only known tabs, preserving independent sources and selected customers. */
const BODIES: Record<Exclude<TabId, 'keel'>, (props: TabProps) => React.ReactNode> = {
  overview: ({ s }) => <OverviewTab o={s.data.overview} s={s} />,
  customers: ({ s, l }) => <CustomersTab s={s} l={l} o={s.data.overview} />,
  'self-hosted': ({ s }) => <SelfHostedTab s={s} o={s.data.overview} />,
  fleet: ({ s }) => s.data.overview && <FleetTab o={s.data.overview} />,
};

/** Keel and financial data stay independently useful when another request fails. */
function TabContent({ s, tab, l }: TabProps & { /** Selected section. */ tab: TabId }) {
  if (tab === 'keel') return <KeelView state={{ ...s.keelFeed, snapshot: s.keelFeed.value }} />;
  const title = { overview: 'Overview', customers: 'Customers', 'self-hosted': 'Self-hosted', fleet: 'Fleet' }[tab];
  return (
    <div className="flex flex-col gap-6">
      <SnapshotHeading
        title={title}
        scope={SCOPES[tab]}
        updated={s.data.overview?.generated_at}
        feed={s.overviewFeed}
      />
      <FeedNotice feed={s.overviewFeed} available={Boolean(s.data.overview)} name="Core activity" />
      {BODIES[tab]({ s, l })}
    </div>
  );
}

/** Authorization-scoped section ownership clears one-time keys and customer selections on credential changes. */
function Sections({ s }: { /** Authorized page state. */ s: AdminState }) {
  const { tab, open } = useTab();
  const l = useLookup(s.lookup, s.scope);
  return (
    <div className="min-h-screen bg-bg">
      <TopBar tab={tab} open={open} l={l} />
      <main
        role="tabpanel"
        id="admin-panel"
        aria-labelledby={`admin-tab-${tab}`}
        tabIndex={0}
        className="mx-auto max-w-6xl px-4 py-6 outline-offset-4 focus-visible:outline-accent lg:px-6"
      >
        <TabContent s={s} tab={tab} l={l} />
      </main>
    </div>
  );
}

/** Staff-only admin navigation remains available through ordinary source failures. */
export default function AdminPage() {
  const s = useAdmin();
  if (s.loading)
    return (
      <Notice>
        <Loader2 className="h-5 w-5 animate-spin" aria-label="Loading account" />
      </Notice>
    );
  if (!s.signedIn)
    return (
      <Notice>
        <span>
          Sign in with your getoya.ai account.{' '}
          <Link className="text-accent" href="/login">
            Sign in
          </Link>
        </span>
      </Notice>
    );
  if (s.data.denied) return <Notice>{s.data.error || 'Your admin access is unavailable. Sign in again.'}</Notice>;
  return <Sections key={s.scope.id} s={s} />;
}
