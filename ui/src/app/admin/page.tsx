/**
 * The admin page, for Oya staff (a confirmed @getoya.ai account), in four
 * tabs under a sticky bar with the customer search: the overview (headline
 * numbers, what needs attention, growth), customers (lookup, Login as, the
 * heaviest users), self-hosted (installs, licenses, downloads) and the fleet. The server decides
 * who may see it; anyone else is told so. Its state lives in
 * `components/admin/use-admin.ts`.
 */
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
import type { TabId } from '@/components/admin/constants';
import type { Overview } from '@/components/admin/types';

/** A short message in the middle of the page. */
const Notice = ({ children }: { /** The message. */ children: React.ReactNode }) => (
  <main className="flex min-h-screen items-center justify-center bg-bg p-6 text-sm text-text-muted">{children}</main>
);

/** What every tab's body is drawn from. */
interface TabProps {
  /** The page's state. */
  s: AdminState;
  /** The loaded overview. */
  o: Overview;
  /** The customer lookup. */
  l: LookupState;
}

/** Each tab's body, by its id. */
const BODIES: Record<TabId, (p: TabProps) => React.ReactNode> = {
  overview: ({ o }) => <OverviewTab o={o} />,
  customers: ({ s, o, l }) => <CustomersTab s={s} l={l} o={o} />,
  'self-hosted': ({ s, o }) => <SelfHostedTab s={s} o={o} />,
  fleet: ({ o }) => <FleetTab o={o} />,
};

/** The loaded page: the top bar, then the open tab. */
function Sections({ s, o }: { /** The page's state. */ s: AdminState; /** The loaded overview. */ o: Overview }) {
  const { tab, open } = useTab();
  const l = useLookup(s.lookup);
  return (
    <div className="min-h-screen bg-bg">
      <TopBar tab={tab} open={open} l={l} />
      <main role="tabpanel" className="mx-auto max-w-6xl px-4 py-6 lg:px-6">
        {BODIES[tab]({ s, o, l })}
      </main>
    </div>
  );
}

/** The admin page. */
export default function AdminPage() {
  const s = useAdmin();
  if (s.loading)
    return (
      <Notice>
        <Loader2 className="h-5 w-5 animate-spin" />
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
  if (!s.data)
    return (
      <Notice>
        <Loader2 className="h-5 w-5 animate-spin" />
      </Notice>
    );
  if (s.data.error || !s.data.overview) return <Notice>{s.data.error || 'Nothing to show'}</Notice>;
  return <Sections s={s} o={s.data.overview} />;
}
