/** The setup hint is navigation only and preserves existing claim, connection and MFA priorities. */
import { afterEach, describe, expect, it } from 'vitest';
import { afterSignIn } from '@/components/auth/use-auth-form';
import { pendingMarketplace, forgetMarketplace } from '@/lib/marketplace';
import { keepStepUp } from '@/lib/auth/storage';

afterEach(() => {
  forgetMarketplace();
  sessionStorage.clear();
});

describe('marketplace authentication continuation', () => {
  it('recognizes only the exact nonsecret hint and can clear it', () => {
    document.cookie = 'oya_aws_pending=1; Path=/';
    expect(pendingMarketplace()).toBe(true);
    expect(afterSignIn()).toBe('/marketplace/aws');
    forgetMarketplace();
    expect(pendingMarketplace()).toBe(false);
    expect(afterSignIn()).toBe('/dashboard');
  });

  it('preserves claim and connection priority and wraps the same destination in MFA', () => {
    document.cookie = 'oya_aws_pending=1; Path=/';
    sessionStorage.setItem('oya_pending_connect', 'request');
    expect(afterSignIn()).toBe('/connect');
    sessionStorage.setItem('oya_pending_claim', 'key');
    expect(afterSignIn()).toBe('/claim');
    sessionStorage.clear();
    keepStepUp(true);
    expect(afterSignIn()).toBe('/account/mfa?next=%2Fmarketplace%2Faws');
  });
});
