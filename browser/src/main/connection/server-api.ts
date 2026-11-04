/**
 * HTTP calls to the server behind the control socket, made as this browser
 * with the project key.
 */
import type { AppServices } from '../app/services.ts';

/** What the calls read: the server address and key, and the socket's browser id. */
type Deps = Pick<AppServices, 'config' | 'socket'>;

/** Optional cancellation retains an independently configured request deadline. */
function requestSignal(signal?: AbortSignal, timeoutMs?: number): AbortSignal | undefined {
  const deadline = timeoutMs ? AbortSignal.timeout(timeoutMs) : undefined;
  return signal && deadline ? AbortSignal.any([signal, deadline]) : signal || deadline;
}

/** A server answer's body, as far as an error goes. */
interface ErrorBody {
  /** The server's own reason. */
  error?: string;
  /** The server's code for it (llm_unconfigured, llm_quota, …), when it gave one. */
  code?: string;
}

/** The HTTP origin behind the control socket. */
export function serverHttpBase(serverUrl: string | undefined): string {
  return (serverUrl || '')
    .replace(/^wss/, 'https')
    .replace(/^ws/, 'http')
    .replace(/\/ws\/?$/, '');
}

/** A response's JSON, or an error with the server's own message, the status, and the server's code when it gave one. */
export async function readApiAnswer(res: Response): Promise<unknown> {
  const body = (await res.json().catch(() => ({}))) as ErrorBody;
  if (!res.ok)
    throw Object.assign(new Error(body.error || `Server returned ${res.status}`), {
      status: res.status,
      code: body.code,
    });
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

  /**
   * Sends `method` with a JSON body to `/api/<route>` as this project: answers its JSON, or throws with the
   * server's own error and status. `timeoutMs` gives up on a server that never answers, so a call made inside
   * a queue (a recording save) cannot hold everything behind it.
   */
  async send(method: string, route: string, payload?: unknown, timeoutMs?: number): Promise<unknown> {
    return this.request(method, route, payload, requestSignal(undefined, timeoutMs));
  }

  /** Provider setup can abort its project write after a path or workspace change. */
  postCancelable(route: string, payload: unknown, signal: AbortSignal): Promise<unknown> {
    return this.request('POST', route, payload, signal);
  }
  /** Setup catalog reads share the mutation's cancellation and workspace snapshot. */
  getCancelable(route: string, signal: AbortSignal): Promise<unknown> {
    return this.request('GET', route, undefined, signal);
  }

  /** Credentials and destination are snapshotted together before the network await. */
  private async request(method: string, route: string, payload: unknown, signal?: AbortSignal): Promise<unknown> {
    const config = this.deps.config.values;
    const res = await fetch(`${serverHttpBase(config.serverUrl)}/api/${route}`, {
      method,
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${config.apiKey}` },
      ...(payload === undefined ? {} : { body: JSON.stringify(payload) }),
      signal,
    });
    return readApiAnswer(res);
  }

  /** POSTs JSON to `/api/<route>` as this project: answers its JSON, or throws with the server's own error. */
  post(route: string, payload: unknown): Promise<unknown> {
    return this.send('POST', route, payload);
  }
}
