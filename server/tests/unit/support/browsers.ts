/**
 * Test doubles for the browser, MCP and app route handlers: an Express-shaped
 * request and response that record what a handler answered, a driven (CDP)
 * browser in the registry, an MCP server that captures what is registered on
 * it, and a control plane stubbed on its singleton.
 */
import { EventEmitter } from 'node:events';
import { mock } from 'node:test';
import { registry } from '../../../src/modules/browsers/registry.ts';
import { control } from '../../../src/modules/control/service.ts';

/** An Express response stand-in: records the status, body, headers and anything streamed. */
export class FakeResponse extends EventEmitter {
  /** The answered status. */
  statusCode = 200;
  /** The JSON body, once answered. */
  body: any = undefined;
  /** Headers set with set() or writeHead(). */
  headers: Record<string, string> = {};
  /** Chunks written with write(). */
  written: string[] = [];
  /** What end() was called with, once ended. */
  ended: string | null = null;
  /** Set once a status line has gone out. */
  headersSent = false;
  /** Bytes buffered but not yet flushed, as a slow viewer would have. */
  writableLength = 0;

  /** Sets the status; chainable like Express. */
  status(code: number) {
    this.statusCode = code;
    return this;
  }

  /** Answers with a JSON body. */
  json(body: unknown) {
    this.body = body;
    this.headersSent = true;
    return this;
  }

  /** Sets one header. */
  set(name: string, value: string) {
    this.headers[name] = value;
  }

  /** Sends the status line and headers. */
  writeHead(code: number, headers: Record<string, string> = {}) {
    this.statusCode = code;
    Object.assign(this.headers, headers);
    this.headersSent = true;
  }

  /** Records a streamed chunk. */
  write(chunk: string) {
    this.written.push(chunk);
  }

  /** Records the end of the response. */
  end(chunk = '') {
    this.ended = chunk;
  }

  /** Express's socket timeout; ignored. */
  setTimeout() {}
}

/** What a fake request is built from. */
type RequestOptions = {
  /** The caller's API key, sent as a bearer token; '' sends none. */
  key?: string;
  /** Route parameters. */
  params?: Record<string, string>;
  /** Parsed JSON body. */
  body?: any;
  /** Parsed query string. */
  query?: Record<string, unknown>;
  /** Extra headers. */
  headers?: Record<string, string>;
  /** Anything else the handler reads (controlSession, principal…). */
  extra?: Record<string, unknown>;
};

/** An Express request stand-in carrying a bearer key; emits 'close' when told to. */
export function fakeRequest({
  key = 'key-a',
  params = {},
  body = {},
  query = {},
  headers = {},
  extra = {},
}: RequestOptions = {}) {
  const req: any = new EventEmitter();
  req.headers = { ...(key ? { authorization: `Bearer ${key}` } : {}), ...headers };
  Object.assign(req, { params, body, query, method: 'POST', path: '/', socket: {}, setTimeout: () => {}, ...extra });
  return req;
}

/** Registers a driven (CDP) browser whose driver answers every command with `answer`; returns the driver. */
export function driveBrowser(browserId: string, answer: (action: string, params: any) => any, apiKey = 'key-a') {
  const driver = {
    sent: [] as { action: string; params: any; timeout: number }[],
    closed: false,
    alive: true,
    async send(action: string, params: any, timeout: number) {
      driver.sent.push({ action, params, timeout });
      return answer(action, params);
    },
    close() {
      driver.closed = true;
    },
    isAlive: () => driver.alive,
  };
  registry.add(browserId, { apiKey, name: 'Driven', clientType: 'cdp', provider: 'cdp', driver });
  return driver;
}

/** One tool or resource registered on a FakeMcpServer. */
type Registered = {
  /** What the model is told. */
  description: string;
  /** The zod shape of its arguments. */
  schema: unknown;
  /** Its handler. */
  handler: (args?: any) => Promise<any>;
};

/** An McpServer stand-in: keeps each tool and resource by name so a test can call it. */
export class FakeMcpServer {
  /** Registered tools by name. */
  tools = new Map<string, Registered>();
  /** Registered resources by name. */
  resources = new Map<string, { uri: string; read: () => Promise<any> }>();

  /** Registers a tool, as McpServer.tool does. */
  tool(name: string, description: string, schema: unknown, handler: (args?: any) => Promise<any>) {
    this.tools.set(name, { description, schema, handler });
  }

  /** Registers a resource, as McpServer.resource does. */
  resource(name: string, uri: string, _meta: unknown, read: () => Promise<any>) {
    this.resources.set(name, { uri, read });
  }

  /** Calls a tool by name. */
  call(name: string, args: any = {}) {
    return this.tools.get(name).handler(args);
  }
}

/**
 * Replaces the control plane's methods on its process-wide singleton so a
 * handler never touches the durable store: every command slot opens, no
 * session exists, updates succeed and tickets are 'ticket-1'. `overrides`
 * replaces any of them. Undone by mock.restoreAll().
 */
export function stubControl(overrides: Record<string, (...args: any[]) => any> = {}) {
  const defaults = {
    beginCommand: async () => async () => {},
    findSession: async () => null,
    sessions: async () => [],
    update: async () => ({}),
    assertProvisioning: async () => {},
    cancel: async () => ({ state: 'cancelled' }),
    ticket: async () => 'ticket-1',
    emit: async () => {},
    authenticate: async () => null,
  };
  const service = control();
  const mocks: Record<string, any> = {};
  for (const [name, fn] of Object.entries({ ...defaults, ...overrides })) mocks[name] = mock.method(service, name, fn);
  return mocks;
}

/**
 * Sends `method url` through an Express router as `key` and resolves with the
 * response once a handler answers JSON. An error reaching the end goes to
 * `onError`; a request no route takes is answered 404 `{ unrouted: true }`.
 */
export function routeThrough(router, method: string, url: string, key = 'oya_op', onError?: (...args: any[]) => void) {
  const res: any = new FakeResponse();
  const answered = new Promise<any>((resolve) => {
    res.json = (body: unknown) => {
      res.body = body;
      res.headersSent = true;
      res.emit('finish');
      resolve(res);
      return res;
    };
  });
  const req = fakeRequest({ key, extra: { method, url, originalUrl: url, path: url } });
  router(req, res, (err) =>
    err && onError ? onError(err, req, res, () => {}) : res.status(404).json({ unrouted: true }),
  );
  return answered;
}
