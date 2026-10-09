/** Refuse aggregate suites that still launch prohibited browser providers; never silently skip them. */
import { readFileSync } from 'node:fs';

/** Resolve from this module so invocation from another working directory cannot bypass the gate. */
const root = new URL('../../', import.meta.url);
/** Inspect the actual integration command rather than a separate, potentially stale file inventory. */
const manifest = JSON.parse(readFileSync(new URL('package.json', root), 'utf8'));
/** Each listed entry must be migrated before the aggregate integration suite can pass. */
const entries = manifest.scripts['test:integration'].match(/tests\/integration\/[\w-]+\.test\.js/g) || [];
/** This narrow guard catches the known executable launchers; it is not complete native-boundary enforcement. */
const prohibited = entries.filter((entry) =>
  /\/Applications\/Google Chrome\.app|\/usr\/bin\/(?:google-chrome|chromium)|chromium\.launch\(/.test(
    readFileSync(new URL(entry, root), 'utf8'),
  ),
);
if (!entries.length) throw new Error('Cannot inventory integration entries; review the native-only gate');
if (prohibited.length) {
  console.error(
    'Native migration required. Refusing to launch legacy non-Oya browser fixtures:\n' + prohibited.join('\n'),
  );
  process.exitCode = 1;
}
