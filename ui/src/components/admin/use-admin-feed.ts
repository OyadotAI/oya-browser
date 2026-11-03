/** Each staff feed owns credential-scoped data, retry state and protection against stale responses. */
import { useEffect, useState, type Dispatch, type SetStateAction } from 'react';
import { Status } from '@/lib/http-status';
import { stepUpUrl } from '@/lib/auth/step-up';

/** User-visible data and its most recent request status. */
interface View<T> {
  /** Last successful value, withheld on access refusal. */ value: T | null;
  /** Latest request error. */ error: string;
  /** Machine-readable recovery reason. */ code: string;
  /** A credential or staff permission refusal invalidates the whole admin page. */ denied: boolean;
}

/** Internal request identity is never rendered or persisted. */
interface Saved<T> extends View<T> {
  /** Credential owning the response. */ token: string;
  /** Refresh generation owning the response. */ generation: number;
}

/** One mounted effect owns the right to apply its success or failure. */
interface Request<T> {
  /** Cleanup revokes this capability. */ current: boolean;
  /** The request's in-memory credential. */ token: string;
  /** The requested refresh generation. */ generation: number;
  /** Component state setter. */ keep: Dispatch<SetStateAction<Saved<T> | null>>;
}

/** Missing data is not a measured zero, and stale data never belongs to a replacement token. */
const empty = <T>(): View<T> => ({ value: null, error: '', code: '', denied: false });

/** Authentication failures apply globally even when the server supplied no machine-readable code. */
function refusedAccess(error: unknown) {
  const { code, status } = Object(error);
  return (
    ['forbidden', 'unauthorized', 'mfa_required'].includes(code) ||
    [Status.UNAUTHORIZED, Status.FORBIDDEN].includes(status) ||
    (error instanceof Error && error.message === 'Admins only')
  );
}

/** Last-good data survives ordinary refresh failures, but never an access refusal. */
function failure<T>(old: Saved<T> | null, request: Request<T>, error: unknown): Saved<T> {
  const denied = refusedAccess(error);
  return {
    ...identity(request),
    denied,
    code: Object(error).code ?? '',
    value: !denied && old?.token === request.token ? old.value : null,
    error: error instanceof Error ? error.message : 'Could not load this section.',
  };
}

/** Cancelled successes cannot replace another credential's protected data. */
function received<T>(request: Request<T>, value: T) {
  if (request.current) request.keep({ ...empty<T>(), token: request.token, generation: request.generation, value });
}

/** Cancelled failures cannot clear newer data or redirect another session to MFA. */
function refused<T>(request: Request<T>, error: unknown) {
  if (!request.current) return;
  if (Object(error).code === 'mfa_required') window.location.assign(stepUpUrl('/admin'));
  request.keep((old) => failure(old, request, error));
}

/** Request identity is copied without exposing setters or mutable ownership. */
const identity = <T>(request: Request<T>) => ({ token: request.token, generation: request.generation });

/** Each effect creates an explicitly revocable request owner. */
function subscribe<T>(request: Request<T>, source: (token: string) => Promise<T>) {
  void source(request.token).then(
    (value) => received(request, value),
    (error) => refused(request, error),
  );
  return () => {
    request.current = false;
  };
}

/** A token change withholds the previous snapshot immediately, before effects run. */
function visible<T>(saved: Saved<T> | null, token: string | null, generation: number) {
  const own = saved?.token === token ? saved : null;
  return { ...payload(own), loading: own?.generation !== generation };
}

/** Credentials and refresh generations never escape the internal request state. */
function payload<T>(saved: Saved<T> | null): View<T> {
  const { value, error, code, denied } = saved ?? empty<T>();
  return { value, error, code, denied };
}

/** Shared behavior keeps overview, finance, licenses and Keel independent without duplicating races. */
export function useAdminFeed<T>(token: string | null, source: (token: string) => Promise<T>) {
  const [saved, keep] = useState<Saved<T> | null>(null);
  const [generation, refresh] = useState(0);
  useEffect(
    () => (token ? subscribe({ token, generation, keep, current: true }, source) : undefined),
    [token, generation, source],
  );
  return { ...visible(saved, token, generation), refresh: () => refresh((value) => value + 1) };
}
