/**
 * HTTP calls to the server behind the control socket, made as this browser
 * with the project key.
 */
import type { AppServices } from '../app/services.ts';

/** What the calls read: the server address and key, and the socket's browser id. */
type Deps = Pick<AppServices, 'config' | 'socket'>;

/** A server answer's body, as far as an error goes. */
interface ErrorBody {
  /** The server's own reason. */
  error?: string;
}

/** The HTTP origin behind the control socket. */
export function serverHttpBase(serverUrl: string | undefined): string {
  return (serverUrl || '')
    .replace(/^wss/, 'https')
    .replace(/^ws/, 'http')
    .replace(/\/ws\/?$/, '');
}

/** A response's JSON, or an error with the server's own message and the status. */
export async function readApiAnswer(res: Response): Promise<unknown> {
  const body = (await res.json().catch(() => ({}))) as ErrorBody;
  if (!res.ok) throw Object.assign(new Error(body.error || `Server returned ${res.status}`), { status: res.status });
  return body;
}

/** The server's HTTP API, called as this browser and project. */
export class ServerApi {
  /** The settings and the socket. */
  private readonly deps: Deps;

  /** `deps` gives the server address, the key and the browser id. */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Whether the socket is authenticated and this browser has an id to call as. */
  canCall(): boolean {
    return this.deps.socket.ready && !!this.deps.socket.browserId;
  }

  /** POSTs JSON to `/api/browsers/<this browser>/<route>`, given up after `timeoutMs` when one is set, or when `signal` aborts. */
  postToBrowser(route: string, payload: unknown, timeoutMs?: number, signal?: AbortSignal): Promise<Response> {
    const config = this.deps.config.values;
    const signals = [signal, timeoutMs && AbortSignal.timeout(timeoutMs)].filter((s): s is AbortSignal => !!s);
    return fetch(`${serverHttpBase(config.serverUrl)}/api/browsers/${this.deps.socket.browserId}/${route}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${config.apiKey}` },
      body: JSON.stringify(payload),
      ...(signals.length ? { signal: AbortSignal.any(signals) } : {}),
    });
  }

  /** GETs `/api/<route>` as this project: answers its JSON, or throws. */
  async get(route: string): Promise<unknown> {
    const config = this.deps.config.values;
    const res = await fetch(`${serverHttpBase(config.serverUrl)}/api/${route}`, {
      headers: { Authorization: `Bearer ${config.apiKey}` },
    });
    if (!res.ok) throw new Error(`Server returned ${res.status}`);
    return res.json();
  }

  /** Sends `method` with a JSON body to `/api/<route>` as this project: answers its JSON, or throws with the server's own error and status. */
  async send(method: string, route: string, payload?: unknown): Promise<unknown> {
    const config = this.deps.config.values;
    const res = await fetch(`${serverHttpBase(config.serverUrl)}/api/${route}`, {
      method,
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${config.apiKey}` },
      ...(payload === undefined ? {} : { body: JSON.stringify(payload) }),
    });
    return readApiAnswer(res);
  }

  /** POSTs JSON to `/api/<route>` as this project: answers its JSON, or throws with the server's own error. */
  post(route: string, payload: unknown): Promise<unknown> {
    return this.send('POST', route, payload);
  }
}
