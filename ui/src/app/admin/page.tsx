/**
 * The admin page, for Oya staff (a confirmed @getoya.ai account): accounts and
 * plans, growth day by day and week over week, revenue from Stripe,
 * downloads, self-hosted installs and their licenses, the heaviest users, the
 * live fleet, and a person looked up by email. The server decides
 * who may see it; anyone else is told so. Its state lives in
 * `components/admin/use-admin.ts`.
 */
'use client';

import Link from 'next/link';
import { Loader2 } from 'lucide-react';
import { useAdmin, type AdminState } from '@/components/admin/use-admin';
import { Downloads, Fleet, Headline, Installs, TopUsers } from '@/components/admin/overview-sections';
import { Daily, Growth, Reach } from '@/components/admin/growth-sections';
import Licenses from '@/components/admin/licenses';
import Lookup from '@/components/admin/lookup';
import type { Overview } from '@/components/admin/types';

/** A short message in the middle of the page. */
const Notice = ({ children }: { /** The message. */ children: React.ReactNode }) => (
  <main className="flex min-h-screen items-center justify-center bg-bg p-6 text-sm text-text-muted">{children}</main>
);

/** Every section, once the overview has loaded. */
function Sections({ s, o }: { /** The page's state. */ s: AdminState; /** The loaded overview. */ o: Overview }) {
  return (
    <main className="mx-auto flex max-w-6xl flex-col gap-4 bg-bg p-4 lg:p-6">
      <h1 className="text-lg font-semibold text-text">Admin</h1>
      <Headline o={o} />
      <Reach o={o} />
      <Growth o={o} />
      <Daily o={o} />
      <Lookup s={s} />
      <Licenses s={s} />
      <Installs o={o} />
      <Downloads o={o} />
      <TopUsers o={o} />
      <Fleet o={o} />
    </main>
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
