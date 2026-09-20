/**
 * Unit tests for reading a one-time code out of a mailbox: Gmail and Microsoft
 * Graph through a stubbed fetch, the `since` window that keeps a previous
 * run's code out, and the HTML and MIME handling of message bodies.
 */
import { describe, it, afterEach, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  decodeBase64Url,
  gmail,
  gmailBody,
  graph,
  htmlToText,
  reset,
} from '../../../../src/modules/challenges/inbox.ts';
import { Status } from '../../../../src/platform/http-status.ts';
import { json, stubFetch } from '../../support/http.ts';

/** base64url, as Gmail encodes bodies. */
const b64 = (value: string) => Buffer.from(value).toString('base64url');
/** A mailbox factor config. */
const CONFIG = { refreshToken: 'rt-inbox', clientId: 'cid' };

/** Answers the token endpoint, then hands the rest to `mail`. */
function mailbox(mail: (url: string) => Response) {
  return stubFetch((url) => (url.includes('oauth2') ? json({ access_token: 'at' }) : mail(url)));
}

describe('htmlToText', () => {
  it('drops scripts, styles and tags, decodes entities and squeezes whitespace', () => {
    const html =
      '<style>p{}</style><p>Code:&nbsp;<b>445566</b></p>\n<script>x()</script> &lt;ok&gt; &amp; &quot;q&quot; &#39;s&#39;';
    assert.equal(htmlToText(html), 'Code: 445566 <ok> & "q" \'s\'');
  });
});

describe('decodeBase64Url', () => {
  it('decodes Gmail’s URL-safe base64 to UTF-8', () => {
    assert.equal(decodeBase64Url(b64('héllo ?>')), 'héllo ?>');
  });
});

describe('gmailBody', () => {
  it('prefers the plain-text part anywhere in the tree', () => {
    const payload = {
      mimeType: 'multipart/alternative',
      parts: [
        { mimeType: 'text/html', body: { data: b64('<p>html 1</p>') } },
        {
          mimeType: 'multipart/related',
          parts: [{ mimeType: 'text/plain; charset=utf-8', body: { data: b64('plain 2') } }],
        },
      ],
    };
    assert.equal(gmailBody(payload), 'plain 2');
  });

  it('falls back to the HTML part as text', () => {
    assert.equal(gmailBody({ mimeType: 'text/html', body: { data: b64('<b>445566</b>') } }), '445566');
  });

  it('falls back to any body, and to empty without a payload', () => {
    assert.equal(gmailBody({ body: { data: b64('raw') } }), 'raw');
    assert.equal(gmailBody(null), '');
  });
});

describe('gmail', () => {
  beforeEach(() => reset());
  afterEach(() => mock.restoreAll());

  it('returns the newest message after `since`, with its subject', async () => {
    const calls = mailbox((url) =>
      url.includes('?maxResults')
        ? json({ messages: [{ id: 'm1' }] })
        : json({
            internalDate: '5000',
            payload: {
              headers: [{ name: 'Subject', value: 'Your code' }],
              mimeType: 'text/plain',
              body: { data: b64('445566') },
            },
          }),
    );
    assert.deepEqual(await gmail({ ...CONFIG, query: 'from:portal' }, 4000), { text: 'Your code\n445566', at: 5000 });
    const list = new URL(calls[1].url);
    assert.equal(list.searchParams.get('q'), 'from:portal after:4');
    assert.equal(calls[1].init.headers.Authorization, 'Bearer at');
    assert.match(calls[2].url, /\/messages\/m1\?format=full$/);
  });

  it('skips a message older than `since`, which belongs to the previous run', async () => {
    mailbox((url) =>
      url.includes('?maxResults')
        ? json({ messages: [{ id: 'old' }, { id: 'unreadable' }] })
        : url.includes('/old?')
          ? json({ internalDate: '3999', payload: { body: { data: b64('111111') } } })
          : json({}, Status.NOT_FOUND),
    );
    assert.equal(await gmail(CONFIG, 4000), null);
  });

  it('fails with 502 when the message list cannot be read', async () => {
    mailbox(() => json({}, Status.UNAUTHORIZED));
    await assert.rejects(gmail(CONFIG), { status: Status.BAD_GATEWAY, message: 'Gmail list failed (401)' });
  });
});

describe('graph', () => {
  beforeEach(() => reset());
  afterEach(() => mock.restoreAll());

  /** A Graph message. */
  const message = (receivedDateTime: string, content: string, contentType = 'text', subject = 'Sign in') => ({
    subject,
    receivedDateTime,
    body: { contentType, content },
  });

  it('returns the newest message after `since`, asking only for that window', async () => {
    const calls = mailbox(() =>
      json({ value: [message('1970-01-01T00:00:05Z', '<p>Code <b>445566</b></p>', 'html')] }),
    );
    assert.deepEqual(await graph(CONFIG, 4000), { text: 'Sign in\nCode 445566', at: 5000 });
    assert.match(decodeURIComponent(calls[1].url), /receivedDateTime ge 1970-01-01T00:00:04\.000Z/);
  });

  it('skips messages older than `since` and ones that do not match the query', async () => {
    mailbox(() =>
      json({
        value: [message('1970-01-01T00:00:01Z', 'old 111111'), message('1970-01-01T00:00:06Z', 'newsletter 222222')],
      }),
    );
    assert.equal(await graph({ ...CONFIG, query: 'VERIFY' }, 4000), null);
  });

  it('matches the query case-insensitively against subject and body', async () => {
    mailbox(() => json({ value: [message('1970-01-01T00:00:06Z', 'code 333333', 'text', 'Verify your login')] }));
    assert.equal((await graph({ ...CONFIG, query: 'verify' }, 0))?.text, 'Verify your login\ncode 333333');
  });

  it('fails with 502 when the message list cannot be read', async () => {
    mailbox(() => json({}, Status.FORBIDDEN));
    await assert.rejects(graph(CONFIG), { status: Status.BAD_GATEWAY, message: 'Graph list failed (403)' });
  });
});
