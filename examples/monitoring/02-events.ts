// Events: tail your project's lifecycle log (sessions starting, failing, needing a person) as it happens. Ctrl+C stops.
import { setTimeout as sleep } from 'node:timers/promises';
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
const POLL_MS = 5_000;

// Start from the newest event, so only what happens from now on prints. Pass 0 to replay the whole log.
let cursor = (await oya.control.overview()).events.reduce((max, e) => Math.max(max, e.id), 0);
console.log(`watching from event ${cursor}…`);

for (;;) {
  const page = await oya.control.events(cursor); // up to 500 events after the cursor
  for (const e of page.events)
    console.log(new Date(e.at).toISOString(), e.type.padEnd(24), e.sessionId ?? '-', JSON.stringify(e.detail));
  cursor = page.cursor;
  if (page.events.length < 500) await sleep(POLL_MS); // caught up: wait before asking again
}
