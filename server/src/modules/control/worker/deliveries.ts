/**
 * Webhook and Slack delivery: claim the due deliveries under a lease, send
 * them, and record the outcome with backoff.
 */
import { randomUUID } from 'node:crypto';
import { postSlack } from './slack-sink.ts';
import { sendWebhook, signedEvent } from './webhook.ts';
import { backoff } from './backoff.ts';
import {
  DAY_MS,
  DELIVERY_BACKOFF_CAP_MS,
  DELIVERY_BACKOFF_MAX_EXPONENT,
  DELIVERY_BATCH,
  DELIVERY_LEASE_MS,
} from './constants.ts';

/** Claims up to eight due deliveries and sends them; returns how many deliveries were pending. */
export async function deliver(service, sender = sendWebhook) {
  const claim = randomUUID(),
    now = Date.now();
  const { batch, pending } = await service.store.transact((tx) => claimDue(tx, claim, now));
  const events = new Map((await service.store.events({ seqs: batch.map((d) => d.eventSeq) })).map((e) => [e.id, e]));
  await Promise.all(batch.map((d) => deliverOne(service, d, events.get(d.eventSeq), claim, sender)));
  return pending;
}

/** Leases the due deliveries to this claim; ones whose hook is off or that are over a day old are closed instead. */
async function claimDue(tx, claim, now) {
  const pending = await tx.list('delivery', { states: ['pending'] });
  const due = pending.filter((d) => d.nextAt <= now && !(d.leaseUntil > now)).slice(0, DELIVERY_BATCH);
  const hooks = await hooksFor(tx, due);
  const batch = due.map((d) => claimOne(d, hooks.get(d.hook), claim, now)).filter(Boolean);
  return { batch, pending: pending.length };
}

/** The hooks the deliveries go to, by id. */
async function hooksFor(tx, due) {
  const hooks = await tx.getMany(
    'webhook',
    due.map((d) => d.hook),
  );
  return new Map(hooks.map((h) => [h.id, h]));
}

/** Leases one delivery and returns it with its hook, or closes it and returns null. */
function claimOne(d, hook, claim, now) {
  const refused = refusal(d, hook, now);
  if (refused) {
    d.state = refused;
    return null;
  }
  Object.assign(d, { claim, leaseUntil: now + DELIVERY_LEASE_MS, attempts: d.attempts + 1 });
  return { ...d, hook };
}

/** Why a due delivery will not be sent: its hook is off, or it has been retrying for a day. */
function refusal(d, hook, now) {
  if (!hook?.enabled) return 'cancelled';
  if (now - (d.replayAt || d.at) > DAY_MS) return 'failed';
  return null;
}

/** Sends one delivery and records the outcome. */
async function deliverOne(service, d, event, claim, sender) {
  const outcome = await attempt(service, d, event, sender);
  await service.store.transact((tx) => settle(tx, d, claim, outcome));
}

/** Sends the event to the hook's sink: `ok` when it was accepted, `dead` when the sink is gone for good. */
async function attempt(service, d, event, sender) {
  try {
    if (!event) throw new Error('Event is past retention');
    if (d.hook.kind === 'slack') return await postSlack(service, d.hook, event);
    const { body, headers } = signedEvent(d.hook, event);
    return { ok: await sender(d.hook.url, body, headers), dead: false };
  } catch {
    /* delivery state retains the retry obligation */
    return { ok: false, dead: false };
  }
}

/** Records the outcome, unless another claim has taken the delivery since. */
async function settle(tx, d, claim, { ok, dead }) {
  const current = await tx.get('delivery', d.id);
  if (current?.claim !== claim) return;
  // A revoked token or a deleted channel is not a transient failure: retrying it
  // every backoff step for a day would bury the queue behind an install that is gone.
  if (dead) return cancelDead(tx, current, d);
  Object.assign(current, {
    state: ok ? 'delivered' : 'pending',
    leaseUntil: null,
    nextAt: Date.now() + backoff(d.attempts, DELIVERY_BACKOFF_MAX_EXPONENT, DELIVERY_BACKOFF_CAP_MS),
  });
}

/** Cancels the delivery and disables its hook. */
async function cancelDead(tx, current, d) {
  Object.assign(current, { state: 'cancelled', leaseUntil: null });
  const hook = await tx.get('webhook', d.hook.id);
  if (hook) hook.enabled = false;
}
