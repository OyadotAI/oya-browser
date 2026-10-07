/** Keeps mutation errors local and preserves grant identity across failed retries. */
import { useRef, useState, type MutableRefObject } from 'react';
import type { AdminState } from './use-admin';
import type { Found } from './types';

/** Fields shared by the two explicit billing actions. */
type Fields = {
  /** Override selection; empty means follow the subscription. */ plan: string;
  /** Cloud hours typed. */ hours: string;
  /** Hosted model dollars typed. */ credits: string;
  /** Support explanation. */ reason: string;
};

/** Visible mutation feedback. */
type Outcome = {
  /** Disables controls during a request. */ busy: boolean;
  /** Success feedback. */ message: string;
  /** Failure feedback. */ error: string;
};

/** Identity retained until a grant succeeds. */
type Identity = { /** Serialized payload. */ body: string; /** Retry-safe request id. */ id: string };

/** Reports a saved change and refreshes the selected customer's data. */
function succeeded(done: () => void, set: (value: Outcome) => void) {
  done();
  set({ busy: false, message: 'Saved. Customer allowances updated.', error: '' });
}

/** Runs one mutation and reports its result, always releasing the submit guard. */
async function perform(work: () => Promise<unknown>, done: () => void, set: (value: Outcome) => void) {
  set({ busy: true, message: '', error: '' });
  await Promise.resolve()
    .then(work)
    .then(
      () => succeeded(done, set),
      (e) => set({ busy: false, message: '', error: e instanceof Error ? e.message : 'Could not save' }),
    );
}

/** Editable fields and the successful-grant reset. */
function useFields(p: Found) {
  const [fields, setFields] = useState<Fields>({ plan: p.override?.plan || '', hours: '', credits: '', reason: '' });
  const set = (patch: Partial<Fields>) => setFields((f) => ({ ...f, ...patch }));
  return { fields, set };
}

/** A successful action releases its retry identity and empties the grant fields. */
function cleared(
  previous: MutableRefObject<Identity | null>,
  set: (fields: Partial<Fields>) => void,
  changed: () => void,
) {
  previous.current = null;
  set({ hours: '', credits: '', reason: '' });
  changed();
}

/** Stable identity for the same grant payload; editing fields creates a new request. */
function grantRequest(fields: Fields, previous: MutableRefObject<Identity | null>) {
  const body = JSON.stringify({ hours: Number(fields.hours), credits: Number(fields.credits), reason: fields.reason });
  if (previous.current?.body !== body) previous.current = { body, id: crypto.randomUUID() };
  return { ...JSON.parse(body), requestId: previous.current.id };
}

/** Fields, pending state and separate plan/grant submissions for one customer. */
export function useBillingAdjustment(p: Found, s: AdminState, changed: () => void) {
  const { fields, set } = useFields(p);
  const [outcome, setOutcome] = useState<Outcome>({ busy: false, message: '', error: '' });
  const previous = useRef<Identity | null>(null);
  const locked = useRef(false);
  const done = () => cleared(previous, set, changed);
  const submit = (kind: 'plan' | 'grant') =>
    submitAdjustment({ kind, p, s, fields, previous, locked, done, setOutcome });
  return { fields, set, submit, ...outcome };
}

/** Serializes clicks synchronously, before React has painted the pending state. */
async function submitAdjustment({ kind, p, s, fields, previous, locked, done, setOutcome }: Submit) {
  if (locked.current) return;
  locked.current = true;
  const work = () =>
    kind === 'plan'
      ? s.setPlan(p.profile.id, { plan: fields.plan || null, reason: fields.reason })
      : s.grant(p.profile.id, grantRequest(fields, previous));
  await perform(work, done, setOutcome);
  locked.current = false;
}

/** The submission context, retained for retries and safe customer switching. */
type Submit = {
  /** Which action was selected. */ kind: 'plan' | 'grant';
  /** Target account. */ p: Found;
  /** Authenticated mutations. */ s: AdminState;
  /** Values to save. */ fields: Fields;
  /** Last grant identity. */ previous: MutableRefObject<Identity | null>;
  /** Synchronous click guard. */ locked: MutableRefObject<boolean>;
  /** Clears inputs and refreshes the customer. */ done: () => void;
  /** Updates feedback. */ setOutcome: (value: Outcome) => void;
};
