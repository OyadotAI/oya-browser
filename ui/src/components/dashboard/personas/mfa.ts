/**
 * A second factor as the forms hold it, and the body the server takes for it.
 * Shared by the create form and the drawer so both offer every type the
 * server accepts.
 */

/**
 * A second factor, however it arrives: a TOTP seed, a relay URL, or a mailbox
 * the code is read out of.
 */
export type MfaDraft = {
  /** Which kind of factor; empty means none. */
  type: '' | 'totp' | 'email' | 'sms' | 'gmail' | 'graph';
  /** The seed, relay URL or refresh token, depending on the type. */
  value: string;
  /** OAuth client for a mailbox factor. */
  clientId: string;
  /** OAuth client secret, when the mailbox app has one. */
  clientSecret: string;
  /** Microsoft tenant for a Microsoft 365 mailbox; blank means common. */
  tenant: string;
  /** Site the factor is for; blank applies to every site. */
  domain: string;
};

/** How the value input reads for each factor type. */
export interface MfaHint {
  /** Accessible name of the value input. */
  label: string;
  /** What to paste there. */
  placeholder: string;
  /** Whether the value is a secret, and so masked. */
  secret?: boolean;
}

/** An empty draft of the given type. */
export const newMfa = (type: MfaDraft['type'] = 'totp'): MfaDraft => ({
  type,
  value: '',
  clientId: '',
  clientSecret: '',
  tenant: '',
  domain: '',
});

/** Mailbox factors read the code out of mail, so they need an OAuth client too. */
export const mailbox = (type: MfaDraft['type']) => type === 'gmail' || type === 'graph';

/** Whether the draft holds enough for the server to accept it. */
export const mfaReady = (m: MfaDraft) => !!m.type && !!m.value.trim() && (!mailbox(m.type) || !!m.clientId.trim());

/** The optional site the factor is scoped to. */
const domainPart = (m: MfaDraft) => (m.domain.trim() ? { domain: m.domain.trim() } : {});

/** A mailbox factor: refresh token, OAuth client, and the optional secret and tenant. */
const mailboxPart = (m: MfaDraft) => ({
  type: m.type,
  refreshToken: m.value.trim(),
  clientId: m.clientId.trim(),
  ...(m.clientSecret.trim() ? { clientSecret: m.clientSecret.trim() } : {}),
  ...(m.type === 'graph' && m.tenant.trim() ? { tenant: m.tenant.trim() } : {}),
});

/** The type-specific part: a TOTP secret, a mailbox, or a relay URL. */
const factorPart = (m: MfaDraft) => {
  if (m.type === 'totp') return { type: m.type, secret: m.value.trim() };
  return mailbox(m.type) ? mailboxPart(m) : { type: m.type, url: m.value.trim() };
};

/** The body PUT /personas/:id/mfa takes for this draft. */
export const mfaBody = (m: MfaDraft) => ({ ...domainPart(m), ...factorPart(m) });

/** The relay types share one hint: the server polls the URL for the code. */
const RELAY_HINT: MfaHint = { label: 'Relay URL', placeholder: 'https://relay.example/latest — polled for the code' };
/** The mailbox types share one hint. */
const MAILBOX_HINT: MfaHint = {
  label: 'Refresh token',
  placeholder: 'OAuth refresh token for the mailbox',
  secret: true,
};

/** How the value input reads, by factor type. */
export const MFA_HINT: Record<string, MfaHint> = {
  totp: { label: 'Secret', placeholder: 'Base32 secret from the QR code', secret: true },
  email: RELAY_HINT,
  sms: RELAY_HINT,
  gmail: MAILBOX_HINT,
  graph: MAILBOX_HINT,
};

/** The hint for a type, or none when no type is chosen. */
export const mfaHint = (type: MfaDraft['type']) => (Object.hasOwn(MFA_HINT, type) ? MFA_HINT[type] : undefined);
