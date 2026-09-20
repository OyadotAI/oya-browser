/**
 * `oya.control`: the durable control plane. Sessions (including disconnected
 * and cleanup-pending ones), human takeover, lifecycle events, service
 * credentials, members and webhooks. Built from one group per topic.
 */
import type {
  ControlOverview,
  ControlRole,
  ControlSession,
  HumanInputAction,
  ProjectSettings,
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
const session = (id: string, action = '') => `/api/control/sessions/${encodeURIComponent(id)}${action}`;
/** An endpoint for one item in a control collection. */
const item = (kind: string, id: string) => `/api/control/${kind}/${encodeURIComponent(id)}`;

/** Reading sessions and project settings. */
const sessionCalls = (http: HttpRef) => ({
  /** Settings, sessions and recent events at a glance. */
  overview: (): Promise<ControlOverview> => http().request('GET', '/api/control'),
  /** Every session, including disconnected and cleanup-pending ones. */
  sessions: (): Promise<ControlSession[]> => http().request('GET', '/api/control/sessions'),
  /** One session. */
  session: (id: string): Promise<ControlSession> => http().request('GET', session(id)),
  /** Update limits, rate cards and retention. */
  settings: (changes: Partial<ProjectSettings>): Promise<ControlOverview['project']> =>
    http().request('PATCH', '/api/control/project', changes),
});

/** Ending, steering and recovering sessions. */
const lifecycleCalls = (http: HttpRef) => ({
  /** Cancel queued or provisioning work. */
  cancel: (id: string): Promise<ControlSession> => http().request('POST', session(id, '/cancel'), {}),
  /** Stop a session; `force` stops despite a profile-save error, or reconciles. */
  stop: (id: string, force = false): Promise<StopResult> => http().request('POST', session(id, '/stop'), { force }),
  /** Acquire or release human control, or acknowledge the agent's resume. */
  takeover: (id: string, action: 'acquire' | 'release' | 'resume'): Promise<ControlSession['control']> =>
    http().request('POST', session(id, '/control'), { action }),
  /** Send one input as the human holding the control lease. */
  input: (id: string, action: HumanInputAction, params: Record<string, unknown>): Promise<unknown> =>
    http().request('POST', session(id, '/input'), { action, params }),
});

/** Recovery, live-stream tickets and the event log. */
const recoveryCalls = (http: HttpRef) => ({
  /** Explicitly recover a session, or replace it with a fresh one. */
  recover: (id: string, replace = false): Promise<unknown> =>
    http().request('POST', session(id, '/recover'), { replace }),
  /** A single-use ticket for the live stream. */
  ticket: (id: string): Promise<Ticket> => http().request('POST', session(id, '/ticket'), {}),
  /** Durable lifecycle events after a cursor. */
  events: (after = 0): Promise<EventPage> => http().request('GET', `/api/control/events?after=${after}`),
});

/** Service credentials. */
const credentialCalls = (http: HttpRef) => ({
  /** Mint a service credential. Its token is returned this once. */
  createCredential: (options: CredentialRequest): Promise<NewCredential> =>
    http().request('POST', '/api/control/credentials', options),
  /** Revoke a service credential. */
  revokeCredential: (id: string): Promise<Ack> => http().request('DELETE', item('credentials', id)),
});

/** Project members. */
const memberCalls = (http: HttpRef) => ({
  /** The owner and every member. */
  members: (): Promise<MemberList> => http().request('GET', '/api/control/members'),
  /** An invitation code for a new member. */
  inviteMember: (role: ControlRole = 'operator'): Promise<Invite> =>
    http().request('POST', '/api/control/members/invite', { role }),
  /** Remove a member. */
  removeMember: (userId: string): Promise<Ack> => http().request('DELETE', item('members', userId)),
});

/** Signed event webhooks. */
const webhookCalls = (http: HttpRef) => ({
  /** Register a webhook; `types` limits which events it receives. */
  createWebhook: (url: string, types: string[] = []): Promise<Webhook> =>
    http().request('POST', '/api/control/webhooks', { url, types }),
  /** Disable a webhook. */
  removeWebhook: (id: string): Promise<Ack> => http().request('DELETE', item('webhooks', id)),
  /** Send a delivery again. */
  replayDelivery: (id: string): Promise<Ack> => http().request('POST', `${item('deliveries', id)}/replay`, {}),
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
