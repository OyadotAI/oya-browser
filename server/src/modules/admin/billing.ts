/** Validated, attributed admin billing changes, isolated from Stripe payments. */
import { HttpError, invalid, notFound } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { SECONDS_PER_HOUR } from '../../platform/constants.ts';
import * as billing from '../billing/index.ts';
import { profileById } from './repository.ts';
import {
  BILLING_REASON_MAX,
  MICRO_USD_PER_DOLLAR,
  MAX_GRANT_HOURS,
  MAX_GRANT_DOLLARS,
  MAX_GRANT_STEPS,
} from './constants.ts';

/** A support adjustment always records why it was made. */
function reasonOf(value: unknown) {
  if (typeof value !== 'string' || !value.trim() || value.trim().length > BILLING_REASON_MAX)
    throw invalid('reason', `1–${BILLING_REASON_MAX} characters`, value);
  return value.trim();
}

/** Rejects unknown people before creating any billing state. */
async function requireProfile(id: string) {
  if (!(await profileById(id))) throw notFound('Account');
}

/** Sets or removes complimentary plan access without changing the paid subscription. */
export async function setPlan(id: string, body: Record<string, unknown>, actor: string) {
  if (body.plan !== null && !billing.isPlan(body.plan))
    throw invalid('plan', 'free, developer, startup or null', body.plan);
  const reason = reasonOf(body.reason);
  await requireProfile(id);
  await billing.saveOverride(id, body.plan as string | null, actor, reason);
  return verifiedPlan(id, body.plan);
}

/** Confirms persistence before reporting that access changed. */
async function verifiedPlan(id: string, plan: unknown) {
  const saved = await billing.overrideFor(id);
  if (!saved || saved.plan !== plan)
    throw new HttpError(Status.CONFLICT, 'Plan access could not be verified. Refresh the account and try again.');
  return { plan: saved.plan, reason: saved.reason };
}

/** Converts a bounded input to exact integer storage units. */
function quantity(value: unknown, field: string, max: number, scale: number) {
  if (value === undefined) return 0;
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > max)
    throw invalid(field, `a number from 0 to ${max}`, value);
  return Math.round(value * scale);
}

/** A client-generated UUID is a stable identity for one grant across retries. */
function requestId(value: unknown) {
  if (
    typeof value !== 'string' ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value)
  )
    throw invalid('requestId', 'a UUID v4', value);
  return value.toLowerCase();
}

/** Validates the entire grant before storage or period lookup. */
function grantFields(body: Record<string, unknown>) {
  const id = requestId(body.requestId);
  const reason = reasonOf(body.reason);
  const cloud_seconds = quantity(body.hours, 'hours', MAX_GRANT_HOURS, SECONDS_PER_HOUR);
  const hosted_llm_microusd = quantity(body.credits, 'credits', MAX_GRANT_DOLLARS, MICRO_USD_PER_DOLLAR);
  const agent_steps = stepsOf(body.steps);
  if (!cloud_seconds && !hosted_llm_microusd && !agent_steps)
    throw invalid('grant', 'positive hours, credits or steps', body);
  return { id, reason, cloud_seconds, hosted_llm_microusd, agent_steps };
}

/** Steps are whole actions; fractional grants would misrepresent the allowance. */
function stepsOf(value: unknown) {
  if (value !== undefined && !Number.isInteger(value)) throw invalid('steps', 'a whole number', value);
  return quantity(value, 'steps', MAX_GRANT_STEPS, 1);
}

/** Rejects reuse of a request id for another person, amount, reason or administrator. */
function sameGrant(stored: Record<string, any>, asked: Record<string, any>) {
  if (Object.keys(asked).some((key) => (key === 'agent_steps' ? (stored[key] ?? 0) : stored[key]) !== asked[key]))
    throw invalid('requestId', 'a new UUID for a different grant', asked.id);
  return stored;
}

/** Adds a grant once; duplicate requests cannot double the allowance, including across replicas. */
export async function grant(id: string, body: Record<string, unknown>, actor: string, now = Date.now()) {
  const fields = { ...grantFields(body), user_id: id, actor };
  await requireProfile(id);
  const previous = await billing.grantById(fields.id);
  if (previous) return sameGrant(previous, fields);
  const { since } = billing.standingOf(id, await billing.findSubscription(id), now);
  await billing.saveGrant({ ...fields, period_start: since, created_at: new Date(now).toISOString() });
  return sameGrant((await billing.grantById(fields.id))!, fields);
}
