/**
 * The run dialog's state: where to run (profile, browser), with what values,
 * and the run it started, followed until it ends, answered when it pauses.
 */
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useToast } from '../toast';
import type { BrowserRow } from '../types';
import { CONNECT_TIMEOUT_MESSAGE } from './constants';
import { isEnded } from './format';
import {
  answerRun,
  followRun,
  keepInFilter,
  launchRun,
  settlePending,
  startPending,
  type PendingBrowser,
} from './run-steps';
import type { PlaybookInfo, RunInfo } from './types';

/**
 * A profile is chosen when a browser starts, never at replay, so picking one here
 * means picking a browser already running it, or starting one that does.
 */
export function useRunTarget(browsers: BrowserRow[]) {
  const [persona, setPersona] = useState('');
  const matching = useMemo(
    () => (persona ? browsers.filter((b) => b.persona === persona) : browsers),
    [browsers, persona],
  );
  const [browserId, setBrowserId] = useState(browsers[0]?.id || '');
  // Keep the browser choice inside the profile filter.
  useEffect(() => keepInFilter(matching, browserId, setBrowserId), [matching, browserId]);
  return { persona, setPersona, matching, browserId, setBrowserId };
}

/**
 * The variable values, prefilled with what the recording used, which is what the
 * replay does with an untouched field anyway, so the form shows the run it is about
 * to make, and whether to auto-heal.
 */
export function useRunForm(playbook: PlaybookInfo) {
  const [values, setValues] = useState<Record<string, string>>(() => ({ ...playbook.defaults }));
  const [autoHeal, setAutoHeal] = useState(true);
  return { values, setValues, autoHeal, setAutoHeal };
}

/** The run, and a start that replays with the form's values. */
function useRunStart(apiKey: string, playbook: string, form: ReturnType<typeof useRunForm>, toast: Toast) {
  const [run, setRun] = useState<RunInfo | null>(null);
  const [starting, setStarting] = useState(false);
  const start = useCallback(
    (on: string) =>
      launchRun(apiKey, on, { playbook, data: form.values, autoHeal: form.autoHeal }, { setRun, setStarting, toast }),
    [apiKey, playbook, form.values, form.autoHeal, toast],
  );
  return { run, setRun, starting, setStarting, start, toast };
}

/** The run and its start. */
type Runner = ReturnType<typeof useRunStart>;

/** Shows a toast. */
type Toast = ReturnType<typeof useToast>;

/** Polls the run while it is going. */
function useRunFollow(apiKey: string, runner: Runner, onFinished: () => void) {
  const { run, setRun, toast } = runner;
  const runId = run?.id;
  const ended = isEnded(run?.status);
  useEffect(() => {
    if (!runId || ended) return;
    return followRun(apiKey, runId, { setRun, onFinished, onTrouble: (message) => toast(message, 'error') });
  }, [runId, ended, apiKey, onFinished, setRun, toast]);
}

/** Nothing is running on this profile: start one, then replay on it once it dials in. */
function usePendingBrowser(apiKey: string, persona: string, browsers: BrowserRow[], runner: Runner) {
  const [pending, setPending] = useState<PendingBrowser | null>(null);
  const { start, setStarting, toast } = runner;
  useEffect(() => {
    if (!pending) return;
    const timeout = () => (setStarting(false), toast(CONNECT_TIMEOUT_MESSAGE, 'error'));
    settlePending(pending, browsers, { clear: () => setPending(null), start: (id) => void start(id), timeout });
  }, [pending, browsers, start, toast, setStarting]);
  return () => startPending(apiKey, persona, { setStarting, setPending, toast });
}

/** The reply box for a paused run, and sending it. */
function useReply(apiKey: string, runner: Runner) {
  const [reply, setReply] = useState('');
  const { run, setRun, toast } = runner;
  const respond = () => (run ? answerRun(apiKey, run, reply, { setReply, setRun, toast }) : undefined);
  return { reply, setReply, respond };
}

/** Everything the run dialog shows and does. */
export function useRunDialog(apiKey: string, playbook: PlaybookInfo, browsers: BrowserRow[], onFinished: () => void) {
  const target = useRunTarget(browsers);
  const form = useRunForm(playbook);
  const runner = useRunStart(apiKey, playbook.name, form, useToast());
  useRunFollow(apiKey, runner, onFinished);
  const startAndRun = usePendingBrowser(apiKey, target.persona, browsers, runner);
  const reply = useReply(apiKey, runner);
  return { target, form, runner, startAndRun, reply };
}

/** The run dialog's state and actions. */
export type RunDialogState = ReturnType<typeof useRunDialog>;
