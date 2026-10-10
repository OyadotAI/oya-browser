/** Shared validation for external commands arriving over local or authenticated server transports. */
import { ProtocolError } from './protocol.ts';
import { NATIVE_DOOR } from './constants.ts';
import type { NativeCommand } from './types.ts';

/** Parse only flat JSON-RPC commands; arrays and invalid id/session/params shapes are rejected. */
export function nativeCommand(raw: string): NativeCommand {
  const msg = boundedMessage(raw);
  if (!msg || Array.isArray(msg) || !Number.isSafeInteger(msg.id) || typeof msg.method !== 'string')
    throw new ProtocolError('Invalid command', NATIVE_DOOR.invalid);
  if (msg.sessionId !== undefined && typeof msg.sessionId !== 'string')
    throw new ProtocolError('Invalid session', NATIVE_DOOR.invalid);
  if (msg.params !== undefined && (!msg.params || typeof msg.params !== 'object' || Array.isArray(msg.params)))
    throw new ProtocolError('Invalid params', NATIVE_DOOR.invalid);
  return { ...msg, params: msg.params || {} };
}

/** Bound decoded control-socket input just as strictly as WebSocket requests. */
function boundedMessage(raw: string) {
  if (Buffer.byteLength(raw) > NATIVE_DOOR.maxPayload) throw new ProtocolError('Native command is too large');
  return JSON.parse(raw);
}
