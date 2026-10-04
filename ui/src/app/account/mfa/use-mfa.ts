/**
 * The two-factor page's state: the person's authenticator apps, an app being
 * added, and the code they type. Checking a code gives the session its second
 * factor (the server issues new tokens and sets the refresh cookie), so the
 * page then reloads into `next`, where the restored session is the new one.
 */
'use client';

import { useEffect, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import {
  mfaEnroll,
  mfaStatus,
  mfaUnenroll,
  mfaVerify,
  type MfaEnrolment,
  type MfaStatus,
  type StepUpAnswer,
} from '@/lib/api';
import { errorMessage } from '@/lib/api-client';
import { keepRefreshToken, keepStepUp } from '@/lib/auth/storage';
import { MFA_PAGE, safeNext } from '@/lib/auth/step-up';

/** What the page shows: loading, a code for an app already set up, setting one up, or the list of apps. */
export type MfaMode = 'loading' | 'code' | 'setup' | 'manage';

/**
 * The view for what is known. An app being added wins; with no working app the
 * person sets one up; with one, a session that has not passed it asks for its
 * code, and one that has shows the list.
 */
export function mfaMode(status: MfaStatus | null, enrolment: MfaEnrolment | null): MfaMode {
  if (enrolment) return 'setup';
  if (!status) return 'loading';
  if (!status.factors.some((f) => f.status === 'verified')) return 'setup';
  return status.aal === 'aal2' ? 'manage' : 'code';
}

/** The person's apps and session level, reloaded when `version` moves on. */
function useStatus(token: string | null) {
  const [status, setStatus] = useState<MfaStatus | null>(null);
  const [error, setError] = useState('');
  const [version, setVersion] = useState(0);
  useEffect(() => {
    if (token) void mfaStatus(token).then(setStatus, (e) => setError(errorMessage(e)));
  }, [token, version]);
  return { status, error, setError, reload: () => setVersion((v) => v + 1) };
}

/** Where to go once the code is accepted: the page that sent the person here, else back to this one. */
const nextPage = () => safeNext(new URLSearchParams(window.location.search).get('next')) || MFA_PAGE;

/** Adopts the two-factor session the server just issued and reloads into the next page. */
function stepUpDone(answer: StepUpAnswer) {
  keepRefreshToken(answer.refresh_token);
  keepStepUp(false);
  window.location.replace(nextPage());
}

/** Runs one request with the busy flag up, showing its failure as the error. */
function useRunner(setError: (e: string) => void) {
  const [busy, setBusy] = useState(false);
  const run = async (work: () => Promise<unknown>) => {
    setBusy(true);
    setError('');
    await work().catch((e) => setError(errorMessage(e)));
    setBusy(false);
  };
  return { busy, run };
}

/** The app being added and the typed code. */
function useEntry() {
  const [enrolment, setEnrolment] = useState<MfaEnrolment | null>(null);
  const [code, setCode] = useState('');
  return { enrolment, setEnrolment, code, setCode };
}

/** The factor a code is checked against: the app being added, else the first working one. */
const factorFor = (status: MfaStatus | null, enrolment: MfaEnrolment | null) =>
  enrolment?.id || status?.factors.find((f) => f.status === 'verified')?.id || '';

/** Adding an app, checking a code and removing an app, each a request with the busy flag up. */
function useMfaActions(token: string, s: ReturnType<typeof useStatus>, e: ReturnType<typeof useEntry>) {
  const { busy, run } = useRunner(s.setError);
  const start = () => run(async () => e.setEnrolment(await mfaEnroll(token)));
  const factorId = factorFor(s.status, e.enrolment);
  const verify = () => run(async () => stepUpDone(await mfaVerify(token, factorId, e.code.trim())));
  const remove = (id: string) => run(async () => (await mfaUnenroll(token, id), s.reload()));
  return { busy, start, verify, remove };
}

/** Everything the two-factor page shows and does. */
export function useMfa() {
  const { token, loading } = useAuth();
  const s = useStatus(token);
  const e = useEntry();
  const actions = useMfaActions(token || '', s, e);
  return { signedOut: !loading && !token, mode: mfaMode(s.status, e.enrolment), ...s, ...e, ...actions };
}

/** The two-factor page's state, as its views take it. */
export type MfaState = ReturnType<typeof useMfa>;
