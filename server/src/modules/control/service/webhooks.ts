/**
 * Where a project's events go: its one customer webhook, its Slack sink, and
 * events recorded from outside the control plane's state machine.
 */
import { randomBytes } from 'node:crypto';
import { sealText } from '../../../platform/secrets.ts';
import { Status } from '../../../platform/http-status.ts';
import { fault, projectId } from './model.ts';
import { ensure } from './projects.ts';
import { RECENT_DELIVERIES, TOKEN_BYTES } from './constants.ts';

/** What the Settings endpoint offers to subscribe to; an empty selection means all. */
export const WEBHOOK_EVENTS = [
  'session.ready',
  'session.stopped',
  'session.failed',
  'session.disconnected',
  'run.needs_attention',
  'run.failed',
  'budget.threshold',
  'persona.created',
  'persona.updated',
  'persona.deleted',
  'recording.ready',
  'login.completed',
  'login.failed',
  'mfa.completed',
  'credential.created',
  'credential.revoked',
];
/** What a Slack sink subscribes to when it is created. */
export const SLACK_EVENTS = ['run.needs_attention', 'run.failed', 'session.failed'];

/** The stored hook: edits keep what they do not change, and the secret is sealed. */
function hookRow(existing, id, project, url, types, secret) {
  const sealed = secret ? sealText(`webhook:${id}`, secret) : existing.secret;
  return { ...existing, id, project, url, types, enabled: true, secret: sealed };
}

/** The save transaction: mint a secret on first save or `roll`, store the hook and announce it. */
async function saveWebhook(tx, key, { url, types, roll }) {
  const p = await ensure(tx, key);
  const id = `hook:${p.id}`,
    existing = await tx.get('webhook', id);
  const secret = !existing?.secret || roll ? randomBytes(TOKEN_BYTES).toString('base64url') : null;
  tx.put('webhook', id, hookRow(existing, id, p.id, url, types, secret));
  tx.emit(p.id, existing ? 'webhook.updated' : 'webhook.created', null, { id });
  return { id, url, types, enabled: true, ...(secret ? { secret } : {}) };
}

/**
 * The project's one customer endpoint, Stripe-style: a derived id so saving again
 * edits it rather than stacking a second hook. The secret survives edits and is
 * returned only when minted — on first save or on `roll`.
 */
export async function webhook(store, key, { url, types = [], roll = false }) {
  if (!Array.isArray(types) || types.some((x) => typeof x !== 'string'))
    throw fault('invalid_events', 'Event types must be an array of strings', Status.BAD_REQUEST);
  return store.transact((tx) => saveWebhook(tx, key, { url, types, roll }));
}

/** The hook's latest deliveries, newest first. */
async function recentDeliveries(store, key, id) {
  return (await store.list('delivery', { project: projectId(key) }))
    .filter((d) => d.hook === id)
    .sort((a, b) => b.at - a.at)
    .slice(0, RECENT_DELIVERIES);
}

/** Deliveries as the settings page lists them, each with its event's type. */
async function deliveryView(store, deliveries) {
  const events = new Map((await store.events({ seqs: deliveries.map((d) => d.eventSeq) })).map((e) => [e.id, e.type]));
  return deliveries.map(({ id, state, attempts, at, eventSeq }) => ({
    id,
    state,
    attempts,
    at,
    type: events.get(eventSeq) ?? null,
  }));
}

/** The project's endpoint without its secret, and its latest deliveries; null hook when never set. */
export async function webhookConfig(store, key) {
  const id = `hook:${projectId(key)}`;
  const hook = await store.get('webhook', id);
  const deliveries = hook ? await recentDeliveries(store, key, id) : [];
  return {
    hook: hook && { id, url: hook.url, types: hook.types, enabled: hook.enabled },
    events: WEBHOOK_EVENTS,
    deliveries: await deliveryView(store, deliveries),
  };
}

/** The stored sink: defaults, then what it had, then only what the caller passed. */
function slackRow(existing, patch, id, project) {
  const defaults = { channel: null, types: SLACK_EVENTS, enabled: true };
  return { ...defaults, ...existing, ...patch, id, project, kind: 'slack', url: null };
}

/** The Slack sink transaction. */
async function saveSlackSink(tx, key, patch) {
  const p = await ensure(tx, key);
  const id = `slack:${p.id}`;
  const existing = await tx.get('webhook', id);
  // Only what the caller passed changes: the worker disables a dead install
  // without knowing which channel it was pointed at.
  const hook = tx.put('webhook', id, slackRow(existing, patch, id, p.id));
  tx.emit(p.id, existing ? 'webhook.updated' : 'webhook.created', null, { id, kind: 'slack' });
  return { id, channel: hook.channel, types: hook.types, enabled: hook.enabled };
}

/**
 * The project's Slack sink, as one webhook row with a derived id, so connecting
 * twice replaces the sink instead of stacking duplicates. It carries no secret:
 * the bot token lives in the key's sealed settings and is resolved at send time,
 * which is also what lets one install serve both the OAuth and pasted-token paths.
 */
export async function slackSink(store, key, patch = {}) {
  return store.transact((tx) => saveSlackSink(tx, key, patch));
}

/**
 * Record an event for a project that already exists. Used by paths outside the
 * control plane's own state machine — SDK runs — so their failures and handover
 * requests reach the same delivery pipeline as session events.
 */
export async function emit(store, key, type, sessionId = null, detail = {}) {
  await store.transact(async (tx) => {
    tx.emit(projectId(key), type, sessionId, detail);
  });
}
