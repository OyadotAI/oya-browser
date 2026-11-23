/**
 * The self-hosted license section's state: the person's license and key, the
 * form to buy one, and the wait for the key after Checkout, which the Stripe
 * webhook issues a moment after the person returns.
 */
import { useCallback, useEffect, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import { getLicense, licenseCheckout, licensePortal } from '@/lib/api';
import { useLeave } from './use-plan';
import {
  COPIED_MS,
  LICENSE_ANCHOR,
  LICENSE_MIN,
  LICENSE_POLL_MS,
  LICENSE_SLOW_MS,
  licenseState,
  type License,
  type LicenseInterval,
} from './license-model';

/** What the server answers for the person's license. */
interface Loaded {
  /** False on a self-hosted server. */
  enabled: boolean;
  /** Their license, or null. */
  license: License | null;
}

/** Whether the person just came back from paying. */
const justPaid = () =>
  typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('license') === 'paid';

/** Loads the license, again whenever `reload` is called; a failure leaves the section out. */
function useLicense(token: string | null) {
  const [data, setData] = useState<Loaded | null>(null);
  const reload = useCallback(() => {
    if (token) getLicense(token).then(setData, () => setData({ enabled: false, license: null }));
  }, [token]);
  useEffect(reload, [reload]);
  return { data, reload };
}

/** After paying, looks for the key until it arrives; says when it is taking long. */
function useKeyWait(waiting: boolean, reload: () => void) {
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    if (!waiting) return;
    const poll = setInterval(reload, LICENSE_POLL_MS);
    const late = setTimeout(() => setSlow(true), LICENSE_SLOW_MS);
    return () => (clearInterval(poll), clearTimeout(late));
  }, [waiting, reload]);
  return slow;
}

/** Copies the key, and says so for a moment. */
function useCopy(key: string | undefined) {
  const [copied, setCopied] = useState(false);
  const copy = () =>
    void navigator.clipboard?.writeText(key || '').then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), COPIED_MS);
    });
  return { copied, copy };
}

/** Arriving on #self-hosted, brings the section's heading into view and focus once it has loaded. */
function useArrival(loaded: boolean) {
  useEffect(() => {
    if (!loaded || window.location.hash !== `#${LICENSE_ANCHOR}`) return;
    const heading = document.getElementById(`${LICENSE_ANCHOR}-title`);
    heading?.scrollIntoView?.();
    heading?.focus();
  }, [loaded]);
}

/** Runs a leave-for-Stripe action, saying it is busy until the page goes or it fails. */
function useBusy() {
  const [busy, setBusy] = useState(false);
  const run = async (action: () => Promise<void> | undefined) => {
    setBusy(true);
    await action();
    setBusy(false);
  };
  return { busy, run };
}

/** The buy form's fields: how many browsers, billed how often. */
function useForm() {
  const [browsers, setBrowsers] = useState(LICENSE_MIN);
  const [interval, setInterval] = useState<LicenseInterval>('month');
  return { browsers, setBrowsers, interval, setInterval };
}

/** The two ways out to Stripe: buy with the form's fields, or manage the license. */
function useStripeExits(token: string | null, form: ReturnType<typeof useForm>) {
  const { error, leaveFor } = useLeave();
  const { busy, run } = useBusy();
  const buy = () =>
    run(() => (token ? leaveFor(() => licenseCheckout(token, form.browsers, form.interval)) : undefined));
  const manage = () => (token ? leaveFor(() => licensePortal(token)) : undefined);
  return { error, busy, buy, manage };
}

/** Whether the person just paid and the new key has not arrived: no license yet, or only an ended one. */
const awaitingKey = (data: Loaded | null) =>
  Boolean(
    data?.enabled && justPaid() && (!data.license || ['cancelled', 'expired'].includes(licenseState(data.license))),
  );

/** Everything the self-hosted license section shows and does. */
export function useSelfHostedLicense() {
  const { token } = useAuth();
  const { data, reload } = useLicense(token);
  const form = useForm();
  const pending = awaitingKey(data);
  const slow = useKeyWait(pending, reload);
  useArrival(Boolean(data?.enabled));
  return { data, pending, slow, form, ...useStripeExits(token, form), ...useCopy(data?.license?.key) };
}

/** The section's state, as its parts take it. */
export type SelfHostedLicenseState = ReturnType<typeof useSelfHostedLicense>;
