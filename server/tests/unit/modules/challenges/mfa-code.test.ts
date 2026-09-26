/**
 * Unit tests for getting a one-time code: reading it out of a message (by the
 * tenant's LLM, with the pattern as fallback), out of a person's reply, and by
 * polling a relay or mailbox inside a bounded window.
 */
import { describe, it, afterEach, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { MAX_MESSAGE_CHARS, RELAY_POLL_MS } from '../../../../src/modules/challenges/constants.ts';
import { Status } from '../../../../src/platform/http-status.ts';
import { advance, json, stubFetch, text } from '../../support/http.ts';

// The poll loop sleeps through timers/promises, whose binding is fixed when first
// imported: importing with the clock mocked lets mock.timers drive it.
mock.timers.enable({ apis: ['setTimeout'] });
const { extractCode, codeInReply, fetchRelayCode } = await import('../../../../src/modules/challenges/mfa-code.ts');
const { clearTokens } = await import('../../../../src/modules/challenges/mailbox-token.ts');
mock.timers.reset();

/** A tenant LLM config. */
const LLM = { openaiKey: 'sk-test', baseUrl: 'https://llm.example.test/v1', model: 'm' };
/** A relay on a public IP literal, so the SSRF check needs no DNS. */
const RELAY = 'https://93.184.216.34/sms';

/** An OpenAI-shaped completion answering `content`. */
const completion = (content: string) => json({ choices: [{ message: { content } }] });

afterEach(() => mock.restoreAll());

describe('extractCode', () => {
  it('takes the first 4-8 digit run when there is no LLM', async () => {
    assert.equal(await extractCode('Ref 12, your code is 445566, valid 5 minutes'), '445566');
  });

  it('uses the caller’s pattern, preferring its first group', async () => {
    assert.equal(await extractCode('code: K7R4QP', { pattern: /code: (\w+)/ }), 'K7R4QP');
    assert.equal(await extractCode('XK-99', { pattern: /XK-\d+/ }), 'XK-99');
  });

  it('answers null when the message holds no code', async () => {
    assert.equal(await extractCode('Welcome back!'), null);
    assert.equal(await extractCode(null), null);
  });

  it('reads only the first stretch of a long message', async () => {
    assert.equal(await extractCode(`${'x'.repeat(MAX_MESSAGE_CHARS)} 445566`), null);
  });

  it('trusts an LLM answer shaped like a code, sending only the message', async () => {
    const calls = stubFetch(() => completion(' K7R4QP \n'));
    assert.equal(await extractCode('Your code is K7R4QP', { llm: LLM }), 'K7R4QP');
    assert.equal(calls[0].url, 'https://llm.example.test/v1/chat/completions');
    const body = JSON.parse(calls[0].init.body);
    assert.equal(body.messages[1].content, 'Your code is K7R4QP');
  });

  it('answers null when the LLM says there is no code', async () => {
    stubFetch(() => completion('NONE'));
    assert.equal(await extractCode('Your case 445566 is open', { llm: LLM }), null);
  });

  it('falls back to the pattern when the LLM answers with prose', async () => {
    stubFetch(() => completion('The code in this message is 445566.'));
    assert.equal(await extractCode('code 445566', { llm: LLM }), '445566');
  });

  it('falls back to the pattern when the LLM call fails', async () => {
    mock.method(console, 'error', () => {});
    stubFetch(() => text('down', 503));
    assert.equal(await extractCode('code 445566', { llm: LLM }), '445566');
  });
});

describe('codeInReply', () => {
  it('takes a reply that is only the code', () => {
    assert.equal(codeInReply(' 445566 '), '445566');
  });

  it('finds the code inside a sentence', () => {
    assert.equal(codeInReply('the code is K7R4QP, thanks'), 'K7R4QP');
  });

  it('never mistakes a word without a digit for a code', () => {
    assert.equal(codeInReply('done'), null);
    assert.equal(codeInReply('finished it myself'), null);
    assert.equal(codeInReply(undefined), null);
  });
});

describe('fetchRelayCode', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout', 'Date'], now: 1_000_000 }));
  afterEach(() => mock.timers.reset());

  it('returns the code the relay answers with', async () => {
    const calls = stubFetch(() => text('Your code: 445566'));
    const code = await fetchRelayCode({ type: 'sms', url: RELAY, headers: { 'x-token': 't' } });
    assert.equal(code, '445566');
    assert.equal(calls[0].init.headers['x-token'], 't');
    assert.equal(calls[0].init.redirect, 'error', 'a redirect cannot lead the fetch somewhere unchecked');
  });

  it('skips a timestamped message older than the login, and waits for the new one', async () => {
    const answers = [
      text('old 111111', 200, { 'x-oya-received-at': '999' }),
      text('new 222222', 200, { 'x-oya-received-at': '1000001' }),
    ];
    stubFetch(() => answers.shift()!);
    const pending = fetchRelayCode({ type: 'sms', url: RELAY }, 1_000_000);
    await advance(RELAY_POLL_MS);
    assert.equal(await pending, '222222');
  });

  it('keeps polling through a relay error', async () => {
    const answers = [text('', 500), text('code 333333')];
    stubFetch(() => answers.shift()!);
    const pending = fetchRelayCode({ type: 'email', url: RELAY });
    await advance(RELAY_POLL_MS);
    assert.equal(await pending, '333333');
  });

  it('gives up with 504 once the window closes', async () => {
    stubFetch(() => text('nothing yet'));
    const pending = fetchRelayCode({ type: 'sms', url: RELAY, timeoutMs: RELAY_POLL_MS * 2 });
    const outcome = assert.rejects(pending, { status: Status.GATEWAY_TIMEOUT });
    await advance(RELAY_POLL_MS, 2);
    await outcome;
  });

  it('never fetches a relay that points at a private address', async () => {
    const calls = stubFetch(() => text('code 445566'));
    const pending = fetchRelayCode({ type: 'sms', url: 'http://127.0.0.1/sms', timeoutMs: RELAY_POLL_MS });
    const outcome = assert.rejects(pending, { status: Status.GATEWAY_TIMEOUT });
    await advance(RELAY_POLL_MS);
    await outcome;
    assert.equal(calls.length, 0);
  });

  it('reads the code from a mailbox factor', async () => {
    clearTokens();
    stubFetch((url) => {
      if (url.includes('oauth2')) return json({ access_token: 'at' });
      if (url.includes('/messages?')) return json({ messages: [{ id: 'm1' }] });
      return json({ internalDate: '1000500', payload: { mimeType: 'text/plain', body: { data: b64('code 777777') } } });
    });
    const config = { type: 'gmail', refreshToken: 'rt-code', clientId: 'c' };
    assert.equal(await fetchRelayCode(config, 1_000_000), '777777');
  });

  it('takes a mailbox code dated to the whole second before the login it answers', async () => {
    // Sent while the login post was handled: Gmail dates it 08:00:26.000, the login 08:00:26.400.
    clearTokens();
    const calls = stubFetch((url) => {
      if (url.includes('oauth2')) return json({ access_token: 'at' });
      if (url.includes('/messages?')) return json({ messages: [{ id: 'm1' }] });
      return json({ internalDate: '1000000', payload: { mimeType: 'text/plain', body: { data: b64('code 872051') } } });
    });
    const config = { type: 'gmail', refreshToken: 'rt-second', clientId: 'c' };
    assert.equal(await fetchRelayCode(config, 1_000_400), '872051');
    assert.match(new URL(calls[1].url).searchParams.get('q') ?? '', /after:995\b/);
  });

  it('stops at once when the mailbox refuses its refresh token', async () => {
    clearTokens();
    const calls = stubFetch(() => text('{"error":"invalid_grant"}', 400));
    const config = { type: 'graph', refreshToken: 'rt-revoked', clientId: 'c' };
    await assert.rejects(fetchRelayCode(config), { status: Status.BAD_GATEWAY, message: /invalid_grant/ });
    assert.equal(calls.length, 1);
  });
});

/** base64url, as Gmail encodes bodies. */
function b64(value: string) {
  return Buffer.from(value).toString('base64url');
}
