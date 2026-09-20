/**
 * Unit tests for the Oya Cloud settings: the documented OYA_CLOUD_* names with
 * the DAYTONA_* fallback, the idle-stop floor, and naming exactly what is missing.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  displayName,
  isConfigured,
  missingSettings,
  ownerTag,
  settings,
  unconfigured,
} from '../../../../src/drivers/sandbox/config.ts';
import { MIN_SANDBOX_TTL_MINUTES, DEFAULT_SANDBOX_TTL_MINUTES } from '../../../../src/drivers/constants.ts';
import { Status } from '../../../../src/platform/http-status.ts';

/** A complete configuration. */
const ENV = { OYA_CLOUD_API_KEY: 'k', OYA_CLOUD_SNAPSHOT: 'snap', OYA_PUBLIC_WS_URL: 'wss://oya.example/ws' };

describe('settings', () => {
  it('reads a complete configuration with its defaults', () => {
    assert.deepEqual(settings(ENV), {
      apiKey: 'k',
      snapshot: 'snap',
      wsUrl: 'wss://oya.example/ws',
      apiUrl: null,
      target: 'us',
      ttlMinutes: DEFAULT_SANDBOX_TTL_MINUTES,
    });
    assert.equal(isConfigured(ENV), true);
  });

  it('falls back to the legacy DAYTONA_* names, the documented ones winning', () => {
    const env = { DAYTONA_API_KEY: 'old', DAYTONA_SNAPSHOT: 's', DAYTONA_TARGET: 'eu', OYA_PUBLIC_WS_URL: 'wss://x' };
    assert.deepEqual([settings(env)!.apiKey, settings(env)!.target], ['old', 'eu']);
    assert.equal(settings({ ...env, OYA_CLOUD_API_KEY: 'new' })!.apiKey, 'new');
  });

  it('never lets the idle stop go below the floor', () => {
    assert.equal(settings({ ...ENV, OYA_CLOUD_SANDBOX_TTL_MINUTES: '1' })!.ttlMinutes, MIN_SANDBOX_TTL_MINUTES);
    assert.equal(settings({ ...ENV, OYA_CLOUD_SANDBOX_TTL_MINUTES: '30' })!.ttlMinutes, 30);
  });

  it('is not configured without all three of key, snapshot and public URL', () => {
    for (const name of Object.keys(ENV)) {
      const env: Record<string, string> = { ...ENV };
      delete env[name];
      assert.equal(settings(env), null, name);
    }
  });
});

describe('missingSettings and unconfigured', () => {
  it('names only what is missing, under the documented names', () => {
    assert.deepEqual(missingSettings({ DAYTONA_API_KEY: 'k' }), ['OYA_CLOUD_SNAPSHOT', 'OYA_PUBLIC_WS_URL']);
    assert.deepEqual(missingSettings(ENV), []);
  });

  it('answers 409, hinting at a tunnel when the public URL is the one missing', () => {
    const err = unconfigured({ OYA_CLOUD_API_KEY: 'k', OYA_CLOUD_SNAPSHOT: 's' });
    assert.equal(err.status, Status.CONFLICT);
    assert.match(err.message, /^Cloud browsers need OYA_PUBLIC_WS_URL, which is not set\. .*tunnel/);
    assert.equal(
      unconfigured({ OYA_PUBLIC_WS_URL: 'wss://x' }).message,
      'Cloud browsers need OYA_CLOUD_API_KEY, OYA_CLOUD_SNAPSHOT, which are not set.',
    );
  });
});

describe('naming', () => {
  it('tags an owner with a stable digest, never the key', () => {
    assert.match(ownerTag('secret-key'), /^[0-9a-f]{32}$/);
    assert.equal(ownerTag('secret-key'), ownerTag('secret-key'));
    assert.notEqual(ownerTag('secret-key'), ownerTag('other-key'));
  });

  it('names a browser as given, or after its id', () => {
    assert.equal(displayName('Mine', 'abcdef123456'), 'Mine');
    assert.equal(displayName('', 'abcdef123456'), 'Cloud browser abcdef12');
  });
});
