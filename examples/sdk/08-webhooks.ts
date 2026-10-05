// Webhooks: a signed POST for every run event, so a run that stalls or fails reaches you even when no script is watching.
import { createHmac, timingSafeEqual } from 'node:crypto';
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
// WEBHOOK_URL is an https endpoint you run. Omit the list to receive every event in `details` below.
const hook = await oya.control.createWebhook(process.env.WEBHOOK_URL!, [
  'run.started',
  'run.needs_attention',
  'run.resumed',
  'run.completed',
  'run.failed',
]);
console.log(hook.secret ? `signing secret: ${hook.secret}` : 'updated; the secret from the first run still signs'); // returned once: keep it

// In your endpoint: Oya-Signature is "t=<seconds>,v1=<hex>", an HMAC-SHA256 of "<seconds>.<raw body>".
export const verify = (rawBody: string, signature: string, secret: string) => {
  const { t, v1 } = Object.fromEntries(signature.split(',').map((part) => part.split('=')));
  const expected = createHmac('sha256', secret).update(`${t}.${rawBody}`).digest('hex');
  return v1?.length === expected.length && timingSafeEqual(Buffer.from(v1), Buffer.from(expected));
};

// What arrives: one JSON POST per event. Delivery is at least once, retried for 24 hours,
// so deduplicate on `id` (also sent as the Oya-Event-Id header).
export const envelope = {
  id: 4812,
  project: 'prj_3f9a…',
  type: 'run.failed',
  sessionId: '6a98cd76-739e-435d-9851-6c1201c9e26a', // the browser it is about, or null
  at: 1759100000000, // milliseconds
  detail: { runId: 'run_…', owner: '08b1716f1cf2aeda', error: 'Timed out' },
};

// Only `detail` changes with `type`. A sample of each:
export const details = {
  // A browser's lifecycle, in order. Only a stop or a failure the control plane caused carries a `reason`.
  'session.queued': {},
  'session.provisioning': {},
  'session.ready': {},
  'session.disconnected': {},
  'session.stopping': {},
  'session.cleanup_pending': { reason: 'cancelled' }, // 'cancelled' (stopped) | 'budget' (budget spent) | 'project_deleted'; {} when its own lifetime ended
  'session.unknown_outcome': {},
  'session.stopped': {}, // { reason: 'cancelled' | 'reconciled' } when stopped before it started
  'session.failed': { reason: 'queue_timeout' }, // or 'Managed runtime required by current project settings'; {} when the browser failed

  // Who is driving the browser.
  'control.agent': {}, // the agent drives again
  'control.human': {}, // a person took over in the live view
  'control.paused': {},

  // Runs from the SDK (browser.submit, browser.play). `owner` identifies the API key that started it.
  'run.started': { runId: 'run_…', owner: '08b1716f1cf2aeda' },
  'run.needs_attention': {
    runId: 'run_…',
    owner: '08b1716f1cf2aeda',
    reason: 'mfa', // 'captcha' | 'mfa' | 'login' | 'agent' | 'heal_failed'
    message: 'The code was entered, but the site has not confirmed it. Open the live view to finish.',
  },
  'run.resumed': { runId: 'run_…', owner: '08b1716f1cf2aeda' }, // a person answered
  'run.completed': { runId: 'run_…', owner: '08b1716f1cf2aeda' },
  'run.failed': { runId: 'run_…', owner: '08b1716f1cf2aeda', error: 'Timed out' },

  // The same five types from Ask in the dashboard or POST /chat: `source: "chat"` instead of `owner`,
  // and a person answers in the chat, so there is no run.resumed.
  'run.started (from Ask)': { runId: '0825ecdc-…', source: 'chat' },
  'run.needs_attention (from Ask)': {
    runId: '0825ecdc-…',
    source: 'chat',
    reason: 'agent',
    message: 'Which account?',
  },
  'run.completed (from Ask)': { runId: '0825ecdc-…', source: 'chat' },
  'run.failed (from Ask)': {
    runId: '0825ecdc-…',
    source: 'chat',
    error: 'Your AI provider refused the request (401).',
  },

  // Routines: saved prompts run on a schedule. A routine run wraps the chat run that does the work,
  // and its sessionId is the browser running it.
  'routine.created': { routineId: 'b3e0c1a4-…', name: 'Morning inbox check' },
  'routine.updated': { routineId: 'b3e0c1a4-…', fields: ['prompt', 'schedule'] }, // of name, prompt, schedule, enabled, target, tz
  'routine.deleted': { routineId: 'b3e0c1a4-…' },
  'routine.run.started': { routineId: 'b3e0c1a4-…', runId: '7d2f90b1-…' },
  'routine.run.finished': { routineId: 'b3e0c1a4-…', runId: '7d2f90b1-…', status: 'done' }, // 'done' | 'failed' | 'stopped' | 'interrupted'

  // A session recording is ready to watch; sessionId is the recorded browser.
  'recording.ready': {},

  // Sign-ins with the logins and MFA seeds stored on a persona.
  // login method: 'credentials' | 'username' | 'request_code' | 'handoff' (passed to a person) | 'none' | null
  'login.completed': { personaId: 'p-911e966b7a967be6', domain: 'app.example.com', method: 'credentials' },
  'login.failed': { personaId: 'p-911e966b7a967be6', domain: 'app.example.com', method: 'handoff' },
  // Sent only when the code was accepted. method: 'totp' | 'gmail' | 'graph' | 'email' | 'sms' | null
  'mfa.completed': { personaId: 'p-911e966b7a967be6', domain: 'app.example.com', method: 'totp' },

  // Personas: one stable device identity each.
  'persona.created': { personaId: 'p-911e966b7a967be6', name: 'us-shopper' },
  'persona.created (a clone)': {
    personaId: 'p-3cae0ed6a6a9eeec',
    name: 'us-shopper (copy)',
    clonedFrom: 'p-911e966b7a967be6',
  },
  'persona.updated': { personaId: 'p-911e966b7a967be6', fields: ['proxy'] }, // the fields the request changed
  'persona.deleted': { personaId: 'p-911e966b7a967be6' },

  // Service credentials.
  'credential.created': { id: '5f1d2c9e-…', role: 'operator' }, // 'viewer' | 'operator' | 'administrator'
  'credential.revoked': { id: '5f1d2c9e-…' },

  // Spend crossed 80% (0.8) or 100% (1) of the project budget.
  'budget.threshold': { threshold: 0.8, estimatedUsd: 40.12 },

  // The project and its members.
  'project.created': {},
  'project.renamed': {},
  'project.settings.updated': { fields: ['budgetUsd', 'maxConcurrent'] }, // the settings the request changed
  'project.deleted': {},
  'member.invited': { role: 'operator' }, // 'viewer' | 'operator' | 'administrator'
  'member.joined': { userId: '1c7e4b2a-…', role: 'operator' },
  'member.removed': { userId: '1c7e4b2a-…' },

  // Webhooks themselves: your endpoint, or the Slack sink with kind "slack".
  'webhook.created': { id: 'hook:prj_3f9a…' },
  'webhook.updated': { id: 'hook:prj_3f9a…' },
  'webhook.updated (Slack)': { id: 'slack:prj_3f9a…', kind: 'slack' }, // webhook.created has the same Slack form
  // Sent only by "Send test event" in Settings → Webhooks; you cannot subscribe to it.
  'webhook.test': { message: 'Test event from Oya. Your endpoint is reachable and can verify the signature.' },
};
