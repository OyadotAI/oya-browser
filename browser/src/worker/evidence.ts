/**
 * Evidence: what a page did during a run (console severity, requests,
 * dialogs, downloads), reported as run events without page content.
 */
import type { ConsoleMessage, Dialog, Page, Request, Response } from '@playwright/test';
import { safeUrl } from '../workflow/index.ts';
import type { RunState } from './run-state.ts';
import type { RunEvent } from './types.ts';

/** One evidence event: its kind and what it carries. */
interface Evidence extends RunEvent {
  /** console, network, attention or download. */
  kind: string;
}

/** Reports one evidence event; the step running then is added. */
type Report = (event: Evidence) => void;

/**
 * An alert has one button, so dismissing it stalled replays that a person
 * had clicked straight through. Accept those; a confirm or prompt is a
 * decision nobody recorded, so it is still refused, but say which, and
 * what it asked, instead of a fixed string.
 */
async function answerDialog(dialog: Dialog, report: Report): Promise<void> {
  const accept = ['alert', 'beforeunload'].includes(dialog.type());
  const outcome = accept ? 'accepted' : 'dismissed (no recorded checkpoint)';
  report({ kind: 'attention', message: `Browser ${dialog.type()}: "${dialog.message()}", ${outcome}.` });
  await (accept ? dialog.accept() : dialog.dismiss());
}

/** Reports a console error or warning by severity only. */
function consoleEvent(msg: ConsoleMessage, report: Report): void {
  if (!['error', 'warning'].includes(msg.type())) return;
  report({ kind: 'console', level: msg.type(), message: 'Page console ' + msg.type() + ' (message omitted)' });
}

/** Page event → what it reports. */
const PAGE_EVENTS = {
  console: consoleEvent,
  pageerror: (_: unknown, report: Report) =>
    report({ kind: 'console', level: 'error', message: 'Uncaught page error' }),
  requestfailed: (request: Request, report: Report) =>
    report({ kind: 'network', url: safeUrl(request.url()), message: 'Request failed' }),
  response: (response: Response, report: Report) =>
    report({ kind: 'network', url: safeUrl(response.url()), status: response.status() }),
  dialog: answerDialog,
  download: (_: unknown, report: Report) => report({ kind: 'download', message: 'Download started' }),
};

/** One page event's handler, as the loop subscribes it. */
type Handler = (value: never, report: Report) => unknown;

/** Listens to a page for the rest of the run; each event is tagged with the step running then. */
export function attachEvidence(p: Page, emit: (event: RunEvent) => void, state: RunState): void {
  const report: Report = ({ kind, ...rest }) => emit({ kind, stepId: state.current, ...rest });
  const on = p.on.bind(p) as (event: string, listener: (value: never) => unknown) => Page;
  for (const [event, handle] of Object.entries(PAGE_EVENTS) as [string, Handler][]) {
    on(event, (value) => handle(value, report));
  }
}
