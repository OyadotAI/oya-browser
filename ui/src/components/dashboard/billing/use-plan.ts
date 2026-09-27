/**
 * The account dialog's plan: what the person is on and used this period, and
 * the two ways to change it, both of which leave for a Stripe page.
 */
import { useCallback, useEffect, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import { billingCheckout, billingPortal, getBilling } from '@/lib/api';
import { SECONDS_PER_HOUR } from '@/lib/constants';
import { TENTHS } from './constants';

/** A plan's allowances. */
export interface Included {
  /** Cloud browser time, in seconds. */
  cloudSeconds: number;
  /** Agent steps. */
  steps: number;
  /** Residential proxy traffic, in bytes. */
  proxyBytes?: number;
  /** The hosted model's cost, in micro-USD. */
  llmMicroUsd?: number;
  /** Cloud browsers at once. */
  concurrent?: number;
  /** Whether use past the allowances is billed rather than refused. */
  overage?: boolean;
}

/** What was used this period. */
export interface Used {
  /** Cloud browser time, in seconds. */
  cloud_seconds?: number;
  /** Agent steps. */
  agent_steps?: number;
  /** Residential proxy traffic, in bytes. */
  residential_proxy_bytes?: number;
  /** The hosted model's cost, in micro-USD. */
  hosted_llm_microusd?: number;
}

/** The plan as the server says it. */
export interface Billing {
  /** False on a self-hosted server, which has no plans. */
  enabled: boolean;
  /** free, developer or startup. */
  plan?: string;
  /** The subscription's status, when there is one. */
  status?: string | null;
  /** The plan's allowances. */
  included?: Included;
  /** What was used this period. */
  used?: Used;
  /** When the period began. */
  since?: string;
  /** When it ends, for a paid plan. */
  until?: string | null;
}

/** A page the person is sent to. */
export interface Redirect {
  /** Its address. */
  url: string;
}

/** Hours with one decimal. */
export const hours = (seconds = 0) => Math.round((seconds / SECONDS_PER_HOUR) * TENTHS) / TENTHS;

/** Loads the plan whenever the dialog opens; nothing on a self-hosted server or a failed load. */
function useBilling(open: boolean, token: string | null) {
  const [billing, setBilling] = useState<Billing | null>(null);
  useEffect(() => {
    if (!open || !token) return;
    getBilling(token)
      .then(setBilling)
      .catch(() => setBilling(null));
  }, [open, token]);
  return billing;
}

/** Goes to the page `page` opens; answers why not when it cannot. */
async function goTo(page: () => Promise<Redirect>) {
  try {
    window.location.assign((await page()).url);
    return '';
  } catch (e) {
    return e instanceof Error ? e.message : 'Could not open billing';
  }
}

/** Sends the person to a Stripe page, or keeps why it could not be opened. */
export function useLeave() {
  const [error, setError] = useState('');
  const leaveFor = useCallback(async (page: () => Promise<Redirect>) => {
    setError('');
    setError(await goTo(page));
  }, []);
  return { error, leaveFor };
}

/** Everything the plan section shows and does. */
export function usePlan(open: boolean) {
  const { token } = useAuth();
  const billing = useBilling(open, token);
  const { error, leaveFor } = useLeave();
  const upgrade = (plan: string) => (token ? leaveFor(() => billingCheckout(token, plan)) : undefined);
  const manage = () => (token ? leaveFor(() => billingPortal(token)) : undefined);
  return { billing: billing?.enabled ? billing : null, error, upgrade, manage };
}
