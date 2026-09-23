/**
 * Express stand-ins for the auth tests: a response that records its status,
 * body and cookies, a request, and a way to run a middleware or a router.
 */

/** A recording Express response. */
export class FakeResponse {
  /** The status set, 200 until one is. */
  statusCode = 200;
  /** The JSON body sent, once sent. */
  body: any = undefined;
  /** Cookies set, by name. */
  cookies: Record<string, { value: string; options: any }> = {};
  /** Cookies cleared, by name, with their options. */
  cleared: Record<string, any> = {};

  /** Sets the status. */
  status(code: number) {
    this.statusCode = code;
    return this;
  }

  /** Records the body. */
  json(body: any) {
    this.body = body;
    return this;
  }

  /** Records a cookie. */
  cookie(name: string, value: string, options: any) {
    this.cookies[name] = { value, options };
    return this;
  }

  /** Where a redirect pointed, once one was sent. */
  location = '';

  /** Records a 302 and finishes the response the way json() does. */
  redirect(url: string) {
    this.statusCode = 302;
    this.location = url;
    return this.json(undefined);
  }

  /** Records a cleared cookie. */
  clearCookie(name: string, options: any) {
    this.cleared[name] = options;
    return this;
  }
}

/** A request with the given method, path, headers and body. */
export function fakeRequest({ method = 'GET', path = '/', baseUrl = '', headers = {}, body = {} }: any = {}) {
  return { method, path, baseUrl, url: path, originalUrl: baseUrl + path, headers: { ...headers }, body } as any;
}

/** Runs one middleware; resolves with the response and whether it called next(). */
export async function runMiddleware(middleware: (req: any, res: any, next: () => void) => unknown, req: any) {
  const res = new FakeResponse();
  let passed = false;
  await middleware(req, res, () => {
    passed = true;
  });
  return { res, passed };
}

/**
 * Sends a request through an Express router and resolves once it answers (or
 * falls through, answering 404 here).
 */
export function routeThrough(router: any, req: any) {
  const res = new FakeResponse();
  return new Promise<FakeResponse>((resolve, reject) => {
    const answer = res.json.bind(res);
    res.json = (body: any) => {
      answer(body);
      resolve(res);
      return res;
    };
    router(req, res, (err?: any) => (err ? reject(err) : resolve(res.status(404))));
  });
}
