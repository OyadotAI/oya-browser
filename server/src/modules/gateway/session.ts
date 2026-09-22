/**
 * A gateway session. Owns the upstream CDP connection and survives a client
 * disconnect for GRACE_MS so a dropped client can resume against the same
 * provider with its page state intact.
 */
import { WebSocket } from 'ws';
import { metrics } from '../../platform/metrics.ts';
import { fingerprint } from '../../platform/audit.ts';
import * as recorder from './recorder.ts';
import { sessions } from './session-store.ts';
import { CommandGate } from './session-commands.ts';
import { teardown } from './session-teardown.ts';
import { GRACE_MS, MAX_PENDING_TO_CLIENT, MS_PER_SECOND } from './constants.ts';
import type { CdpEndpoint } from '../browsers/driver/index.ts';

/** A gateway session: one browser, and the client (if any) driving it. */
export class Session {
  /** The API key that opened the session. */
  declare apiKey: any;
  /** The registry browser this session attached to, when it was not acquired from a provider. */
  declare attachedTo: any;
  /** The credential the client connected with. */
  declare authToken: any;
  /** Bytes forwarded from the browser to the client. */
  bytesDown = 0;
  /** Bytes forwarded from the client to the browser. */
  bytesUp = 0;
  /** The connected client socket; null while disconnected. */
  client: any = null;
  /** Set once destroy() starts. */
  closed = false;
  /** Client commands in flight and their order. */
  readonly commands = new CommandGate(this);
  /** Pending destroy after the client disconnects. */
  graceTimer: any = null;
  /** Session id. */
  declare id: any;
  /** Fingerprint of the API key. */
  declare owner: any;
  /** Browser messages held while no client is connected, up to MAX_PENDING_TO_CLIENT. */
  pendingToClient: any[] = [];
  /** Named cookie profile restored at start and captured at the end, or null. */
  declare profile: any;
  /** The CDP connection profile restore opened, closed on destroy. */
  declare profileConn: any;
  /** Name of the provider the browser came from. */
  declare provider: any;
  /** Hands the browser back when the session ends. */
  declare release: any;
  /** Epoch ms the session began. */
  startedAt = Date.now();
  /** WebSocket to the browser's CDP endpoint, or the relay to an Oya client. */
  declare upstream: any;
  /** Where the browser speaks raw CDP; profiles and recordings open their own connection through it. */
  declare endpoint: CdpEndpoint;

  /** A session over an already-open upstream; no client yet. */
  constructor({ id, apiKey, provider, release, upstream, profile }) {
    Object.assign(this, { id, apiKey, provider, release, upstream, profile });
    // Profiles and recordings are namespaced by this, never by the raw key.
    this.owner = fingerprint(apiKey);
  }

  /** Connect a client: flush held messages, then forward its commands to the browser. */
  attach(client) {
    clearTimeout(this.graceTimer);
    this.graceTimer = null;
    this.client = client;
    this.flushPending(client);
    client.on('message', (data, isBinary) => this.commands.enqueue(client, data, isBinary));
    client.on('close', () => this.clientClosed(client));
    client.on('error', () => {});
  }

  /**
   * Anything the browser said while nobody was listening is delivered on
   * resume rather than lost.
   */
  private flushPending(client) {
    for (const buf of this.pendingToClient.splice(0)) {
      try {
        client.send(buf);
      } catch {}
    }
  }

  /** The client left: hold the browser briefly so a reconnect resumes the same session. */
  private clientClosed(client) {
    if (this.client !== client) return;
    this.client = null;
    if (this.closed) return;
    this.graceTimer = setTimeout(() => this.destroy('grace expired'), GRACE_MS);
    metrics.gatewaySessions.set({}, sessions.size);
  }

  /** Release a command's in-flight slot: on its reply, or once it has run for STUCK_COMMAND_MS. */
  settle(key) {
    this.commands.settle(key);
  }

  /** Forward browser traffic to the client (or hold it), and end the session if the browser goes. */
  bindUpstream() {
    this.upstream.on('message', (data, isBinary) => this.fromBrowser(data, isBinary));
    this.upstream.on('close', () => this.destroy('provider closed'));
    this.upstream.on('error', () => this.destroy('provider error'));
  }

  /** One browser message: release its command slot if it is a reply, then deliver or hold it. */
  private fromBrowser(data, isBinary) {
    // Only replies release command slots; skip parsing event traffic when nothing is outstanding.
    if (this.commands.releases.size) this.commands.settleReply(data);
    this.bytesDown += data.length;
    if (this.client?.readyState === WebSocket.OPEN) {
      this.client.send(data, { binary: isBinary });
    } else if (this.pendingToClient.length < MAX_PENDING_TO_CLIENT) {
      this.pendingToClient.push(data);
    }
  }

  /** End the session once: release its browser, capture its profile and recording, record usage. */
  async destroy(reason) {
    if (this.closed) return;
    this.closed = true;
    await teardown(this, reason);
  }

  /** Whole seconds since the session began. */
  elapsedSeconds() {
    return Math.round((Date.now() - this.startedAt) / MS_PER_SECOND);
  }

  /** The session as the API lists it. */
  toJSON() {
    return { ...this.summary(), ...this.traffic() };
  }

  /** What the session is attached to and whether a client is on it. */
  private summary() {
    return {
      id: this.id,
      provider: this.provider,
      profile: this.profile || null,
      attachedTo: this.attachedTo || null,
      connected: !!this.client,
    };
  }

  /** How long it has run and what it has carried. */
  private traffic() {
    return {
      startedAt: new Date(this.startedAt).toISOString(),
      seconds: this.elapsedSeconds(),
      bytesUp: this.bytesUp,
      bytesDown: this.bytesDown,
      recording: recorder.isRecording(this.id),
    };
  }
}
