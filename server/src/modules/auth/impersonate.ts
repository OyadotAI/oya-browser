/**
 * "Login as" tokens: a one-hour HS256 JWT that lets an admin act as one
 * customer to see what they see. It is signed with its own secret, never
 * Supabase's, so it can never pass as a sign-in token, every request made
 * with it is audited under the admin who asked for it, and every request
 * re-checks that the admin still is one.
 */
import { createHmac, timingSafeEqual } from 'node:crypto';
import { dbAuth } from '../../platform/db.ts';
import { HttpError, notFound } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MS_PER_SECOND } from '../../platform/constants.ts';
import { isAdmin } from './admins.ts';
import { IMPERSONATE_CONTEXT, IMPERSONATE_MIN_SECRET, IMPERSONATE_TTL_MS } from './constants.ts';

/** What a token says: who is acted as, by which admin, and until when. */
export type Impersonation = {
  /** The customer's user id. */
  sub: string;
  /** The admin's user id. */
  impersonated_by: string;
  /** Marks this as a "Login as" token, so no other token shape can pass. */
  imp: true;
  /** Issued at, in seconds. */
  iat: number;
  /** Expires at, in seconds. */
  exp: number;
};

/** A value as unpadded base64url JSON, the way a JWT carries its parts. */
const encode = (value: object) => Buffer.from(JSON.stringify(value)).toString('base64url');

/** The fixed JWT header every token starts with. */
const HEADER = encode({ alg: 'HS256', typ: 'JWT' });

/** The signing secret: OYA_IMPERSONATE_SECRET, else one derived from the Supabase service key. */
function secret(env = process.env): string {
  const service = env.SUPABASE_SERVICE_KEY;
  const derived = service ? createHmac('sha256', service).update(IMPERSONATE_CONTEXT).digest('hex') : '';
  const key = env.OYA_IMPERSONATE_SECRET || derived;
  if (key.length >= IMPERSONATE_MIN_SECRET) return key;
  throw new HttpError(Status.UNAVAILABLE, 'Login as is not configured: set OYA_IMPERSONATE_SECRET');
}

/** The signature over a token's header and payload. */
const sign = (body: string) => createHmac('sha256', secret()).update(body).digest('base64url');

/** A token for an admin to act as a customer, good for an hour from now. */
export function mintImpersonation(sub: string, admin: string, now = Date.now()): string {
  const iat = Math.floor(now / MS_PER_SECOND);
  const exp = iat + IMPERSONATE_TTL_MS / MS_PER_SECOND;
  const body = `${HEADER}.${encode({ sub, impersonated_by: admin, imp: true, iat, exp })}`;
  return `${body}.${sign(body)}`;
}

/** Whether a signature is the one this server would make, compared in constant time. */
function signedHere(body: string, signature: string) {
  const want = Buffer.from(sign(body));
  const got = Buffer.from(signature);
  return want.length === got.length && timingSafeEqual(want, got);
}

/** A payload's claims, or null when it is not JSON. */
function claimsOf(payload: string): Partial<Impersonation> | null {
  try {
    return JSON.parse(Buffer.from(payload, 'base64url').toString());
  } catch {
    return null;
  }
}

/** A token's claims when it is genuine, a "Login as" token and unexpired; null otherwise. */
export function readImpersonation(token: string, now = Date.now()): Impersonation | null {
  const [head, payload, signature, extra] = token.split('.');
  if (extra !== undefined || head !== HEADER || !signature || !signedHere(`${head}.${payload}`, signature)) return null;
  const claims = claimsOf(payload);
  const live = claims?.imp === true && !!claims.sub && !!claims.impersonated_by && claims.exp * MS_PER_SECOND > now;
  return live ? (claims as Impersonation) : null;
}

/** The Supabase user a token acts as, or 404 when they are gone. */
export async function impersonatedUser(id: string) {
  if (!dbAuth) throw new HttpError(Status.UNAVAILABLE, 'Auth not configured');
  const { data, error } = await dbAuth.auth.admin.getUserById(id);
  if (error || !data?.user) throw notFound('User');
  return data.user;
}

/**
 * The customer a "Login as" token acts as, checked on every request rather
 * than only when it was issued: an admin who has since lost admin rights (or
 * been deleted), or a customer who has since become an admin, ends the token
 * at once with a 401. `find` is the user lookup, a seam for tests.
 */
export async function actingAs(claims: Impersonation, find = impersonatedUser) {
  const stillAdmin = find(claims.impersonated_by).then(isAdmin, () => false);
  const [user, admin] = await Promise.all([find(claims.sub), stillAdmin]);
  if (!admin || isAdmin(user)) throw new HttpError(Status.UNAUTHORIZED, 'Login as is no longer allowed');
  return user;
}
