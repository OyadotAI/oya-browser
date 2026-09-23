/**
 * The answer when Oya Cloud browsers are asked for but the server is not
 * configured to launch them.
 */
import { missingSettings } from '../../../drivers/sandbox.ts';
import { Status } from '../../../platform/http-status.ts';

/**
 * The sandbox dials back to this server, so a localhost address is
 * unreachable from a cloud VM.
 */
const TUNNEL_HINT =
  ' OYA_PUBLIC_WS_URL must be reachable from the sandbox, so a server on' +
  ' localhost needs a tunnel (ngrok, cloudflared) rather than ws://localhost.';

/** Names the missing settings, with the tunnel hint when the public URL is one of them. */
function missingMessage(missing: string[]) {
  const verb = missing.length > 1 ? 'are' : 'is';
  const hint = missing.includes('OYA_PUBLIC_WS_URL') ? TUNNEL_HINT : '';
  return `Cloud browsers need ${missing.join(', ')}, which ${verb} not set.${hint}`;
}

/** Answers 409 with what the key (or, without one, the deployment) is missing. */
export function sandboxMissing(res, key?) {
  const missing = missingSettings(key);
  return res.status(Status.CONFLICT).json({ error: missingMessage(missing), missing });
}
