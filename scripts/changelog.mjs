/**
 * The changelog at release time: what the release script asks of CHANGELOG.md.
 *
 *   node scripts/changelog.mjs check            refuse a changelog with no Unreleased section
 *   node scripts/changelog.mjs stamp <version>  Unreleased becomes the version, linked to its release
 *   node scripts/changelog.mjs notes <version>  that version's notes, for the tag and the GitHub release
 *
 * A release's notes are written once, under Unreleased, as the work lands; the tag,
 * the GitHub release and the website all carry those same words. A release can be
 * cut with nothing written there: it then says "Maintenance and fixes."
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

/** Where a version's GitHub release lives. */
export const RELEASES = 'https://github.com/OyadotAI/oya-browser/releases/tag';
/** The public page the same notes are published on. */
export const NOTES_PAGE = 'https://oyabrowser.com/release-notes';
/** The heading work in progress is written under. */
const UNRELEASED = '## Unreleased';
/** The start of any release heading. */
const RELEASE = '## ';

/** The line range [start, end) of the section whose heading starts with `heading`, or null. */
function sectionOf(lines, matches) {
  const start = lines.findIndex(matches);
  if (start < 0) return null;
  const next = lines.findIndex((line, i) => i > start && line.startsWith(RELEASE));
  return [start, next < 0 ? lines.length : next];
}

/** Whether a section holds at least one change: an item under a group. */
const hasChanges = (body) => body.some((line) => line.startsWith('- '));

/** The Unreleased section's lines, without its heading, or null when there is none. */
export function unreleased(markdown) {
  const lines = markdown.split('\n');
  const range = sectionOf(lines, (line) => line.trim() === UNRELEASED);
  return range && lines.slice(range[0] + 1, range[1]);
}

/** Why a release cannot be cut from this changelog, or null when it can. */
export function refusal(markdown) {
  return unreleased(markdown) ? null : `CHANGELOG.md has no "${UNRELEASED}" section.`;
}

/** What a release says when nothing was written under Unreleased: a release can be cut any time. */
export const NO_NOTES = '### Changed\n\n- Maintenance and fixes.';

/** The changelog with a default note under an empty Unreleased, so every release says something. */
const withNotes = (markdown) =>
  hasChanges(unreleased(markdown)) ? markdown : markdown.replace(UNRELEASED, `${UNRELEASED}\n\n${NO_NOTES}`);

/** The heading a released version gets: its number linked to its GitHub release, and the date. */
export const releaseHeading = (version, date) => `## [${version}](${RELEASES}/v${version}) · ${date}`;

/** The changelog with Unreleased turned into `version`, and a fresh, empty Unreleased above it. */
export function stamp(markdown, version, date) {
  const why = refusal(markdown);
  if (why) throw new Error(why);
  return withNotes(markdown).replace(UNRELEASED, `${UNRELEASED}\n\n${releaseHeading(version, date)}`);
}

/** One version's notes, as the tag and the GitHub release carry them, with a link back to all of them. */
export function notesFor(markdown, version) {
  const lines = markdown.split('\n');
  const range = sectionOf(lines, (line) => line.startsWith(`${RELEASE}[${version}]`));
  if (!range) throw new Error(`CHANGELOG.md has no section for ${version}.`);
  const body = lines.slice(range[0] + 1, range[1]).join('\n').trim();
  return `${body}\n\nAll release notes: ${NOTES_PAGE}\n`;
}

/** Today, as the changelog writes dates. */
const today = () => new Date().toISOString().slice(0, 10);

/** Each command, by name. */
const COMMANDS = {
  check: (file) => {
    const why = refusal(readFileSync(file, 'utf8'));
    if (why) throw new Error(why);
  },
  stamp: (file, version) => writeFileSync(file, stamp(readFileSync(file, 'utf8'), version, today())),
  notes: (file, version) => process.stdout.write(notesFor(readFileSync(file, 'utf8'), version)),
};

/** Runs a command against CHANGELOG.md at the repository root. */
function main([command, version]) {
  if (!Object.hasOwn(COMMANDS, command)) throw new Error('Usage: changelog.mjs check | stamp <version> | notes <version>');
  if (command !== 'check' && !version) throw new Error(`changelog.mjs ${command} needs a version`);
  COMMANDS[command](fileURLToPath(new URL('../CHANGELOG.md', import.meta.url)), version);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (err) {
    console.error(err.message);
    process.exit(1);
  }
}
