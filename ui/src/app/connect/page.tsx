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

/** The app's name, or a plain stand-in when the request did not carry one. */
const appName = (client?: string) => client || 'the app';

/** The project dropdown, shown only when there is a choice to make; preselected to the last one connected. */
function ProjectPicker({ state, project, setProject }: Connect) {
  if (!state.projects?.[1]) return null;
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
        {state.projects?.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </select>
    </>
  );
}

/** The project picker when needed, and the two answers. */
function Answer(connect: Connect) {
  const { state, project, allow, deny } = connect;
  const busy = state.status === 'working';
  if (!state.projects?.length)
    return (
      <button type="button" className={secondary} disabled={busy} onClick={deny}>
        Back to {appName(state.client)}
      </button>
    );
  return (
    <>
      <ProjectPicker {...connect} />
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

/** The heading and line while the request waits for an answer: one project is named, several are chosen from. */
function asking({ client, redirectHost, projects }: Connect['state']): [string, string] {
  const app = appName(client);
  if (!projects?.length)
    return [
      'No project to connect',
      `Connecting needs an operator or administrator on a project. Ask one to add you, then start again from ${app}.`,
    ];
  const where = projects.length === 1 ? projects[0].name : 'the project you choose';
  return [
    `Allow ${app} to use Oya Browser?`,
    `${app} can open and drive browsers in ${where}. You return to ${redirectHost}.`,
  ];
}

/** The heading and line for each state. */
function content(state: Connect['state']): [string, string] {
  const { status, client, error } = state;
  if (status === 'signed-out') return ['Connect an app to Oya', 'Sign in or create an account to continue.'];
  if (status === 'missing') return ['No request in this link', 'Start connecting again from the app.'];
  if (status === 'expired') return ['This request has expired', `Start connecting again from ${appName(client)}.`];
  if (status === 'failed')
    return [
      'Could not connect',
      `${(error || 'Something went wrong').replace(/\.$/, '')}. Try again, or start again from ${appName(client)}.`,
    ];
  return asking(state);
}

/** A failed answer can be tried again from the same link. */
function Retry() {
  return (
    <button type="button" className={primary} onClick={() => window.location.reload()}>
      Try again
    </button>
  );
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
  const answering = status === 'ready' || status === 'working';
  // While the person is answering, Allow is the only way on: the dashboard link would be a second.
  const footer = answering ? null : <Footer signedOut={status === 'signed-out'} />;
  return (
    <AuthShell glow="accent" footer={footer}>
      <h1 className="mb-2 font-display text-2xl font-bold text-text">{title}</h1>
      <p className="mb-6 text-sm text-text-muted" role={status === 'failed' ? 'alert' : undefined}>
        {body}
      </p>
      {answering && <Answer {...connect} />}
      {status === 'failed' && <Retry />}
    </AuthShell>
  );
}
