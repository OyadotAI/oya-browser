/**
 * The admin page's state: the overview and licenses, loaded for the signed-in
 * person, and the actions it takes (issue, revoke, look a person up). The
 * server decides who is an admin; a refusal is shown as it was said.
 */
import { useEffect, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import { adminIssueLicense, adminLicenses, adminLookup, adminOverview, adminRevokeLicense } from '@/lib/api';
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

/** Loads the overview and licenses, or says why not. */
async function load(token: string): Promise<Loaded> {
  try {
    const [overview, { licenses }] = await Promise.all([adminOverview(token), adminLicenses(token)]);
    return { overview, licenses, error: '' };
  } catch (e) {
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

/** Issue, revoke and look up, each reloading what it changed. */
function useActions(token: string, reload: () => void) {
  const issue = async (request: Record<string, unknown>) => {
    const issued: Issued = await adminIssueLicense(token, request);
    reload();
    return issued;
  };
  const revoke = async (id: string) => void (await adminRevokeLicense(token, id), reload());
  return { issue, revoke, lookup: async (email: string): Promise<Found> => adminLookup(token, email) };
}

/** Everything the admin page shows and does. */
export function useAdmin() {
  const { token, user, loading } = useAuth();
  const { data, reload } = useLoaded(token);
  return { signedIn: Boolean(user), loading, data, ...useActions(token || '', reload) };
}

/** The admin page's state, as its sections take it. */
export type AdminState = ReturnType<typeof useAdmin>;
