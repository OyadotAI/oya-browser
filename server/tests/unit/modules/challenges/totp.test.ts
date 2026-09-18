/**
 * Unit tests for totp(): the RFC 6238 appendix B test vectors for SHA-1,
 * SHA-256 and SHA-512, plus how the base32 secret is read.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { totp } from '../../../../src/modules/challenges/totp.ts';
import { Status } from '../../../../src/platform/http-status.ts';

/** The RFC's seeds ("1234567890" repeated to the hash's key length), base32-encoded. */
const SEEDS = {
  sha1: 'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ',
  sha256: 'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA',
  sha512: 'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNA',
};

/** RFC 6238 appendix B: unix time -> eight-digit code, per algorithm. */
const VECTORS: [number, Record<keyof typeof SEEDS, string>][] = [
  [59, { sha1: '94287082', sha256: '46119246', sha512: '90693936' }],
  [1111111109, { sha1: '07081804', sha256: '68084774', sha512: '25091201' }],
  [1111111111, { sha1: '14050471', sha256: '67062674', sha512: '99943326' }],
  [1234567890, { sha1: '89005924', sha256: '91819424', sha512: '93441116' }],
  [2000000000, { sha1: '69279037', sha256: '90698825', sha512: '38618901' }],
  [20000000000, { sha1: '65353130', sha256: '77737706', sha512: '47863826' }],
];

describe('totp', () => {
  for (const algorithm of Object.keys(SEEDS) as (keyof typeof SEEDS)[]) {
    it(`matches every RFC 6238 vector for ${algorithm}`, () => {
      for (const [at, codes] of VECTORS) {
        assert.equal(totp(SEEDS[algorithm], at, { digits: 8, algorithm }), codes[algorithm], `T=${at}`);
      }
    });
  }

  it('defaults to six digits of SHA-1 over 30-second steps', () => {
    assert.equal(totp(SEEDS.sha1, 59), '287082');
  });

  it('keeps leading zeros', () => {
    assert.equal(totp(SEEDS.sha1, 1111111109, { digits: 8 }), '07081804');
  });

  it('gives the same code throughout one period and a new one after it', () => {
    assert.equal(totp(SEEDS.sha1, 30), totp(SEEDS.sha1, 59));
    assert.notEqual(totp(SEEDS.sha1, 59), totp(SEEDS.sha1, 60));
  });

  it('honours a custom period', () => {
    assert.equal(totp(SEEDS.sha1, 118, { period: 60 }), totp(SEEDS.sha1, 59));
  });

  it('uses the current time when none is given', () => {
    mock.timers.enable({ apis: ['Date'], now: 59_000 });
    try {
      assert.equal(totp(SEEDS.sha1), '287082');
    } finally {
      mock.timers.reset();
    }
  });

  it('reads a secret written in lowercase, with spaces, dashes and padding', () => {
    const messy = 'gezd gnbv-gy3t qojq gezd gnbv gy3t qojq====';
    assert.equal(totp(messy, 59), totp(SEEDS.sha1, 59));
  });

  it('refuses a secret with characters outside the base32 alphabet', () => {
    assert.throws(() => totp('GEZD1NBV', 59), {
      status: Status.BAD_REQUEST,
      message: 'TOTP secret is not valid base32',
    });
  });

  it('refuses a secret too short to hold one byte', () => {
    assert.throws(() => totp('  = ', 59), { status: Status.BAD_REQUEST, message: 'TOTP secret is empty' });
    assert.throws(() => totp('A', 59), { message: 'TOTP secret is empty' });
  });
});
