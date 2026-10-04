/**
 * The admin page's state: the overview and licenses, loaded for the signed-in
 * person, and the actions it takes (issue, revoke, look a person up, log in as them). The
 * server decides who is an admin; a refusal is shown as it was said.
 */
import { useEffect, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import {
  adminImpersonate,
  adminIssueLicense,
  adminLicenses,
  adminLookup,
  adminOverview,
  adminRevokeLicense,
} from '@/lib/api';
import { setImpersonation } from '@/lib/auth/storage';
import { stepUpUrl } from '@/lib/auth/step-up';
import type { Found, License, Overview } from './types';

/** Loaded data, or why it could not be. */
interface Loaded {
  /** The overview. */
  overview: Overview | null;
  /** Issued licenses. */
  licenses: License[];
  /** Why loading failed; empty when it did not. */
  error: string;
}

/** Whether the server refused because admins must sign in with a second factor and this session has not. */
const needsSecondFactor = (e: unknown) => Object(e).code === 'mfa_required';

/**
 * Loads the overview and licenses, or says why not. Refused for want of a
 * second factor, it sends the person to the code page (which offers set-up
 * when they have no authenticator yet) and back here.
 */
async function load(token: string): Promise<Loaded> {
  try {
    const [overview, { licenses }] = await Promise.all([adminOverview(token), adminLicenses(token)]);
    return { overview, licenses, error: '' };
  } catch (e) {
    if (needsSecondFactor(e)) window.location.assign(stepUpUrl('/admin'));
    return { overview: null, licenses: [], error: e instanceof Error ? e.message : 'Could not load' };
  }
}

/** The data, reloaded whenever `version` moves on. */
function useLoaded(token: string | null) {
  const [data, setData] = useState<Loaded | null>(null);
  const [version, setVersion] = useState(0);
  useEffect(() => {
    if (token) void load(token).then(setData);
  }, [token, version]);
  return { data, reload: () => setVersion((v) => v + 1) };
}

/** A license just issued, with its key, shown once. */
type Issued = License & {
  /** The license key. */
  key: string;
};

/** Opens the dashboard in this tab as that customer, on a one-hour "Login as" token. */
async function loginAs(token: string, id: string) {
  const { impersonate_token, email } = await adminImpersonate(token, id);
  setImpersonation({ token: impersonate_token, email });
  window.location.replace('/dashboard');
}

/** Issue, revoke and look up, each reloading what it changed. */
function useActions(token: string, reload: () => void) {
  const issue = async (request: Record<string, unknown>) => {
    const issued: Issued = await adminIssueLicense(token, request);
    reload();
    return issued;
  };
  const revoke = async (id: string) => void (await adminRevokeLicense(token, id), reload());
  const lookup = async (email: string): Promise<Found> => adminLookup(token, email);
  return { issue, revoke, lookup, loginAs: (id: string) => loginAs(token, id) };
}

/** Everything the admin page shows and does. */
export function useAdmin() {
  const { token, user, loading } = useAuth();
  const { data, reload } = useLoaded(token);
  return { signedIn: Boolean(user), loading, data, ...useActions(token || '', reload) };
}

/** The admin page's state, as its sections take it. */
export type AdminState = ReturnType<typeof useAdmin>;
