/** Which sentence each result shows. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Messages, errorMessage, keyRefusedMessage, keySavedMessage, sentMessage } from '../../src/messages.js';
import { OyaError } from '../../src/oya.js';

test('each refusal status has its own message', () => {
  assert.equal(errorMessage(new OyaError(401, 'expired')), Messages.expired);
  assert.equal(errorMessage(new OyaError(410, '')), Messages.gone);
  assert.equal(errorMessage(new OyaError(429, '')), Messages.busy);
  assert.equal(errorMessage(new OyaError(0, '')), Messages.offline);
});

test('a 403 for an unknown key differs from a 403 for a read-only key', () => {
  assert.equal(errorMessage(new OyaError(403, 'Invalid API key')), Messages.unknownKey);
  assert.equal(errorMessage(new OyaError(403, 'Viewer credentials cannot change resources')), Messages.viewer);
});

test('a server failure says Oya had a problem, not that it was unreachable', () => {
  assert.equal(errorMessage(new OyaError(502, '')), Messages.failed);
  assert.equal(errorMessage(new OyaError(500, 'boom')), Messages.failed);
});

test('an error that never reached Oya says it could not be reached', () => {
  assert.equal(errorMessage(new Error('boom')), Messages.offline);
});

test('a 400 means too many logins only when the server names its cap', () => {
  assert.equal(errorMessage(new OyaError(400, 'Send { "cookies": [...] }')), Messages.failed);
});

test('a full import names the count and the profile', () => {
  assert.equal(
    sentMessage({ imported: 12, skipped: 0 }, 'Work').text,
    '12 logins sent to Work. Open the site in Oya and you are signed in.',
  );
});

test('a partial import says how many were skipped', () => {
  assert.equal(sentMessage({ imported: 9, skipped: 3 }, 'Work').text, '9 sent, 3 skipped (expired or unusable).');
});

test('saving a key reports profiles found, or why it was refused', () => {
  assert.equal(keySavedMessage(3), 'Key saved. 3 profiles found.');
  assert.equal(keySavedMessage(1), 'Key saved. 1 profile found.');
  assert.equal(keyRefusedMessage(new OyaError(403, '')), 'Oya did not accept this key.');
  assert.equal(keyRefusedMessage(new OyaError(0, '')), 'Could not reach Oya. Check your connection.');
});

test('a deleted profile and an oversized site each say what happened', () => {
  assert.equal(errorMessage(new OyaError(404, 'Persona not found')), Messages.noProfile);
  assert.equal(errorMessage(new OyaError(413, '')), Messages.tooMany);
  assert.equal(errorMessage(new OyaError(400, 'At most 20000 cookies per import')), Messages.tooMany);
});
