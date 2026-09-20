/**
 * The console's gate: shows a spinner until a signed-in account or a working
 * console credential is confirmed, else sends the visitor to /login.
 */
'use client';

import { useEffect, useState, type PropsWithChildren } from 'react';
import { useRouter } from 'next/navigation';
import { Loader2 } from 'lucide-react';
import { useAuth } from '@/components/auth-provider';
import { ToastProvider } from '@/components/dashboard/toast';
import { apiUrl, authHeaders, consoleCredential } from '@/lib/api';

/**
 * An API key is enough to be here.
 *
 * The key is the identity for everything this dashboard shows — browsers,
 * personas, settings, usage — so requiring a hosted account on top of it would
 * lock a self-hosted deployment (which has API_KEYS and no Supabase) out of its
 * own UI. A signed-in account still works, and is what mints keys in the first
 * place.
 */
export default function DashboardLayout({ children }: PropsWithChildren) {
  const { user, loading } = useAuth();
  const router = useRouter();
  const [keyOk, setKeyOk] = useState<boolean | null>(null);

  // Re-checked whenever the session changes, not just on mount. Logging out
  // clears the stored key, but a keyOk left over from mount kept `allowed`
  // true, so the redirect below never fired and log out appeared to do
  // nothing — the session was gone, the page just stayed.
  useEffect(() => {
    const key = consoleCredential();
    let cancelled = false;
    const check = key
      ? fetch(apiUrl('/control'), { headers: authHeaders(key) }).then((res) => res.ok)
      : Promise.resolve(false);
    check
      .then((ok) => {
        if (!cancelled) setKeyOk(ok);
      })
      .catch(() => {
        if (!cancelled) setKeyOk(false);
      });
    return () => {
      cancelled = true;
    };
  }, [user]);

  const checking = loading || keyOk === null;
  const allowed = !!user || keyOk === true;

  useEffect(() => {
    if (!checking && !allowed) router.replace('/login');
  }, [checking, allowed, router]);

  if (checking || !allowed) {
    return (
      <div className="flex items-center justify-center min-h-screen bg-bg">
        <div className="flex flex-col items-center gap-3">
          <Loader2 className="w-8 h-8 animate-spin text-accent" />
          {checking && <p className="text-text-muted text-sm">Loading...</p>}
        </div>
      </div>
    );
  }

  return <ToastProvider>{children}</ToastProvider>;
}
