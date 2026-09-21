/**
 * The gateway's live sessions and the client-facing WebSocket server, shared
 * by the session, its teardown and the upgrade handler.
 */
import { WebSocketServer } from 'ws';
import { MAX_PAYLOAD_BYTES } from './constants.ts';

/** Live gateway sessions, keyed by session id. */
export const sessions = new Map();

/** The client-facing WebSocket server; upgrades are routed in by handleUpgrade(). */
export const wss = new WebSocketServer({ noServer: true, perMessageDeflate: false, maxPayload: MAX_PAYLOAD_BYTES });
