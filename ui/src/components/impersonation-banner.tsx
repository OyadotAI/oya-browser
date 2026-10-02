/**
 * The bar an admin sees across every page while logged in as a customer: whose
 * account this tab is in, and Exit, which ends it and goes back to the admin page.
 */
'use client';

import { useSyncExternalStore } from 'react';
import { impersonation, setImpersonation } from '@/lib/auth/storage';

/** The tab's "Login as" only changes with a page load, so there is nothing to listen to. */
const subscribe = () => () => {};

/** Whose account this tab is in ('' when it is the admin's own), read on the client only, and Exit. */
function useImpersonation() {
  const email = useSyncExternalStore(
    subscribe,
    () => impersonation()?.email ?? '',
    () => '',
  );
  const exit = () => {
    setImpersonation(null);
    window.location.replace('/admin');
  };
  return { email, exit };
}

/** A warning bar while impersonating; nothing otherwise. */
export function ImpersonationBanner() {
  const { email, exit } = useImpersonation();
  if (!email) return null;
  return (
    <div
      role="alert"
      className="sticky top-0 z-50 flex items-center justify-center gap-3 bg-amber-500 px-4 py-1.5 text-xs font-medium text-black"
    >
      <span>Logged in as {email}. Every action is logged under you.</span>
      <button onClick={exit} className="rounded border border-black/40 px-2 py-0.5 hover:bg-black/10">
        Exit
      </button>
    </div>
  );
}
