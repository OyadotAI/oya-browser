/**
 * Answering an app's request to connect to Oya (claude.ai, over OAuth). The
 * request id arrives in the link and waits in this tab's storage through a
 * sign-in; once someone is signed in the page shows which app asks and lets
 * them allow it for one of their projects, or turn it down. Either answer sends
 * the browser back to the app.
 */
'use client';

import { useCallback, useEffect, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import { call, message } from '@/components/dashboard/projects/project-api';
import type { Failure, Project } from '@/components/dashboard/projects/types';
import { FIRST_PROJECT_NAME, HttpStatus } from '@/components/dashboard/projects/constants';
import { createApiKey } from '@/lib/api';
import { forgetConnect, lastProject, pendingConnect, rememberConnect, rememberProject } from '@/lib/connect';

/** Where the request stands. */
export interface ConnectState {
  /** Loading, waiting on a sign-in, ready to answer, answering, refused, expired, or no request in the link. */
  status: 'loading' | 'signed-out' | 'ready' | 'working' | 'failed' | 'expired' | 'missing';
  /** The app asking. */
  client?: string;
  /** The host the answer goes back to. */
  redirectHost?: string;
  /** Projects the person can connect, operators and up. */
  projects?: Project[];
  /** Why it failed, when it did. */
  error?: string;
}

/** What the page needs: the state, the chosen project, and the two answers. */
export interface Connect {
  /** Where the request stands. */
  state: ConnectState;
  /** The project chosen to connect. */
  project: string;
  /** Chooses the project. */
  setProject: (id: string) => void;
  /** Allows the request for the chosen project. */
  allow: () => void;
  /** Turns the request down. */
  deny: () => void;
}

/** Which app asks, as GET /auth/connect/:id answers. */
interface Asking {
  /** The app's name. */
  client: string;
  /** The host the answer goes back to. */
  redirectHost: string;
}

/** An answer's reply: where to send the browser. */
interface Answered {
  /** The app's redirect URI, carrying a code or an error. */
  redirect: string;
}

/** Moves a request id in the link to storage; the request waiting to be answered, or ''. */
function takeRequestFromUrl() {
  const fromLink = new URLSearchParams(window.location.search).get('request');
  if (fromLink) rememberConnect(fromLink);
  return pendingConnect();
}

/** The request's app and the projects a viewer cannot connect left out (MCP needs an operator). */
async function load(token: string, id: string): Promise<ConnectState> {
  const path = `/auth/connect/${encodeURIComponent(id)}`;
  const [asking, projects] = await Promise.all([
    call<Asking>(token, path, {}, 'Could not load this request'),
    projectsOf(token),
  ]);
  return { status: 'ready', ...asking, projects: projects.filter((p) => p.role !== 'viewer') };
}

/** The account's projects, as /auth/projects lists them. */
const listProjects = (token: string) => call<Project[]>(token, '/auth/projects', {}, 'Could not load projects');

/**
 * The account's projects. Someone who just signed up from the chat app has none yet, and would
 * be told to ask an administrator: they get the first project the dashboard would have made.
 */
async function projectsOf(token: string) {
  const projects = await listProjects(token);
  if (projects.length) return projects;
  await createApiKey(token, FIRST_PROJECT_NAME);
  return listProjects(token);
}

/** The redirect as a URL to follow, refusing anything but http(s): a javascript: one would run on this origin. */
function followable(redirect: string) {
  const url = URL.parse(redirect);
  if (url?.protocol !== 'https:' && url?.protocol !== 'http:')
    throw new Error('The app gave an unsafe address to return to');
  return url.href;
}

/** Sends one answer and follows its redirect back to the app. */
async function answer(token: string, id: string, verb: 'allow' | 'deny', project?: string) {
  const init = { method: 'POST', body: JSON.stringify(project ? { project } : {}) };
  const { redirect } = await call<Answered>(token, `/auth/connect/${encodeURIComponent(id)}/${verb}`, init);
  forgetConnect();
  if (project) rememberProject(project);
  window.location.assign(followable(redirect));
}

/**
 * A request the server no longer holds (410: it expired or was answered) gets its
 * own screen, telling the person to start again from the app; anything else is a
 * failure they can retry.
 */
function refused(e: unknown): ConnectState {
  if ((e as Failure)?.status !== HttpStatus.GONE) return { status: 'failed', error: message(e) };
  // Forgotten, or every later sign-in in this tab would land back on the expired request.
  forgetConnect();
  return { status: 'expired' };
}

/** The request's state once the session is known. */
function useRequest(token: string | null, loading: boolean) {
  const [state, setState] = useState<ConnectState>({ status: 'loading' });
  useEffect(() => {
    if (loading) return;
    const id = takeRequestFromUrl();
    if (!id || !token) return void queueMicrotask(() => setState({ status: id ? 'signed-out' : 'missing' }));
    load(token, id).then(setState, (e) => setState(refused(e)));
  }, [loading, token]);
  return [state, setState] as const;
}

/** Sends an answer, showing it as in flight, and as failed if the server refuses it. */
function useSend(token: string | null, project: string, setState: (next: (s: ConnectState) => ConnectState) => void) {
  return useCallback(
    (verb: 'allow' | 'deny') => {
      setState((s) => ({ ...s, status: 'working' }));
      const fail = (e: unknown) => setState((s) => ({ ...s, ...refused(e) }));
      answer(token || '', pendingConnect(), verb, verb === 'allow' ? project : undefined).catch(fail);
    },
    [token, project, setState],
  );
}

/** The project the person connected last time, when they can still connect it; else their first. */
function preselected(projects: Project[] = []) {
  const last = lastProject();
  return projects.find((p) => p.id === last)?.id || projects[0]?.id || '';
}

/** The connect page's state and answers. */
export function useConnect(): Connect {
  const { token, loading } = useAuth();
  const [state, setState] = useRequest(token, loading);
  const [chosen, setProject] = useState('');
  const project = chosen || preselected(state.projects);
  const send = useSend(token, project, setState);
  return { state, project, setProject, allow: () => send('allow'), deny: () => send('deny') };
}
