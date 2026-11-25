/** Independent staff feeds preserve useful snapshots while access refusals clear the whole surface. */
import { useEffect, useMemo } from 'react';
import { useAuth } from '@/components/auth-provider';
import {
  adminImpersonate,
  adminSetPlan,
  adminGrant,
  adminIssueLicense,
  adminLicenses,
  adminLookup,
  adminOverview,
  adminRevenue,
  adminKeel,
  adminRevokeLicense,
} from '@/lib/api';
import { setImpersonation } from '@/lib/auth/storage';
import { useAdminFeed } from './use-admin-feed';
import type { Found, License, Overview } from './types';
import type { KeelSnapshot } from './keel-types';
import type { FinancialSnapshot } from './financial-types';

/** Authorization lifetime is an opaque object, never a rendered or persisted credential. */
export interface AdminScope {
  /** Non-secret UI lifetime clears sensitive forms on credential replacement. */ id?: string;
  /** Cleanup revokes mutations' right to display results. */ current: boolean;
  /** Memo identity changes with the underlying credential. */ authenticated: boolean;
}

/** Core data, finance and license lists cannot erase one another through Promise.all failure. */
function useSources(token: string | null) {
  return {
    overviewFeed: useAdminFeed<Overview>(token, adminOverview),
    financialFeed: useAdminFeed<FinancialSnapshot>(token, adminRevenue),
    keelFeed: useAdminFeed<KeelSnapshot>(token, adminKeel),
    licenseFeed: useAdminFeed<{ /** Issued Oya license inventory. */ licenses: License[] }>(token, adminLicenses),
  };
}

/** Named feeds are also the reusable refresh and error-state contract. */
type Sources = ReturnType<typeof useSources>;

/** An access failure clears every protected section, while ordinary data errors remain local. */
function loadedData(sources: Sources) {
  const refusal = Object.values(sources).find((feed) => feed.denied);
  return {
    overview: refusal ? null : sources.overviewFeed.value,
    licenses: refusal ? [] : (sources.licenseFeed.value?.licenses ?? []),
    error: refusal?.error || sources.overviewFeed.error,
    licenseError: sources.licenseFeed.error,
    denied: Boolean(refusal),
  };
}

/** A one-time issued key must never be handed to a replacement authenticated session. */
function requireCurrent(scope: AdminScope) {
  if (!scope.current) throw new Error('Your admin session changed. Sign in again before continuing.');
}

/** Global impersonation storage is changed only while the original authorizing session still owns it. */
async function loginAs(token: string, id: string, scope: AdminScope) {
  const { impersonate_token, email } = await adminImpersonate(token, id);
  requireCurrent(scope);
  setImpersonation({ token: impersonate_token, email });
  window.location.replace('/dashboard');
}

/** Mutations carry the authorizing lifetime and the one source they refresh. */
type Mutation = {
  /** Authorizing credential. */ token: string;
  /** Revocable lifetime. */ scope: AdminScope;
  /** License-list refresh. */ refresh: () => void;
};

/** A one-time key is exposed only while its original authorizing session remains current. */
async function issued(context: Mutation, request: Record<string, unknown>) {
  const result = await adminIssueLicense(context.token, request);
  requireCurrent(context.scope);
  context.refresh();
  return result;
}

/** A revoked license refreshes only the issued-license list. */
async function revoked(context: Mutation, id: string) {
  await adminRevokeLicense(context.token, id);
  requireCurrent(context.scope);
  context.refresh();
}

/** Stable source ownership keeps background license refresh from unmounting its form. */
const actions = (context: Mutation) => ({
  issue: (request: Record<string, unknown>): Promise<License & { /** One-time issued key. */ key: string }> =>
    issued(context, request),
  revoke: (id: string) => revoked(context, id),
  lookup: (email: string): Promise<Found> => adminLookup(context.token, email),
  loginAs: (id: string) => loginAs(context.token, id, context.scope),
});

/** Billing actions keep their existing allowance and access semantics. */
const billingActions = (token: string | null) => ({
  setPlan: (id: string, request: Record<string, unknown>) => adminSetPlan(token || '', id, request),
  grant: (id: string, request: Record<string, unknown>) => adminGrant(token || '', id, request),
});

/** Access and refresh metadata are independent of whichever tab happens to be selected. */
function access(auth: ReturnType<typeof useAuth>, sources: Sources, scope: AdminScope) {
  return {
    signedIn: Boolean(auth.user),
    loading: auth.loading,
    data: loadedData(sources),
    scope,
    identity: auth.user?.id || auth.user?.email || '',
    reload: sources.overviewFeed.refresh,
  };
}

/** Closure-owned cancellation can be reactivated by React's effect probe without mutating React state. */
function ownership() {
  let current = true;
  return {
    read: () => current,
    set: (value: boolean) => {
      current = value;
    },
  };
}

/** The public lifetime has no credential; its getter tracks closure-owned revocation. */
function scopeView(owner: ReturnType<typeof ownership>, authenticated: boolean) {
  return Object.defineProperty({ id: crypto.randomUUID(), authenticated }, 'current', {
    enumerable: true,
    get: owner.read,
  }) as AdminScope;
}

/** Effect-owned activation and cleanup retain the same opaque lifetime object and live getter. */
function newScope(authenticated: boolean) {
  const owner = ownership();
  return Object.assign(scopeView(owner, authenticated), {
    activate: () => owner.set(true),
    revoke: () => owner.set(false),
  });
}

/** Combines feeds and actions without coupling their refresh lifetimes. */
function composed(auth: ReturnType<typeof useAuth>, sources: Sources, scope: AdminScope) {
  const mutation = { token: auth.token || '', refresh: sources.licenseFeed.refresh, scope };
  return { ...access(auth, sources, scope), ...sources, ...actions(mutation), ...billingActions(auth.token) };
}

/** The admin page owns source lifetimes; tab navigation and background refresh do not remount forms. */
export function useAdmin() {
  const auth = useAuth();
  const sources = useSources(auth.token);
  const scope = useMemo(() => newScope(Boolean(auth.token)), [auth.token]);
  useEffect(() => {
    scope.activate();
    return scope.revoke;
  }, [scope]);
  return composed(auth, sources, scope);
}

/** Public state shape used by existing account and license controls. */
export type AdminState = ReturnType<typeof useAdmin>;
