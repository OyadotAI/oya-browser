/**
 * Unit tests for the changelog at release time: a release refused with nothing to
 * say, Unreleased stamped with the version and its link, and one version's notes.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { notesFor, refusal, releaseHeading, stamp } from './changelog.mjs';

const WITH_CHANGES = `# Changelog

Intro.

## Unreleased

### Fixed

- A fix.

## [1.0.1](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.1) · 2026-01-01

### Added

- Older thing.
`;

describe('changelog', () => {
  it('refuses a release with nothing listed under Unreleased, or no Unreleased at all', () => {
    assert.match(refusal('# Changelog\n\n## Unreleased\n\n### Fixed\n'), /lists nothing under/);
    assert.match(refusal('# Changelog\n'), /has no "## Unreleased" section/);
    assert.equal(refusal(WITH_CHANGES), null);
  });

  it('stamps Unreleased with the version linked to its release, leaving an empty Unreleased above', () => {
    const out = stamp(WITH_CHANGES, '1.0.2', '2026-02-02');
    assert.match(
      out,
      /## Unreleased\n\n## \[1\.0\.2\]\(https:\/\/github\.com\/OyadotAI\/oya-browser\/releases\/tag\/v1\.0\.2\) · 2026-02-02\n\n### Fixed\n\n- A fix\./,
    );
    assert.equal(refusal(out) !== null, true, 'the new Unreleased is empty until the next change');
  });

  it('refuses to stamp a changelog with nothing to release', () => {
    assert.throws(() => stamp('## Unreleased\n', '1.0.2', '2026-02-02'), /lists nothing/);
  });

  it('gives one version its own notes, ending with a link to all of them', () => {
    const out = stamp(WITH_CHANGES, '1.0.2', '2026-02-02');
    assert.equal(notesFor(out, '1.0.2'), '### Fixed\n\n- A fix.\n\nAll release notes: https://oyabrowser.com/release-notes\n');
    assert.match(notesFor(out, '1.0.1'), /- Older thing\./);
    assert.throws(() => notesFor(out, '9.9.9'), /no section for 9\.9\.9/);
  });

  it('writes a release heading as the version linked to its GitHub release, and the date', () => {
    assert.equal(
      releaseHeading('1.2.3', '2026-03-03'),
      '## [1.2.3](https://github.com/OyadotAI/oya-browser/releases/tag/v1.2.3) · 2026-03-03',
    );
  });
});
