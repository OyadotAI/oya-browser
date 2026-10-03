/**
 * Meta's ad pixel: loads fbevents.js once, the way Meta's own snippet does,
 * counts public page views, and reports a sign-up as CompleteRegistration so
 * Meta can measure and optimize the ads. The layout renders it only when the
 * operator set META_PIXEL_ID, and the component only on public pages.
 *
 * Two of Meta's defaults are turned off because the page stays loaded when a
 * new account moves on to the console: history tracking, which would report
 * console URLs as page views, and automatic event setup, which reads buttons
 * and page text. Page views are sent by hand, from public pages only.
 */
import { META_FLUSH_MS, META_MAX_WAIT_MS } from './constants';

/** Where Meta serves the pixel library from. */
const SCRIPT_URL = 'https://connect.facebook.net/en_US/fbevents.js';
/** What a Meta pixel id looks like: digits only; anything else is not sent to Meta. */
const PIXEL_ID = /^\d{10,20}$/;

/** The pixel's command queue, as Meta's snippet defines it. */
type Fbq = ((...args: unknown[]) => void) & {
  /** Commands sent before the library loaded, replayed once it has. */
  queue: unknown[][];
  /** Meta's flag that the stub is in place. */
  loaded: boolean;
  /** Meta's snippet version. */
  version: string;
  /** Meta's switch for reporting history changes as page views. */
  disablePushState?: boolean;
  /** The library's entry point, set once it has loaded. */
  callMethod?: (...args: unknown[]) => void;
};

/** The window with Meta's globals. */
type MetaWindow = Window & {
  /** The pixel's command function. */
  fbq?: Fbq;
  /** Meta's alias for it. */
  _fbq?: Fbq;
};

/** The pixel the operator configured, known even on pages where it does not load. */
let configured = '';
/** The pixel this page loaded, '' until one has. */
let pixel = '';
/** The page last counted, so a repeat render of the same page is not a second view. */
let counted = '';

/** Settles once the library has loaded or failed to; a sign-up waits on it before leaving the page. */
let ready: Promise<void> = Promise.resolve();

/** Whether `id` is a well-formed Meta pixel id. */
export const validMetaPixelId = (id: string | undefined): id is string => !!id && PIXEL_ID.test(id);

/** The command queue Meta's snippet installs before its library arrives. */
function stub(): Fbq {
  const fbq = ((...args: unknown[]) => (fbq.callMethod ? fbq.callMethod(...args) : fbq.queue.push(args))) as Fbq;
  return Object.assign(fbq, { queue: [], loaded: true, version: '2.0', disablePushState: true });
}

/** Adds the library's script and remembers when it settles. */
function addScript() {
  const script = document.createElement('script');
  script.async = true;
  script.src = SCRIPT_URL;
  ready = new Promise((settle) => {
    script.onload = script.onerror = () => settle();
  });
  document.head.appendChild(script);
}

/**
 * Remembers the operator's pixel without loading it, so a private page (the
 * OAuth callback) can still report a sign-up once it has cleared its URL.
 */
export function configureMetaPixel(id: string) {
  configured = id;
}

/** Starts the pixel for `id`, once per page load. */
export function loadMetaPixel(id: string) {
  const w = window as MetaWindow;
  if (w.fbq || !validMetaPixelId(id)) return;
  w.fbq = w._fbq = stub();
  pixel = id;
  addScript();
  w.fbq('set', 'autoConfig', false, id);
  w.fbq('init', id);
}

/**
 * Counts a view of `path`, the public page the visitor is on, once per visit
 * to it; nothing when the pixel is off. trackSingle, because Meta drops a
 * second plain PageView in one page load, so moving between pages without a
 * reload would count only the first.
 */
export function metaPageView(path: string) {
  if (!pixel || path === counted) return;
  counted = path;
  (window as MetaWindow).fbq?.('trackSingle', pixel, 'PageView');
}

/** Waits `ms`. */
const pause = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));

/**
 * Reports a sign-up, loading the configured pixel first if this page did not.
 * Resolves once the event has had time to leave, so a caller about to reload
 * the page (an OAuth sign-up) can wait for it; never longer than
 * META_MAX_WAIT_MS, so a blocked pixel cannot hold a sign-up up.
 */
export function metaSignUp(): Promise<void> {
  if (configured) loadMetaPixel(configured);
  const fbq = (window as MetaWindow).fbq;
  if (!fbq) return Promise.resolve();
  fbq('track', 'CompleteRegistration');
  return Promise.race([ready.then(() => pause(META_FLUSH_MS)), pause(META_MAX_WAIT_MS)]);
}
