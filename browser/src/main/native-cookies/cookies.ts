/** Native private-context cookie access never uses the default profile, page scripts or a debugging transport. */
import type { Session, Cookie } from 'electron';
import { cookieBatch, cookieURL, cookieString } from './parameters.ts';
import { COOKIE_LIMITS, COOKIE_PRIORITY, COOKIE_SCHEME, COOKIE_SAME_SITE } from './constants.ts';
/** Structured cookie arguments already validated at the external boundary. */
type Params = Record<string, unknown>;
/** Rechecked native session authority. */
type SessionAccess = () => Session;
/** Session resolution rechecks both connection ownership and human control before and after every native await. */
export async function nativeCookies(get: SessionAccess, action: string, params: Params): Promise<object> {
  if (action === 'cookies:get') return read(get);
  if (action === 'cookies:set') return write(get, params.cookies);
  if (action === 'cookies:delete') await get().cookies.remove(cookieURL(params.url), cookieString(params.name, true));
  else if (action === 'cookies:clear') await get().clearStorageData({ storages: ['cookies'] });
  else throw Error('Unsupported native cookie operation');
  get();
  return {};
}
/** Read actual engine metadata and reject unsupported partitioning rather than flattening distinct cookies. */
async function read(get: () => Session): Promise<object> {
  const cookies = await get().cookies.get({});
  get();
  if (cookies.length > COOKIE_LIMITS.records) throw Error('Native cookie result limit exceeded');
  return { cookies: cookies.map(describeCookie) };
}
/** Validation precedes every write; native rejections are surfaced and later entries are not attempted. */
async function write(get: () => Session, input: unknown): Promise<object> {
  const cookies = cookieBatch(input);
  for (const cookie of cookies) {
    await get().cookies.set(cookie);
    get();
  }
  return {};
}
/** Extra canonical fields are supplied by the patched native cookie converter. */
interface NativeCookie extends Cookie {
  /** Native priority enum. */
  _oyaPriority?: number;
  /** Native source scheme enum. */
  _oyaSourceScheme?: number;
  /** Actual native source port, including its unspecified sentinel. */
  _oyaSourcePort?: number;
  /** Partitioning is not silently discarded. */
  _oyaPartitioned?: boolean;
}
/** Fields missing from an older engine cannot be substituted with plausible defaults. */
function metadata(cookie: NativeCookie): object {
  if (!Number.isInteger(cookie._oyaSourcePort) || typeof cookie._oyaPartitioned !== 'boolean')
    throw Error('Oya engine lacks native cookie metadata');
  if (cookie._oyaPartitioned) throw Error('Partitioned cookie export is not supported');
  return {
    priority: nativeEnum(COOKIE_PRIORITY, cookie._oyaPriority),
    sourceScheme: nativeEnum(COOKIE_SCHEME, cookie._oyaSourceScheme),
    sourcePort: cookie._oyaSourcePort,
  };
}
/** Unknown native enum values require an engine/adapter upgrade, never guessed defaults. */
function nativeEnum(table: Record<number, string>, value: number | undefined): string {
  if (value === undefined || !Object.hasOwn(table, value)) throw Error('Oya engine lacks native cookie metadata');
  return table[value];
}
/** Serialize real canonical values, retaining unspecified SameSite rather than inventing explicit None. */
function describeCookie(cookie: NativeCookie): object {
  const sameSite = Object.entries(COOKIE_SAME_SITE).find(([, native]) => native === cookie.sameSite)?.[0];
  return { ...identity(cookie), ...lifetime(cookie), ...(sameSite ? { sameSite } : {}), ...metadata(cookie) };
}
/** Native scope and security attributes are preserved without normalization. */
function identity(cookie: NativeCookie): object {
  const { name, value, domain, path, secure, httpOnly } = cookie;
  return { name, value, domain, path, secure, httpOnly };
}
/** Persistent expiration is native; session cookies use the protocol's explicit sentinel. */
function lifetime(cookie: NativeCookie): object {
  return {
    session: cookie.session,
    expires: cookie.session ? COOKIE_LIMITS.session : cookie.expirationDate,
    size: Buffer.byteLength(cookie.name + cookie.value),
  };
}
