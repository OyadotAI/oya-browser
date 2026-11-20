/**
 * Unit tests for the saved config (src/config.ts): owner-only writes, merges,
 * and flags/environment beating the file.
 */
import './support/harness.ts';
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { lstatSync, readFileSync, statSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { load, save, resolved, origin, configPath } from '../../src/config.ts';

describe('config', () => {
  afterEach(() => {
    delete process.env.OYA_API_KEY;
    delete process.env.OYA_BASE_URL;
  });

  it('reads nothing when there is no file, or it does not parse', () => {
    assert.deepEqual(load(), {});
    writeFileSync(configPath, 'not json');
    assert.deepEqual(load(), {});
  });

  it('merges into the saved file, keeps the file and folder owner-only, and never writes through a symlink', () => {
    writeFileSync(configPath, '{}', { mode: 0o644 });
    save({ apiKey: 'a' });
    save({ baseUrl: 'https://b' });
    assert.deepEqual(load(), { apiKey: 'a', baseUrl: 'https://b' });
    assert.equal(statSync(configPath).mode & 0o777, 0o600);
    assert.equal(statSync(dirname(configPath)).mode & 0o777, 0o700);
    const elsewhere = join(dirname(configPath), 'elsewhere.json');
    writeFileSync(elsewhere, '{}');
    unlinkSync(configPath);
    symlinkSync(elsewhere, configPath);
    save({ apiKey: 'secret' });
    assert.equal(load().apiKey, 'secret');
    assert.equal(lstatSync(configPath).isSymbolicLink(), false);
    assert.equal(readFileSync(elsewhere, 'utf8'), '{}');
  });

  it('prefers the environment, and trims trailing slashes from the URL', () => {
    save({ apiKey: 'saved', baseUrl: 'https://saved' });
    process.env.OYA_API_KEY = 'env';
    process.env.OYA_BASE_URL = 'https://env//';
    assert.deepEqual(resolved(), { apiKey: 'env', baseUrl: 'https://env' });
  });

  it('falls back to the hosted control plane', () => {
    writeFileSync(configPath, '{}');
    assert.deepEqual(resolved(), { apiKey: '', baseUrl: 'https://oyabrowser.com' });
  });

  it('sends the saved key only to the control plane it was saved for', () => {
    save({ apiKey: 'saved', baseUrl: 'https://saved' });
    process.env.OYA_BASE_URL = 'https://other';
    const found = origin();
    assert.deepEqual([found.apiKey, found.apiKeyFrom, found.savedKey], ['', 'none', true]);
  });

  it('reads one address with different spellings as the same control plane', () => {
    save({ apiKey: 'saved', baseUrl: 'https://Saved:443/' });
    process.env.OYA_BASE_URL = 'https://saved';
    assert.equal(origin().apiKey, 'saved');
  });

  it('keeps a saved key with no address, as older files have', () => {
    writeFileSync(configPath, JSON.stringify({ apiKey: 'old' }));
    process.env.OYA_BASE_URL = 'https://anywhere';
    assert.equal(origin().apiKey, 'old');
  });
});
