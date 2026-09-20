/**
 * The ControlService class: every control-plane operation over one control
 * store. Each method is a thin entry into the file that owns its concern.
 */
import { controlStore } from '../store.ts';
import { Status } from '../../../platform/http-status.ts';
import { fault, projectId } from './model.ts';
import { publicSession } from './sessions.ts';
import { settleCosts } from './billing.ts';
import { overview, openProjectKey, projectOf, updateOwnedProject, updateSettings } from './projects.ts';
import { reserve } from './admission.ts';
import { claimQueued } from './queue.ts';
import { adopt, assertProvisioning, cancel, complete, update } from './lifecycle.ts';
import { drain, holdProvider, releaseProvider } from './holds.ts';
import { redeem, ticket } from './tickets.ts';
import { authenticate, credential, issue, revoke, share } from './credentials.ts';
import { beginCommand, takeover } from './takeover.ts';
import { emit, slackSink, webhook, webhookConfig } from './webhooks.ts';
import { DEFAULT_EVENT_LIMIT } from '../store/constants.ts';

/** Every control-plane operation, over one control store. */
export class ControlService {
  /** The durable store every operation transacts against. */
  declare store: any;
  constructor(store = controlStore()) {
    this.store = store;
    store.beforeCommit = settleCosts;
  }
  /** The key's project, created on first use, without its sealed key. */
  async project(key) {
    return projectOf(this.store, key);
  }
  /** Unseal the API key a project row holds; 503 when this server's secret cannot open it. */
  projectKey(project) {
    return openProjectKey(project);
  }
  /** Rename or delete a project its signed-in owner holds (see projects.ts). */
  async updateOwnedProject(userId, id, options: any = {}) {
    return updateOwnedProject(this.store, userId, id, options);
  }
  /** The project overview: sessions, recent events, credentials, webhooks and undelivered deliveries, secrets stripped. */
  async read(key) {
    return overview(this, key);
  }
  /** The project's sessions, optionally only in the given states. */
  async sessions(key, states) {
    const filter = { project: projectId(key), ...(states ? { states } : {}) };
    return (await this.store.list('session', filter)).map(publicSession);
  }
  /** This project's session, or null. */
  async findSession(key, id) {
    const x = await this.store.get('session', id);
    return x?.project === projectId(key) ? publicSession(x) : null;
  }
  /** This project's session; 404 if absent. */
  async session(key, id) {
    const x = await this.findSession(key, id);
    if (!x) throw fault('not_found', 'Session not found', Status.NOT_FOUND);
    return x;
  }
  /** The retained event log, oldest first after the cursor. */
  events(key, { after = 0, limit = DEFAULT_EVENT_LIMIT } = {}) {
    return this.store.events({ project: projectId(key), after, limit });
  }
  /** Merge validated changes into the project's settings. */
  async settings(key, changes) {
    return updateSettings(this.store, key, changes);
  }
  /** Hold one of a provider's slots across replicas for three minutes; 429 when all are held. Returns the hold id. */
  async holdProvider(owner, name, capacity) {
    return holdProvider(this.store, owner, name, capacity);
  }
  /** Give back a provider slot taken with holdProvider. */
  async releaseProvider(id) {
    return releaseProvider(this.store, id);
  }
  /** Admit a browser request before any provider is called (see admission.ts). Returns the session, `provisioning` or `queued`. */
  async reserve(key, options: any = {}) {
    return reserve(this.store, key, options);
  }
  /** Promote the next queued session that fits, with its project key and unsealed request, or null (see queue.ts). */
  async claimQueued() {
    return claimQueued(this.store);
  }
  /** Stop a session; a no-op once it is terminal. */
  async cancel(key, id, options = {}) {
    return cancel(this.store, key, id, options);
  }
  /** Record the provisioning response and move the session to the state it implies. */
  async complete(key, id, status, body) {
    return complete(this.store, key, id, status, body);
  }
  /** Throw unless this replica still holds a live provisioning lease on the session. */
  async assertProvisioning(key, id) {
    return assertProvisioning(this.store, key, id);
  }
  /** Patch a session. `fence` rejects a stale worker; terminal and stopping sessions cannot be moved back. */
  async update(key, id, changes, options: any = {}) {
    return update(this.store, key, id, changes, options);
  }
  /** A browser connected to this replica: reserve it if nothing did, re-check capacity after a disconnect, and mark it ready here. */
  async adopt(key, options: any = {}) {
    return adopt(this, key, options);
  }
  /** Set or clear the fleet-wide draining flag, which stops new admissions. */
  async drain(value) {
    return drain(this.store, value);
  }
  /** The project a key opens, without touching storage. */
  projectIdFor(key) {
    return projectId(key);
  }
  /** A single-use, one-minute ticket that stands in for the credential on one session's connection URL. */
  async ticket(key, sessionId, authToken = key) {
    return ticket(this.store, key, sessionId, authToken);
  }
  /** Consume a ticket for this session and return the credential it stands for. */
  async redeem(token, sessionId) {
    return redeem(this.store, token, sessionId);
  }
  /** Issue a project credential for a person or service. `memberUser` must still hold `role` in the project. */
  async credential(key, options = {}, memberUser = null) {
    return credential(this.store, key, options, memberUser);
  }
  /** A managed browser's own credential: it can only register that session's browser, and it ends with the session. */
  enrollmentCredential(key, sessionId) {
    const grant = { role: 'browser', label: 'Managed browser', expiresAt: null, memberUser: null, sessionId };
    return issue(this.store, key, grant);
  }
  /** A shareable, expiring credential for one live browser, as a viewer or an operator (see credentials.ts). */
  async share(key, options: any = {}) {
    return share(this, key, options);
  }
  /** Resolve a credential token to its principal; null if unknown, throws if revoked, expired or no longer entitled. */
  async authenticate(token) {
    return authenticate(this, token);
  }
  /** Revoke one of the project's credentials. */
  async revoke(key, id) {
    return revoke(this.store, key, id);
  }
  /** Move a session's control between agent and human; `force` takes it from another operator (see takeover.ts). */
  async takeover(key, id, action, holder, options = {}) {
    return takeover(this.store, key, id, action, holder, options);
  }
  /** Admit a browser command under the current control mode. Returns the function that settles it; calling it twice is harmless. */
  async beginCommand(id, holder = null) {
    return beginCommand(this.store, id, holder);
  }
  /** Save the project's one customer webhook; its secret is returned only when minted (see webhooks.ts). */
  async webhook(key, options) {
    return webhook(this.store, key, options);
  }
  /** The project's endpoint without its secret, and its latest deliveries; null hook when never set. */
  async webhookConfig(key) {
    return webhookConfig(this.store, key);
  }
  /** Connect or update the project's Slack sink, one row per project (see webhooks.ts). */
  async slackSink(key, patch = {}) {
    return slackSink(this.store, key, patch);
  }
  /** Record an event for a project that already exists, from outside the session state machine (see webhooks.ts). */
  async emit(key, type, sessionId = null, detail = {}) {
    return emit(this.store, key, type, sessionId, detail);
  }
}
