/** Address completion stays private, bounded, safe and useful without network access. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { BrowsingLibrary, addressSuggestions } from '../../../../src/main/library/index.ts';
import { ADDRESS_SUGGESTION_LIMIT, QUERY_MAX_LENGTH } from '../../../../src/main/library/constants.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

/** A real library over the existing in-memory settings fake. */
function setup() {
  const ctx = mainCtx();
  return { ctx, library: new BrowsingLibrary(ctx) };
}
it('matches URL prefixes and page titles, with prefix matches first', () => {
  const { library } = setup();
  library.visit('https://example.test/', 'Gmail guide');
  library.visit('https://gmail.test/inbox', 'Inbox');
  assert.deepEqual(
    addressSuggestions(library, 'GMAIL').map((item) => item.url),
    ['https://gmail.test/inbox', 'https://example.test/'],
  );
});
it('deduplicates bookmarks and history while preserving saved titles', () => {
  const { library } = setup();
  library.visit('https://example.test/', 'Page title');
  library.addBookmark('https://example.test/', 'Saved example');
  assert.deepEqual(addressSuggestions(library, 'example'), [
    { url: 'https://example.test/', title: 'Saved example', source: 'bookmark' },
  ]);
});
it('returns nothing for blank, invalid or oversized input', () => {
  const { library } = setup();
  library.visit('https://example.test/', 'Example');
  for (const input of ['', '  ', {}, null, 'x'.repeat(QUERY_MAX_LENGTH + 1)])
    assert.deepEqual(addressSuggestions(library, input), []);
});
it('caps the result count and preserves recent history order', () => {
  const { library } = setup();
  for (let i = 0; i < ADDRESS_SUGGESTION_LIMIT + 1; i++) library.visit(`https://example.test/${i}`, 'Example');
  const results = addressSuggestions(library, 'example');
  assert.equal(results.length, ADDRESS_SUGGESTION_LIMIT);
  assert.equal(results[0].url, `https://example.test/${ADDRESS_SUGGESTION_LIMIT}`);
});
it('does not expose another profile or unsafe addresses', () => {
  const { ctx, library } = setup();
  library.visit('https://private.test/', 'Private');
  ctx.persona.partitionName = () => 'another-profile';
  library.visit('javascript:alert(1)', 'Private');
  library.visit('https://user:secret@private.test/', 'Private');
  assert.deepEqual(addressSuggestions(library, 'private'), []);
});
