/** Agent status compares structured endpoint data; stale tokens and matching comments cannot turn a light green. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { codexEndpoint, matchesEndpoint } from '../../../../src/main/local-mcp/config-match.ts';

const endpoint = { url: 'http://127.0.0.1:47321/mcp', token: 'synthetic-current-token' };
const own =
  '[mcp_servers.oya-browser]\nurl = "http://127.0.0.1:47321/mcp"\nhttp_headers = { Authorization = "Bearer synthetic-current-token" }\n';
it('only the exact current endpoint and credential is configured for this browser', () => {
  assert(matchesEndpoint(codexEndpoint(own), endpoint));
  assert(!matchesEndpoint(codexEndpoint(own.replace('47321', '49999')), endpoint));
  assert(!matchesEndpoint(codexEndpoint(own.replace('synthetic-current-token', 'other-profile-token')), endpoint));
  assert(!matchesEndpoint({ url: endpoint.url, headers: {} }, endpoint));
});
it('quoted tables, literal strings and native header subtables are read structurally', () => {
  const text =
    '[mcp_servers."oya-browser"]\nurl = \'http://127.0.0.1:47321/mcp\' # kept\n[mcp_servers."oya-browser".http_headers]\nAuthorization = \'Bearer synthetic-current-token\'\n';
  assert(matchesEndpoint(codexEndpoint(text), endpoint));
});
it('quoted dots and malformed separators cannot alias the nested server table', () => {
  for (const table of ['["mcp_servers.oya-browser"]', '[mcp_servers..oya-browser]', '[mcp_servers oya-browser]'])
    assert.equal(codexEndpoint(own.replace('[mcp_servers.oya-browser]', table)), null);
});
it('comments, duplicate fields and unsupported multiline syntax fail closed', () => {
  for (const text of [
    own
      .split('\n')
      .map((line) => '# ' + line)
      .join('\n'),
    own + 'url = "other"',
    'prompt = """\n' + own + '"""\n',
  ])
    assert.equal(codexEndpoint(text), null);
});
