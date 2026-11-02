/**
 * The tester's state: the browser to run on, a sample value per variable, strict
 * or healing, and the run it started, followed until it ends so its steps show
 * as a timeline. Nothing it runs is saved.
 */
import { useEffect } from 'react';
import { errorMessage } from '@/lib/api-client';
import { usePatchState, type Patch } from '@/lib/hooks/use-patch-state';
import { followRun } from '../run-steps';
import { testDefinition, type Definition, type TestRun } from './editor-api';
import { variableRows } from './definition';

/** The tester's state. */
interface TesterFields {
  /** The browser to run on. */
  browserId: string;
  /** Sample values by variable. */
  values: Record<string, string>;
  /** Let the agent finish a broken step. */
  autoHeal: boolean;
  /** The run, once started. */
  run: TestRun | null;
  /** Why the run could not start. */
  error: string | null;
}

/** Whether a run is over. */
const ended = (run: TestRun | null) => !run || (run.status !== 'running' && run.status !== 'needs_attention');

/** Follows the run until it ends. */
function useFollow(apiKey: string, run: TestRun | null, patch: Patch<TesterFields>) {
  useEffect(() => {
    if (ended(run)) return;
    return followRun(apiKey, (run as TestRun).id, {
      setRun: (r) => patch({ run: r as TestRun }),
      onFinished: () => undefined,
    });
    // ended(run) is a dependency so the poll stops once the run is over.
  }, [apiKey, run?.id, ended(run)]); // eslint-disable-line react-hooks/exhaustive-deps
}

/** A tester with nothing started, on `browserId`. */
const fresh = (browserId: string): TesterFields => ({ browserId, values: {}, autoHeal: false, run: null, error: null });

/** The browser, values, healing choice, the run, and Run. */
export function useTester(apiKey: string, definition: Definition | null, firstBrowser: string) {
  const [state, patch] = usePatchState<TesterFields>(fresh(firstBrowser));
  useFollow(apiKey, state.run, patch);
  const sample = (name: string, value: string) => patch((s) => ({ values: { ...s.values, [name]: value } }));
  const start = () => definition && launch(apiKey, definition, state, patch);
  return { ...state, patch, sample, start, running: !ended(state.run) };
}

/** The values a test sends: each variable's sample, else its default; empty ones left out. */
function dataFor(definition: Definition, values: Record<string, string>) {
  const pairs = variableRows(definition).map((v) => ({ name: v.name, value: values[v.name] ?? v.value }));
  return Object.fromEntries(pairs.filter((p) => p.value !== '').map((p) => [p.name, p.value]));
}

/** Starts the test run, or says why it could not. */
async function launch(apiKey: string, definition: Definition, s: TesterFields, patch: Patch<TesterFields>) {
  patch({ error: null });
  const body = { playbook: definition, data: dataFor(definition, s.values), autoHeal: s.autoHeal };
  await testDefinition(apiKey, s.browserId, body).then(
    (run) => patch({ run }),
    (err) => patch({ error: errorMessage(err) }),
  );
}

/** What the hook gives the tester. */
export type TesterState = ReturnType<typeof useTester>;
