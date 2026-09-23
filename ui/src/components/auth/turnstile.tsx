/**
 * The Turnstile widget. Cloudflare's script is loaded by a script the page's
 * nonce already trusts, so the CSP's 'strict-dynamic' lets it run; its
 * challenge iframe is allowed by frame-src in lib/csp.ts.
 */
'use client';

import { useEffect, useRef, type RefObject } from 'react';
import Script from 'next/script';
import type { Captcha } from './use-turnstile';

/** Cloudflare's script, in explicit-render mode so React decides where the widget goes. */
const TURNSTILE_SCRIPT = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';

/** The parts of a captcha the widget writes to; all stable across renders. */
type Target = Pick<Captcha, 'siteKey' | 'setToken' | 'widget'>;

/** Draws the widget into `el` once, wiring its token into the form's captcha. */
function mount(el: HTMLDivElement | null, { siteKey, setToken, widget }: Target) {
  if (!el || !window.turnstile || widget.current) return;
  widget.current = window.turnstile.render(el, {
    sitekey: siteKey,
    callback: setToken,
    'expired-callback': () => setToken(''),
  });
}

/** Mounts the widget if the script is already loaded, and removes it when the form goes. */
function useWidget(box: RefObject<HTMLDivElement | null>, { siteKey, setToken, widget }: Target) {
  useEffect(() => {
    mount(box.current, { siteKey, setToken, widget });
    return () => {
      if (widget.current) window.turnstile?.remove(widget.current);
      widget.current = null;
    };
  }, [box, siteKey, setToken, widget]);
}

/** Turnstile's props. */
interface TurnstileProps {
  /** The form's captcha state, which the widget fills in. */
  captcha: Captcha;
}

/** The captcha box, or nothing when the captcha is off. */
export function Turnstile({ captcha }: TurnstileProps) {
  const box = useRef<HTMLDivElement>(null);
  useWidget(box, captcha);
  if (!captcha.siteKey) return null;
  return (
    <>
      <Script src={TURNSTILE_SCRIPT} strategy="afterInteractive" onReady={() => mount(box.current, captcha)} />
      <div ref={box} className="flex min-h-[65px] justify-center" />
    </>
  );
}
