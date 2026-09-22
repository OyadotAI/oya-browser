/**
 * `oya.control`: the durable control plane. Sessions (including disconnected
 * and cleanup-pending ones), human takeover, lifecycle events, service
 * credentials, members and webhooks. Built from one group per topic.
 */
import { segment } from '../client.js';
import { refusal } from '../errors.js';
import type {
  ControlOverview,
  ControlRole,
  ControlSession,
  HumanInputAction,
  ProjectSettings,
  RecoverOptions,
  StopResult,
} from '../types/index.js';
import type {
  Ack,
  CredentialRequest,
  EventPage,
  HttpRef,
  Invite,
  MemberList,
  NewCredential,
  Ticket,
  Webhook,
} from './shapes.js';

/** A session's endpoint. */
const session = (id: string, action = '') => `/api/control/sessions/${segment(id)}${action}`;
/** An endpoint for one item in a control collection. */
const item = (kind: string, id: string) => `/api/control/${kind}/${segment(id)}`;

/** Reading sessions and project settings. */
const sessionCalls = (http: HttpRef) => ({
  /** Settings, sessions and recent events at a glance. */
  overview: async (): Promise<ControlOverview> => http().request('GET', '/api/control'),
  /** Every session, including disconnected and cleanup-pending ones. */
  sessions: async (): Promise<ControlSession[]> => http().request('GET', '/api/control/sessions'),
  /** One session. */
  session: async (id: string): Promise<ControlSession> => http().request('GET', session(id)),
  /** Update limits, rate cards and retention. */
  settings: async (changes: Partial<ProjectSettings>): Promise<ControlOverview['project']> =>
    http().request('PATCH', '/api/control/project', changes),
});

/** Ending, steering and recovering sessions. */
const lifecycleCalls = (http: HttpRef) => ({
  /** Stop a session in any state; answers its final state. */
  cancel: async (id: string): Promise<ControlSession> => http().request('POST', session(id, '/cancel'), {}),
  /** Stop a session; `force` stops despite a profile-save error, or reconciles. */
  stop: async (id: string, force = false): Promise<StopResult> =>
    http().request('POST', session(id, '/stop'), { force }),
  /** Acquire or release human control, or acknowledge the agent's resume. */
  takeover: async (id: string, action: 'acquire' | 'release' | 'resume'): Promise<ControlSession['control']> =>
    http().request('POST', session(id, '/control'), { action }),
  /** Send one input as the human holding the control lease. */
  input: async (id: string, action: HumanInputAction, params: Record<string, unknown>): Promise<unknown> =>
    http().request('POST', session(id, '/input'), { action, params }),
});

/** The recover request body; a wsUrl without replace is refused, since recovering in place keeps its endpoint. */
function recoverBody(options: boolean | RecoverOptions | null) {
  const { replace = false, wsUrl } = typeof options === 'boolean' ? { replace: options } : (options ?? {});
  if (wsUrl && !replace)
    throw refusal('wsUrl is only used with replace: true; recovering in place keeps the endpoint it had.', 'wsUrl');
  return wsUrl ? { replace, wsUrl } : { replace };
}

/** Recovery, live-stream tickets and the event log. */
const recoveryCalls = (http: HttpRef) => ({
  /**
   * Explicitly recover a session, or replace it with a fresh one. A cdp
   * session's replacement needs the Chrome it runs on: `{ replace: true, wsUrl }`.
   * `recover(id, true)` still means replace.
   */
  recover: async (id: string, options: boolean | RecoverOptions = {}): Promise<unknown> =>
    http().request('POST', session(id, '/recover'), recoverBody(options)),
  /** A single-use ticket for the live stream. */
  ticket: async (id: string): Promise<Ticket> => http().request('POST', session(id, '/ticket'), {}),
  /** Durable lifecycle events after a cursor. */
  events: async (after = 0): Promise<EventPage> => http().request('GET', `/api/control/events?after=${after}`),
});

/** Service credentials. */
const credentialCalls = (http: HttpRef) => ({
  /** Mint a service credential. Its token is returned this once. */
  createCredential: async (options: CredentialRequest): Promise<NewCredential> =>
    http().request('POST', '/api/control/credentials', options),
  /** Revoke a service credential. */
  revokeCredential: async (id: string): Promise<Ack> => http().request('DELETE', item('credentials', id)),
});

/** Project members. */
const memberCalls = (http: HttpRef) => ({
  /** The owner and every member. */
  members: async (): Promise<MemberList> => http().request('GET', '/api/control/members'),
  /** An invitation code for a new member. */
  inviteMember: async (role: ControlRole = 'operator'): Promise<Invite> =>
    http().request('POST', '/api/control/members/invite', { role }),
  /** Remove a member. */
  removeMember: async (userId: string): Promise<Ack> => http().request('DELETE', item('members', userId)),
});

/** Signed event webhooks. */
const webhookCalls = (http: HttpRef) => ({
  /** Register a webhook; `types` limits which events it receives. */
  createWebhook: async (url: string, types: string[] = []): Promise<Webhook> =>
    http().request('POST', '/api/control/webhooks', { url, types }),
  /** Disable a webhook. */
  removeWebhook: async (id: string): Promise<Ack> => http().request('DELETE', item('webhooks', id)),
  /** Send a delivery again. */
  replayDelivery: async (id: string): Promise<Ack> => http().request('POST', `${item('deliveries', id)}/replay`, {}),
});

/** Builds `oya.control`. */
export const controlApi = (http: HttpRef) => ({
  ...sessionCalls(http),
  ...lifecycleCalls(http),
  ...recoveryCalls(http),
  ...credentialCalls(http),
  ...memberCalls(http),
  ...webhookCalls(http),
});
