/**
 * The inputs for one second factor: its type, its secret or relay URL, the
 * OAuth client for a mailbox, and the site it is for.
 */
'use client';

import { mailbox, mfaHint, type MfaDraft } from './mfa';

/** Changes some fields of the draft. */
type Patch = (patch: Partial<MfaDraft>) => void;

/** What each part of the factor form reads and writes. */
interface PartProps {
  /** The factor as typed. */
  value: MfaDraft;
  /** Changes some of its fields. */
  set: Patch;
}

/** The type picker; `none` adds a "None" choice for forms where a factor is optional. */
function TypeSelect({ value, set, none }: PartProps & { /** Offer "None". */ none: boolean }) {
  return (
    <select
      className="field"
      value={value.type}
      onChange={(e) => set({ type: e.target.value as MfaDraft['type'] })}
      aria-label="MFA type"
    >
      {none && <option value="">None</option>}
      <option value="totp">TOTP</option>
      <option value="email">Email relay</option>
      <option value="sms">SMS relay</option>
      <option value="gmail">Gmail mailbox</option>
      <option value="graph">Microsoft 365 mailbox</option>
    </select>
  );
}

/** The seed, relay URL or refresh token, labelled for the chosen type. */
function ValueInput({ value, set }: PartProps) {
  const hint = mfaHint(value.type);
  if (!hint) return null;
  return (
    <input
      className="field font-mono"
      type={hint.secret ? 'password' : 'url'}
      autoComplete="off"
      aria-label={hint.label}
      value={value.value}
      onChange={(e) => set({ value: e.target.value })}
      placeholder={hint.placeholder}
    />
  );
}

/** A mailbox factor's OAuth client, and the tenant for Microsoft 365. */
function MailboxFields({ value, set }: PartProps) {
  return (
    <>
      <div className="grid grid-cols-2 gap-2">
        <input
          className="field font-mono"
          autoComplete="off"
          aria-label="OAuth client ID"
          value={value.clientId}
          onChange={(e) => set({ clientId: e.target.value })}
          placeholder="Client ID"
        />
        <input
          className="field font-mono"
          type="password"
          autoComplete="off"
          aria-label="OAuth client secret"
          value={value.clientSecret}
          onChange={(e) => set({ clientSecret: e.target.value })}
          placeholder="Client secret (if the app has one)"
        />
      </div>
      {value.type === 'graph' && (
        <input
          className="field font-mono"
          autoComplete="off"
          aria-label="Microsoft tenant"
          value={value.tenant}
          onChange={(e) => set({ tenant: e.target.value })}
          placeholder="Tenant ID (blank = common)"
        />
      )}
      <p className="text-[11.5px] text-text-muted">
        The code is read out of this mailbox and typed into the portal. Mail.Read and offline_access are the only scopes
        needed.
      </p>
    </>
  );
}

/** Every input a factor of the chosen type needs. */
export default function MfaFields(props: {
  /** The factor as typed. */
  value: MfaDraft;
  /** Hears every change. */
  onChange: (m: MfaDraft) => void;
  /** Offer "None", for forms where a factor is optional. */
  none?: boolean;
}) {
  const { value, onChange, none = false } = props;
  const set: Patch = (patch) => onChange({ ...value, ...patch });
  return (
    <div className="space-y-2">
      <div className="grid grid-cols-[140px_1fr] gap-2">
        <TypeSelect value={value} set={set} none={none} />
        <ValueInput value={value} set={set} />
      </div>
      {mailbox(value.type) && <MailboxFields value={value} set={set} />}
      {!!value.type && (
        <input
          className="field font-mono"
          autoComplete="off"
          aria-label="Site this factor is for"
          value={value.domain}
          onChange={(e) => set({ domain: e.target.value })}
          placeholder="Site this factor is for — blank applies to every site"
        />
      )}
    </div>
  );
}
