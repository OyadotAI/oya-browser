/**
 * Unit tests for the second-factor draft: when it is ready to store, and the
 * body each factor type sends.
 */
import { describe, it, expect } from 'vitest';
import { mfaBody, mfaHint, mfaReady, newMfa } from '@/components/dashboard/personas/mfa';

describe('mfa drafts', () => {
  it('starts empty, as TOTP unless told otherwise', () => {
    expect(newMfa()).toEqual({ type: 'totp', value: '', clientId: '', clientSecret: '', tenant: '', domain: '' });
    expect(newMfa('').type).toBe('');
  });

  it('is not ready without a type or a value', () => {
    expect(mfaReady({ ...newMfa(''), value: 'x' })).toBe(false);
    expect(mfaReady({ ...newMfa('totp'), value: '   ' })).toBe(false);
    expect(mfaReady({ ...newMfa('totp'), value: 'SEED' })).toBe(true);
  });

  it('needs an OAuth client for a mailbox factor', () => {
    expect(mfaReady({ ...newMfa('gmail'), value: 'tok' })).toBe(false);
    expect(mfaReady({ ...newMfa('gmail'), value: 'tok', clientId: 'cid' })).toBe(true);
  });

  it('sends a TOTP seed as a trimmed secret', () => {
    expect(mfaBody({ ...newMfa('totp'), value: ' SEED ' })).toEqual({ type: 'totp', secret: 'SEED' });
  });

  it('sends a relay as a URL, scoped to a site when one is given', () => {
    expect(mfaBody({ ...newMfa('sms'), value: 'https://r', domain: ' a.com ' })).toEqual({
      domain: 'a.com',
      type: 'sms',
      url: 'https://r',
    });
  });

  it('sends a mailbox with its client, and the secret only when given', () => {
    expect(mfaBody({ ...newMfa('gmail'), value: 't', clientId: 'c' })).toEqual({
      type: 'gmail',
      refreshToken: 't',
      clientId: 'c',
    });
    expect(mfaBody({ ...newMfa('gmail'), value: 't', clientId: 'c', clientSecret: 's', tenant: 'x' })).toEqual({
      type: 'gmail',
      refreshToken: 't',
      clientId: 'c',
      clientSecret: 's',
    });
  });

  it('sends a tenant only for Microsoft 365', () => {
    expect(mfaBody({ ...newMfa('graph'), value: 't', clientId: 'c', tenant: 'org' })).toMatchObject({ tenant: 'org' });
  });

  it('has a hint for every type and none for no type', () => {
    expect(mfaHint('totp')?.secret).toBe(true);
    expect(mfaHint('email')?.label).toBe('Relay URL');
    expect(mfaHint('')).toBeUndefined();
  });
});
