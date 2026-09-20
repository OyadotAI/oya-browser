/**
 * Before recording: which browser, and what recording does to it.
 */
'use client';

import type { BrowserRow } from '../types';
import type { Recorder } from './recorder';

/** Props for the setup step. */
interface Props {
  /** The recording session. */
  r: Recorder;
  /** Running browsers to choose from. */
  browsers: BrowserRow[];
}

/** Pick the browser to record on; warns when someone else holds it. */
export default function RecordSetup({ r, browsers }: Props) {
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
        <p className="text-sm text-text-muted">No browser is running. Start one from the Browsers tab.</p>
      )}
      <p className="mt-2 text-[12px] text-text-muted">
        Recording takes control of the browser so your input reaches the page. Passwords are masked in the page and
        saved as variables.
      </p>
      {r.held && (
        <p className="mt-2 text-[12px] text-yellow">
          Someone else is holding this browser — another dashboard tab, or one that was closed without releasing. Take
          over to record anyway; their live view stops driving it.
        </p>
      )}
    </div>
  );
}
