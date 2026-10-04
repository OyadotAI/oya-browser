/**
 * A stand-in for a Supabase Auth client signed in as one person, for the MFA
 * tests: it answers setSession and the mfa.* calls from what it was given and
 * records what it was asked to do.
 */

/** A Supabase access token carrying `aal` (unsigned: only its payload is read). */
export const tokenWithAal = (aal: string) =>
  `h.${Buffer.from(JSON.stringify({ sub: 'u1', aal })).toString('base64url')}.s`;

/** The code the fake treats as wrong. */
export const WRONG_CODE = '000000';

/** What the fake knows: the person's factors, the session's level, and whether its tokens sign in. */
interface FakeMfaOptions {
  /** The person's factors, as Supabase lists them. */
  factors?: any[];
  /** The session's assurance level. */
  aal?: string;
  /** When true, setSession refuses the tokens. */
  rejectSession?: boolean;
  /** The person's id. */
  userId?: string;
}

/** A fake Supabase Auth client and the calls made on it. */
export function fakeMfaAuth({ factors = [], aal = 'aal1', rejectSession = false, userId = 'u1' }: FakeMfaOptions = {}) {
  const calls: any[] = [];
  const ok = (data: any) => ({ data, error: null });
  const user = { id: userId, email: `${userId}@example.com`, factors };
  const mfa = {
    listFactors: async () => ok({ all: factors }),
    getAuthenticatorAssuranceLevel: async () => ok({ currentLevel: aal, nextLevel: aal }),
    enroll: async (params: any) => (calls.push(['enroll', params]), ok(ENROLLED)),
    unenroll: async ({ factorId }: any) => (calls.push(['unenroll', factorId]), ok({ id: factorId })),
    challengeAndVerify: async ({ factorId, code }: any) => (
      calls.push(['verify', factorId, code]),
      verified(user, code)
    ),
  };
  const setSession = async (tokens: any) => (calls.push(['setSession', tokens]), sessionAnswer(user, rejectSession));
  return { calls, setSession, mfa };
}

/** What Supabase's enroll answers for an authenticator app. */
const ENROLLED = {
  id: 'f-new',
  type: 'totp',
  totp: { qr_code: 'data:image/svg+xml;utf-8,<svg/>', secret: 'SECRET', uri: 'otpauth://totp/Oya' },
};

/** setSession's answer: the person, or Supabase's refusal. */
const sessionAnswer = (user: any, reject: boolean) =>
  reject ? { data: { user: null, session: null }, error: { message: 'Invalid JWT' } } : { data: { user }, error: null };

/** challengeAndVerify's answer: new aal2 tokens, or Supabase's refusal of a wrong code. */
const verified = (user: any, code: string) =>
  code === WRONG_CODE
    ? { data: null, error: { message: 'Invalid TOTP code entered' } }
    : { data: { access_token: tokenWithAal('aal2'), refresh_token: 'rt-aal2', expires_at: 9, user }, error: null };
