/**
 * The public release notes: CHANGELOG.md from the repository root, read when the
 * site is built, so the page and the file in the code base never disagree.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import type { Metadata } from 'next';
import styles from '../page.module.css';
import notes from './release-notes.module.css';
import { SiteHeader } from '../_home/site-header';
import { SiteFooter } from '../_home/site-footer';
import { inlineOf, parseChangelog, type Inline, type Release } from './changelog';

/** Built once with the site: the changelog is read at build time, not per request. */
export const dynamic = 'force-static';

/** The page's title and description for search and link previews. */
export const metadata: Metadata = {
  title: 'Release notes',
  description: 'What changed in each Oya Browser release: the server, the desktop app, the SDK and the CLI.',
  alternates: { canonical: '/release-notes' },
};

/** The changelog at the repository root; the site is built from ui/, one folder down. */
const CHANGELOG = path.join(process.cwd(), '..', 'CHANGELOG.md');

/** One line's text with its bold, code and links. */
function Line({ line }: { /** The line as written. */ line: string }) {
  return (
    <>
      {inlineOf(line).map((part, i) => (
        <Part key={i} part={part} />
      ))}
    </>
  );
}

/** One piece of a line. */
function Part({ part }: { /** The piece. */ part: Inline }) {
  if (part.kind === 'bold') return <strong>{part.text}</strong>;
  if (part.kind === 'code') return <code>{part.text}</code>;
  if (part.kind === 'link') return <a href={part.href}>{part.text}</a>;
  return <>{part.text}</>;
}

/** One release: its heading, any notes, and its groups of changes. */
function ReleaseEntry({ release }: { /** The release. */ release: Release }) {
  return (
    <section className={notes.release}>
      <h2>
        <Line line={release.title} />
      </h2>
      {release.notes.map((note) => (
        <p key={note}>
          <Line line={note} />
        </p>
      ))}
      {release.groups.map((group) => (
        <div key={group.title} className={notes.group}>
          <h3>{group.title}</h3>
          <ul>
            {group.items.map((item) => (
              <li key={item}>
                <Line line={item} />
              </li>
            ))}
          </ul>
        </div>
      ))}
    </section>
  );
}

/** Whether a release says anything: an Unreleased with nothing under it yet is left out. */
const hasContent = (release: Release) => release.notes.length > 0 || release.groups.some((g) => g.items.length > 0);

/** The release notes page. */
export default function ReleaseNotes() {
  const releases = parseChangelog(readFileSync(CHANGELOG, 'utf8'));
  return (
    <div className={styles.page}>
      <div className={styles.container}>
        <SiteHeader />
        <main className={notes.main}>
          <h1>Release notes</h1>
          <p className={notes.lead}>
            The server, the desktop app, the SDK and the CLI ship together under one version.
          </p>
          {releases.filter(hasContent).map((release) => (
            <ReleaseEntry key={release.title} release={release} />
          ))}
        </main>
        <SiteFooter />
      </div>
    </div>
  );
}
