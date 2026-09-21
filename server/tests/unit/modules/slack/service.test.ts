/**
 * Unit tests for the Slack facade: it exposes the pieces the rest of the server
 * uses, and nothing else.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import * as slack from '../../../../src/modules/slack/service.ts';

describe('Slack facade', () => {
  it('exports the client, message, signature and both routers', () => {
    assert.deepEqual(Object.keys(slack).sort(), [
      'blocksFor',
      'call',
      'consoleUrl',
      'isDeadInstall',
      'oauthConfigured',
      'slackActionsRouter',
      'slackRouter',
      'verifySignature',
    ]);
  });
});
