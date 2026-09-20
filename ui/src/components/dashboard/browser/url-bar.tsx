/**
 * The URL bar: back and forward (CDP browsers only), reload, the address, Go.
 * Driving it needs a person to hold control.
 */
import type { RefObject } from 'react';
import { ArrowLeft, ArrowRight, RotateCw } from 'lucide-react';
import type { BrowserDetail } from '../types';
import type { Busy, Send } from './types';
import type { useNavigation } from './use-navigation';

/** The browser, the navigation state and whether a person holds control. */
interface Props {
  /** Latest detail, or null before the first poll. */
  d: BrowserDetail | null;
  /** State from `useNavigation`. */
  nav: ReturnType<typeof useNavigation>;
  /** Sends back and forward. */
  send: Send;
  /** Which action is running. */
  busy: Busy;
  /** Whether a person holds control; otherwise the bar is read-only. */
  human: boolean;
  /** Why the controls are off, as a tooltip; undefined while a person holds control. */
  needsControl?: string;
  /** Lets the page's shortcut focus the address. */
  urlRef: RefObject<HTMLInputElement | null>;
}

/** What the back and forward buttons need. */
interface HistoryProps {
  /** Sends back and forward. */
  send: Send;
  /** Whether a person holds control. */
  human: boolean;
  /** Why the buttons are off, as a tooltip. */
  needsControl?: string;
}

/** Back and forward; only CDP browsers take them over the socket. */
function HistoryButtons({ send, human, needsControl }: HistoryProps) {
  return (
    <>
      <button
        type="button"
        className="btn-icon"
        title={needsControl ?? 'Back'}
        disabled={!human}
        onClick={() => send('back')}
      >
        <ArrowLeft className="h-4 w-4" />
      </button>
      <button
        type="button"
        className="btn-icon"
        title={needsControl ?? 'Forward'}
        disabled={!human}
        onClick={() => send('forward')}
      >
        <ArrowRight className="h-4 w-4" />
      </button>
    </>
  );
}

/** The address form. */
export default function UrlBar({ d, nav, send, busy, human, needsControl, urlRef }: Props) {
  return (
    <form
      className="flex items-center gap-1.5"
      onSubmit={(e) => {
        e.preventDefault();
        if (human) nav.go();
      }}
    >
      {d?.clientType === 'cdp' && <HistoryButtons send={send} human={human} needsControl={needsControl} />}
      <button
        type="button"
        className="btn-icon"
        title={needsControl ?? 'Reload (R)'}
        onClick={nav.reload}
        disabled={busy === 'reload' || !human}
      >
        <RotateCw className={`h-4 w-4 ${busy === 'reload' ? 'animate-spin' : ''}`} />
      </button>
      <input
        ref={urlRef}
        value={nav.url ?? d?.currentUrl ?? ''}
        onChange={(e) => nav.setUrl(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            nav.setUrl(null);
            (e.target as HTMLInputElement).blur();
          }
        }}
        placeholder="Enter a URL and press Enter"
        className="field flex-1 font-mono text-[12.5px]"
        aria-label="Navigate to URL"
        readOnly={!human}
        title={needsControl}
        spellCheck={false}
      />
      <button type="submit" className="btn-primary h-8" title={needsControl} disabled={busy === 'navigate' || !human}>
        {busy === 'navigate' ? 'Going…' : 'Go'}
      </button>
    </form>
  );
}
