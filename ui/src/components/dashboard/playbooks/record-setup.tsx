/**
 * Before recording: which browser, and what recording does to it.
 */
'use client';

import { Loader2 } from 'lucide-react';
import type { BrowserRow } from '../types';
import type { Recorder } from './recorder';
import type { RecordStart } from './use-record-start';

/** Props for the setup step. */
interface Props {
  /** The recording session. */
  r: Recorder;
  /** Running browsers to choose from. */
  browsers: BrowserRow[];
  /** Starting a browser when none is running. */
  auto: RecordStart;
}

/** No browser yet: say one will be started, that it is starting, or why it did not. */
function NoBrowser({ auto }: { /** The start. */ auto: RecordStart }) {
  if (auto.error)
    return (
      <div role="alert" className="flex flex-wrap items-center gap-3 text-sm text-red">
        <span>Couldn&apos;t start a browser: {auto.error}</span>
        <button type="button" className="btn-ghost h-7 px-2 text-[12px]" autoFocus onClick={() => void auto.start()}>
          Try again
        </button>
      </div>
    );
  if (auto.starting)
    return (
      <p role="status" className="flex items-center gap-2 text-sm text-text-muted">
        <Loader2 className="h-4 w-4 animate-spin" /> Starting a browser…
      </p>
    );
  return (
    <p className="text-sm text-text-muted">No browser is running. Recording starts one with your project defaults.</p>
  );
}

/** Pick the browser to record on; warns when someone else holds it. */
export default function RecordSetup({ r, browsers, auto }: Props) {
  return (
    <div>
      <label className="label" htmlFor="rec-browser">
        Browser
      </label>
      {browsers.length ? (
        <select
          id="rec-browser"
          className="field"
          disabled={!!r.busy}
          value={r.browserId}
          onChange={(e) => r.setBrowserId(e.target.value)}
        >
          {browsers.map((b) => (
            <option key={b.id} value={b.id}>
              {b.name} · {b.id}
            </option>
          ))}
        </select>
      ) : (
        <NoBrowser auto={auto} />
      )}
      <p className="mt-2 text-[12px] text-text-muted">
        Recording takes control of the browser so your input reaches the page. Passwords are masked in the page and
        saved as variables.
      </p>
      {r.held && (
        <p className="mt-2 text-[12px] text-yellow">
          Someone else is holding this browser, another dashboard tab, or one that was closed without releasing. Take
          over to record anyway; their live view stops driving it.
        </p>
      )}
    </div>
  );
}
