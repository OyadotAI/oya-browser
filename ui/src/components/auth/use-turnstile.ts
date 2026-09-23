/**
 * A Turnstile captcha's state for one form: the token Cloudflare issued, the
 * rendered widget, and a reset for after a failed submit, since each token
 * is good for one check only.
 */
'use client';

import { useCallback, useRef, useState } from 'react';

/** The explicit-render API Cloudflare's script puts on window. */
interface TurnstileApi {
  /** Draws a widget into `el`, returning its id. */
  render: (el: HTMLElement, options: Record<string, unknown>) => string;
  /** Issues the widget a fresh challenge. */
  reset: (id: string) => void;
  /** Takes the widget off the page. */
  remove: (id: string) => void;
}

declare global {
  /** Cloudflare's script, once loaded. */
  interface Window {
    /** The Turnstile API. */
    turnstile?: TurnstileApi;
  }
}

/** Captcha state for one form; with no site key it is off and never blocks. */
export function useTurnstile(siteKey: string) {
  const [token, setToken] = useState('');
  const widget = useRef<string | null>(null);
  const reset = useCallback(() => {
    setToken('');
    if (widget.current) window.turnstile?.reset(widget.current);
  }, []);
  return { siteKey, token, setToken, widget, reset, missing: Boolean(siteKey) && !token };
}

/** One form's captcha. */
export type Captcha = ReturnType<typeof useTurnstile>;
