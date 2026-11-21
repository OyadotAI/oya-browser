/**
 * Where an app such as claude.ai sends a person to connect it to Oya: sign in
 * (or sign up) if needed, see which app asks and where its answer goes, then
 * allow it for one project or turn it down. Allowing gives the app a credential
 * the person can revoke under the project's service credentials.
 */
'use client';

import Link from 'next/link';
import { AuthLoading, AuthShell } from '@/components/auth/auth-shell';
import { useConnect, type Connect } from './use-connect';

/** A link styled like the auth pages' links. */
const linkClass = 'font-medium text-accent hover:text-accent-hover';

/** The primary button's look, as the sign-in forms use it. */
const primary =
  'flex-1 rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-accent-foreground hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-60';

/** The secondary button's look. */
const secondary =
  'flex-1 rounded-lg border border-border px-4 py-2.5 text-sm font-medium text-text hover:bg-text/5 disabled:cursor-not-allowed disabled:opacity-60';

/** The project picker and the two answers. */
function Answer({ state, project, setProject, allow, deny }: Connect) {
  const busy = state.status === 'working';
  if (!state.projects?.length)
    return (
      <>
        <p className="mb-5 text-sm text-text-muted">
          You have no project you can connect: that takes an operator or administrator.
        </p>
        <button type="button" className={secondary} disabled={busy} onClick={deny}>
          Cancel
        </button>
      </>
    );
  return (
    <>
      <label htmlFor="connect-project" className="mb-1.5 block text-[13px] font-medium text-text">
        Project
      </label>
      <select
        id="connect-project"
        className="settings-input mb-5 w-full"
        value={project}
        onChange={(e) => setProject(e.target.value)}
      >
        {state.projects.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </select>
      <div className="flex gap-3">
        <button type="button" className={secondary} disabled={busy} onClick={deny}>
          Cancel
        </button>
        <button type="button" className={primary} disabled={busy || !project} onClick={allow}>
          {busy ? 'Connecting…' : 'Allow'}
        </button>
      </div>
    </>
  );
}

/** The heading and line for each state. */
function content({ status, client, redirectHost, error }: Connect['state']): [string, string] {
  if (status === 'signed-out') return ['Connect an app to Oya', 'Sign in or create an account to continue.'];
  if (status === 'missing') return ['No request in this link', 'Start connecting again from the app.'];
  if (status === 'failed') return ['Could not connect', error || 'Something went wrong'];
  return [
    `Allow ${client} to use Oya?`,
    `${client} will be able to start and drive browsers in the project you choose. You return to ${redirectHost}.`,
  ];
}

/** The way on: sign in when signed out, else back to the dashboard. */
function Footer({ signedOut }: { /** Whether nobody is signed in. */ signedOut: boolean }) {
  if (!signedOut)
    return (
      <Link href="/dashboard" className={linkClass}>
        Go to the dashboard
      </Link>
    );
  return (
    <>
      <Link href="/login" className={linkClass}>
        Sign in
      </Link>{' '}
      or{' '}
      <Link href="/signup" className={linkClass}>
        create an account
      </Link>
    </>
  );
}

/** Asks the signed-in person to allow an app's connection. */
export default function ConnectPage() {
  const connect = useConnect();
  const { status } = connect.state;
  if (status === 'loading') return <AuthLoading />;
  const [title, body] = content(connect.state);
  return (
    <AuthShell glow="accent" footer={<Footer signedOut={status === 'signed-out'} />}>
      <h1 className="mb-2 font-display text-2xl font-bold text-text">{title}</h1>
      <p className="mb-6 text-sm text-text-muted" role={status === 'failed' ? 'alert' : undefined}>
        {body}
      </p>
      {(status === 'ready' || status === 'working') && <Answer {...connect} />}
    </AuthShell>
  );
}
