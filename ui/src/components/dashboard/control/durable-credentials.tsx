/**
 * Service credentials: invite members, remove them, and create or revoke
 * credentials for a role. New codes and tokens appear once, above.
 */
import { CREDENTIAL_ROLES, DURABLE_BUTTON, DURABLE_FIELD, SHORT_ID_CHARS } from './constants';
import type { Durable } from './use-durable';
import type { Overview } from './types';

/** Credentials as the overview lists them. */
type Credentials = NonNullable<Overview['credentials']>;

/** Invitation, members, the create form and every credential. */
export default function DurableCredentials({
  d,
  credentials,
}: {
  /** Project operations state and actions. */ d: Durable;
  /** Credentials the key may manage. */ credentials: Credentials;
}) {
  const { role } = d.form;
  return (
    <div className="space-y-4">
      <h3 className="text-sm font-medium">Service credentials</h3>
      <button
        disabled={d.busy}
        className={DURABLE_BUTTON}
        onClick={() => void d.act('/members/invite', 'POST', { role })}
      >
        Create {role} invitation
      </button>
      <MemberRows d={d} />
      <CredentialForm d={d} />
      {credentials.map((c) => (
        <CredentialRow key={c.id} c={c} d={d} />
      ))}
    </div>
  );
}

/** Each member, with Remove. */
function MemberRows({ d }: { /** Project operations state and actions. */ d: Durable }) {
  return d.members.map((m) => (
    <div key={m.userId} className="flex justify-between text-xs">
      <span>
        {m.userId.slice(0, SHORT_ID_CHARS)} · {m.role}
      </span>
      <button disabled={d.busy} className={DURABLE_BUTTON} onClick={() => void d.act(`/members/${m.userId}`, 'DELETE')}>
        Remove member
      </button>
    </div>
  ));
}

/** One credential: its label and role, and Revoke unless already revoked. */
function CredentialRow({
  c,
  d,
}: {
  /** The credential. */ c: Credentials[number];
  /** Project operations state and actions. */ d: Durable;
}) {
  return (
    <div className="flex items-center justify-between border-b border-border py-2 text-xs">
      <span>
        {c.label} <span className="text-text-dim">· {c.role}</span>
      </span>
      {c.revokedAt ? (
        <span className="text-text-dim">Revoked</span>
      ) : (
        <button
          disabled={d.busy}
          className={DURABLE_BUTTON}
          onClick={() => void d.act(`/credentials/${c.id}`, 'DELETE')}
        >
          Revoke
        </button>
      )}
    </div>
  );
}

/** Label and role for a new credential; the role also sets the invitation's. */
function CredentialForm({ d }: { /** Project operations state and actions. */ d: Durable }) {
  const { role, label } = d.form;
  return (
    <form
      className="flex flex-wrap gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        void d.act('/credentials', 'POST', { role, label });
      }}
    >
      <input
        aria-label="Credential label"
        className={`${DURABLE_FIELD} flex-1`}
        value={label}
        onChange={(e) => d.setForm({ label: e.target.value })}
        placeholder="Credential label"
      />
      <select
        aria-label="Credential role"
        className={`${DURABLE_FIELD} max-w-40`}
        value={role}
        onChange={(e) => d.setForm({ role: e.target.value })}
      >
        {CREDENTIAL_ROLES.map((r) => (
          <option key={r}>{r}</option>
        ))}
      </select>
      <button disabled={d.busy} className={DURABLE_BUTTON}>
        Create
      </button>
    </form>
  );
}
