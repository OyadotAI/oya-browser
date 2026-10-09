/** Bearer credentials cannot escape into URLs, serialized state, or cleartext remote handshakes. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CDPConnection } from '../../../../src/drivers/cdp/connection.ts';
import { connectionHeaders } from '../../../../src/drivers/cdp/connection-auth.ts';

test('valid bearer tokens are attached only as authorization headers', () => {
  for (const url of ['ws://127.0.0.1:1234', 'ws://[::1]:1234', 'wss://oya.example/door']) {
    assert.deepEqual(connectionHeaders(url, { bearerToken: 'test-token' }), { Authorization: 'Bearer test-token' });
    assert.ok(!JSON.stringify(new CDPConnection(url, { bearerToken: 'test-token' })).includes('test-token'));
  }
});

test('malformed and oversized bearer tokens fail before opening a socket', () => {
  for (const bearerToken of ['', 'with spaces', 'secret\r\nX: injected', 'x'.repeat(4097), null, 42]) {
    assert.throws(
      () => new CDPConnection('ws://127.0.0.1', { bearerToken: bearerToken as string }),
      /Invalid front-door bearer token/,
    );
  }
});

test('bearer authentication refuses network cleartext and credential-bearing endpoints', () => {
  for (const url of [
    'ws://remote.test',
    'ws://localhost',
    'https://remote.test',
    'wss://user:secret@remote.test',
    'wss://remote.test?token=secret',
    'wss://remote.test#secret',
    'not a url',
  ]) {
    assert.throws(
      () => new CDPConnection(url, { bearerToken: 'secret' }),
      (error: Error) => {
        assert.ok(!error.message.includes('secret'));
        return true;
      },
    );
  }
});

test('existing credential-free connections retain their endpoint behavior', () => {
  assert.deepEqual(connectionHeaders('ws://remote.test', {}), {});
  assert.equal(new CDPConnection('ws://remote.test').url, 'ws://remote.test');
});
