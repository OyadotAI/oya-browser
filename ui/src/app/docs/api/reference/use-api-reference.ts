/**
 * The API reference's life: fetch the spec, mount Scalar on it once its script is
 * in, follow the site's theme, and say "failed" when the script or the spec does not
 * arrive in LOAD_TIMEOUT_MS, so the page offers the raw JSON instead of spinning.
 */
import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import { LOAD_TIMEOUT_MS, SCALAR_CSS } from './constants';

/** Where the reference is. */
export type ReferenceStatus = 'loading' | 'ready' | 'failed';

/** What Scalar's createApiReference gives back. */
interface ScalarInstance {
  /** Replaces its whole configuration. */
  updateConfiguration: (config: object) => void;
  /** Unmounts it. */
  destroy: () => void;
}

declare global {
  /** The page's window, with what Scalar's standalone script adds to it. */
  interface Window {
    /** Scalar's standalone build, once its script has run. */
    Scalar?: {
      /** Renders a reference into `el`. */ createApiReference: (el: HTMLElement, config: object) => ScalarInstance;
    };
  }
}

/** The site's theme as Scalar names it. */
const currentTheme = () => (document.documentElement.dataset.theme === 'light' ? 'light' : 'dark');

/**
 * Scalar's settings: the spec as already fetched, the site's theme with Scalar's own
 * toggle hidden, and no request client. A "try it" box would ask for an API key on a
 * public page and send it through Scalar's proxy, so it is off. Its developer tools,
 * agent and telemetry are off too: on a localhost-style host (a self-hosted console)
 * Scalar turns its tools on by itself, and one of them inserts an unpinned third-party
 * script, which the page's 'strict-dynamic' would then trust beside the console's credential.
 */
const LOCKED_DOWN = {
  hideClientButton: true,
  hideTestRequestButton: true,
  showDeveloperTools: 'never',
  agent: { disabled: true },
  telemetry: false,
};

/** Scalar's whole configuration for this spec, in the site's current theme. */
const configFor = (content: unknown) => ({
  ...LOCKED_DOWN,
  content,
  hideDarkModeToggle: true,
  forceDarkModeState: currentTheme(),
  withDefaultFonts: false,
  customCss: SCALAR_CSS,
});

/** Fetches the spec as JSON; a non-2xx answer is a failure like a network one. */
async function fetchSpec(url: string): Promise<unknown> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url} answered ${res.status}`);
  return res.json();
}

/** Says "failed" if the reference is still loading after LOAD_TIMEOUT_MS. */
function useLoadTimeout(status: ReferenceStatus, setStatus: (s: ReferenceStatus) => void) {
  useEffect(() => {
    if (status !== 'loading') return;
    const timer = setTimeout(() => setStatus('failed'), LOAD_TIMEOUT_MS);
    return () => clearTimeout(timer);
  }, [status, setStatus]);
}

/** What the reference keeps between renders, read only in effects and callbacks. */
interface Live {
  /** The rendered reference, once there is one. */
  instance: ScalarInstance | null;
  /** The spec as fetched, kept for re-theming. */
  content: unknown;
}

/** Sets the reference's status. */
type SetStatus = (s: ReferenceStatus) => void;

/** Re-themes the reference when the site's theme toggle fires, and unmounts it on leave. */
function useFollowTheme(live: RefObject<Live>) {
  useEffect(() => {
    const l = live.current;
    const retheme = () => l.instance?.updateConfiguration(configFor(l.content));
    window.addEventListener('oya-theme', retheme);
    return () => (window.removeEventListener('oya-theme', retheme), l.instance?.destroy());
  }, [live]);
}

/**
 * The spec's fetch, started on first need: by the effect on mount, or by Scalar's
 * script being ready, whichever comes first. On a return visit next/script calls
 * onReady from its own effect, before this hook's effects run, so the fetch cannot
 * wait for an effect to start it.
 */
function useSpec(url: string, onFail: () => void) {
  const spec = useRef<Promise<unknown> | null>(null);
  const ensure = useCallback(() => (spec.current ??= fetchSpec(url)), [url]);
  useEffect(() => void ensure().catch(onFail), [ensure, onFail]);
  return ensure;
}

/** The status, and a setter that marks it failed. */
function useStatus() {
  const [status, setStatus] = useState<ReferenceStatus>('loading');
  const fail = useCallback(() => setStatus('failed'), []);
  return { status, setStatus, fail };
}

/** What rendering needs: where, what it keeps, and the spec's fetch. */
interface Target {
  /** Where Scalar renders. */
  host: RefObject<HTMLDivElement | null>;
  /** What it keeps between renders. */
  live: RefObject<Live>;
  /** The spec's fetch, started if it has not been. */
  spec: () => Promise<unknown>;
}

/** Renders the reference once the spec is in; Scalar missing is an error. */
async function render({ host, live, spec }: Target) {
  const l = live.current;
  l.content = await spec();
  if (!host.current || !window.Scalar) throw new Error('Scalar did not load');
  l.instance?.destroy();
  l.instance = window.Scalar.createApiReference(host.current, configFor(l.content));
}

/** Renders the reference and says how it went. */
const mount = (t: Target, setStatus: SetStatus) =>
  render(t).then(
    () => setStatus('ready'),
    () => setStatus('failed'),
  );

/** The element Scalar renders into, the status, and what to run once Scalar's script is in. */
export function useApiReference(url: string) {
  const host = useRef<HTMLDivElement>(null);
  const live = useRef<Live>({ instance: null, content: null });
  const { status, setStatus, fail } = useStatus();
  const spec = useSpec(url, fail);
  useLoadTimeout(status, setStatus);
  useFollowTheme(live);
  const onReady = useCallback(() => void mount({ host, live, spec }, setStatus), [spec, setStatus]);
  return { host, status, onReady, onError: fail };
}
