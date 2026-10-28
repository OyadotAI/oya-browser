/** Generate ICU preferred time-zone names for the embedded IANA database. */
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

if (process.versions.icu !== '78.3' || process.versions.unicode !== '17.0') {
  throw new Error('Generate with ICU 78.3 and Unicode 17.0.');
}
const source = readFileSync(fileURLToPath(new URL('../src/v9/builtins/temporal/zones.zi', import.meta.url)), 'utf8');
const names = new Set();
for (const line of source.split('\n')) {
  const fields = line.split(/\s+/);
  if (fields[0] === 'Z') names.add(fields[1]);
  if (fields[0] === 'L') names.add(fields[2]);
}
const entries = [];
for (const name of [...names].sort()) {
  try {
    entries.push(`${name.toLowerCase()}\t${new Intl.DateTimeFormat('en', { timeZone: name }).resolvedOptions().timeZone}`);
  } catch (error) {
    if (!(error instanceof RangeError)) throw error;
  }
}
const target = fileURLToPath(new URL('../src/v9/builtins/intl/zone_names.txt', import.meta.url));
writeFileSync(target, `# Generated with ICU 78.3 / Unicode 17.0; see UPSTREAM.md\n${entries.join('\n')}\n`);
console.log(`Generated ${entries.length} of ${names.size} IANA identifiers.`);
